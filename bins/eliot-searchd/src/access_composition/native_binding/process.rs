//! Root-owned standalone provider process state after durable bootstrap.
//!
//! This composition retains the real data-root owner guard, the exact persistent
//! control journal, its admission snapshot, the process-wide rollback-fenced clock,
//! the finite connection registry and the canonical handle/continuation stores.
//! None can outlive the root lock through this API. It creates no alternate store,
//! token-file route, implicit registration, listener or source authority.

use core::fmt;
use std::fs::File;
use std::net::TcpStream;

use search_contracts::{BindingId, OpaqueId};
use search_continuation::{
    ContinuationCleanup, ContinuationError, ContinuationLimits, ContinuationStore,
};
use search_control_redb::{
    ControlCallError, ControlError, ControlSnapshotPublisher, JournalIdentity, JournalLimits,
    MutationId, PersistentControlJournal,
};
use search_handles::{HandleError, HandlePolicy, HandleStore};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, MonotonicMillis, PairingMachine, ProtocolLimits, ServerNonce,
};

use crate::development::DataRootGuard;
use crate::provider_composition::{
    CanonicalProviderConnection, CanonicalTcpConnection, monotonic_millis,
};

use super::{
    BindingConnectionRegistry, BindingConnectionRegistryError, NativeBindingError,
    NativeBindingExpectation, NativeBindingPin, NativeTcpOpenError, StandaloneBootstrapError,
    StandaloneBootstrapReady, SystemGrantClock, begin, check,
    restore_and_publish_standalone_registration,
};

const OPEN_OPERATION_DOMAIN: &[u8] = b"ELIOT-STANDALONE-PROCESS-OPEN-v1\0";
const OWNER_HANDOFF_OPERATION_DOMAIN: &[u8] = b"ELIOT-STANDALONE-PROCESS-OWNER-HANDOFF-v1\0";

/// Failure while taking ownership of an existing native provider process state.
///
/// Every variant is content-free. A failure after redb open/handoff or bootstrap
/// may follow an external effect; it never authorizes retry with changed inputs,
/// listener admission, fallback routing or deletion of the journal/credential.
#[derive(Debug)]
pub enum StandaloneProcessError<E> {
    /// Trusted journal/root coordinates were malformed or did not match.
    Identity(ControlError),
    /// Persistent open or owner-epoch handoff failed, possibly after native work.
    Control(ControlCallError),
    /// Finite live-connection registry configuration was invalid.
    Registry(BindingConnectionRegistryError),
    /// Canonical handle owner configuration was invalid.
    Handle(HandleError),
    /// Canonical continuation owner configuration was invalid.
    Continuation(ContinuationError),
    /// Durable registration recovery/final publication did not complete.
    Bootstrap(StandaloneBootstrapError<E>),
}

impl<E: fmt::Display> fmt::Display for StandaloneProcessError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => fmt::Display::fmt(error, formatter),
            Self::Control(error) => fmt::Display::fmt(error, formatter),
            Self::Registry(error) => fmt::Display::fmt(error, formatter),
            Self::Handle(error) => fmt::Display::fmt(error, formatter),
            Self::Continuation(error) => fmt::Display::fmt(error, formatter),
            Self::Bootstrap(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for StandaloneProcessError<E> {}

impl<E> From<ControlError> for StandaloneProcessError<E> {
    fn from(error: ControlError) -> Self { Self::Identity(error) }
}

impl<E> From<ControlCallError> for StandaloneProcessError<E> {
    fn from(error: ControlCallError) -> Self { Self::Control(error) }
}

impl<E> From<BindingConnectionRegistryError> for StandaloneProcessError<E> {
    fn from(error: BindingConnectionRegistryError) -> Self { Self::Registry(error) }
}

impl<E> From<HandleError> for StandaloneProcessError<E> {
    fn from(error: HandleError) -> Self { Self::Handle(error) }
}

impl<E> From<ContinuationError> for StandaloneProcessError<E> {
    fn from(error: ContinuationError) -> Self { Self::Continuation(error) }
}

/// Sole process owner for one bootstrapped standalone binding.
///
/// Field order is intentional: connection/query state and the redb handle drop
/// before the non-clonable data-root guard releases OS exclusion. Dropping without
/// explicit clean shutdown is fail-stop owner recovery on the next process, not a
/// fabricated RELEASED receipt. A future listener owner must itself drop every
/// accepted transport before this object.
pub struct StandaloneProcessOwner {
    ready: StandaloneBootstrapReady,
    clock: SystemGrantClock,
    continuations: ContinuationStore,
    handles: HandleStore,
    connections: BindingConnectionRegistry,
    publisher: ControlSnapshotPublisher,
    journal: PersistentControlJournal,
    root: DataRootGuard,
}

impl StandaloneProcessOwner {
    /// Open one existing exact journal under the already-acquired root guard,
    /// perform an immediate owner-epoch handoff when required, and finish the
    /// retained standalone registration before any socket can be admitted.
    ///
    /// `file` must be the verified final regular-file handle opened under `root`.
    /// `stored_identity` is independently trusted metadata for that exact handle,
    /// not data echoed from the journal. All immutable coordinates must match the
    /// current root owner. Its owner epoch may equal the current epoch or be its
    /// immediate predecessor only; skipped/guessed epochs fail before redb open.
    ///
    /// One decreasing deadline covers journal open, optional owner handoff,
    /// bootstrap recovery, publication, session fencing, dependent invalidation
    /// and cleanup. Derived operation IDs are correlation-only and never become a
    /// second mutation identity or registration receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn restore_existing<C, E, F>(
        root: DataRootGuard,
        file: File,
        stored_identity: JournalIdentity,
        journal_limits: JournalLimits,
        connection_capacity: usize,
        handle_policy: HandlePolicy,
        continuation_limits: ContinuationLimits,
        binding_id: BindingId,
        operation_id: MutationId,
        expected: NativeBindingExpectation,
        cleanup: &mut F,
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneProcessError<E>>
    where
        C: CancellationProbe + Clone,
        F: FnMut(&ContinuationCleanup, &OperationContext<C>) -> Result<(), E>,
    {
        let (started, deadline) = begin(context).map_err(process_native)?;
        let (current_identity, handoff_required) =
            current_journal_identity(&root, stored_identity)?;

        let call = remaining_context(context, started, deadline).map_err(process_native)?;
        let open_operation = derived_operation_id(OPEN_OPERATION_DOMAIN, operation_id);
        let mut journal = PersistentControlJournal::open_with_context(
            file,
            stored_identity,
            journal_limits,
            open_operation,
            &call,
        )?;

        if handoff_required {
            let call = remaining_context(context, started, deadline).map_err(process_native)?;
            let handoff_operation =
                derived_operation_id(OWNER_HANDOFF_OPERATION_DOMAIN, operation_id);
            journal = journal.advance_owner_with_context(
                current_identity,
                handoff_operation,
                &call,
            )?;
        }
        if journal.identity() != current_identity {
            return Err(StandaloneProcessError::Identity(ControlError::IdentityMismatch));
        }

        let mut publisher = ControlSnapshotPublisher::new();
        let mut connections = BindingConnectionRegistry::new(connection_capacity)?;
        let mut handles = HandleStore::new(handle_policy)?;
        let mut continuations = ContinuationStore::new(continuation_limits)?;

        let call = remaining_context(context, started, deadline).map_err(process_native)?;
        let ready = restore_and_publish_standalone_registration(
            &mut journal,
            &mut publisher,
            &mut connections,
            &mut handles,
            &mut continuations,
            cleanup,
            binding_id,
            operation_id,
            expected,
            &call,
        ).map_err(StandaloneProcessError::Bootstrap)?;
        check(context, started, deadline).map_err(process_native)?;

        Ok(Self {
            ready,
            clock: SystemGrantClock::default(),
            continuations,
            handles,
            connections,
            publisher,
            journal,
            root,
        })
    }

    /// Open one authenticated socket only through the completed process bootstrap.
    ///
    /// The retained root guard and exact journal/publisher remain alive across the
    /// credential read, binding/policy checks and profile handshake. The shared
    /// rollback-fenced clock is not reset per connection. The returned transport
    /// must be dropped before this process owner.
    #[allow(clippy::too_many_arguments)]
    pub fn open_tcp<C: CancellationProbe + Clone>(
        &mut self,
        stream: TcpStream,
        binding: BindingContext,
        ceremony: PairingMachine,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
        boot_id: &OpaqueId,
        context: &OperationContext<C>,
    ) -> Result<(CanonicalTcpConnection, NativeBindingPin), NativeTcpOpenError> {
        CanonicalProviderConnection::open_standalone_tcp(
            stream,
            binding,
            ceremony,
            server_nonce,
            limits,
            &self.journal,
            &self.publisher,
            &mut self.connections,
            &self.ready,
            boot_id,
            &mut self.clock,
            context,
        )
    }

    /// Executed startup evidence retained by this process owner.
    #[must_use]
    pub const fn readiness(&self) -> &StandaloneBootstrapReady { &self.ready }

    /// Exact current journal identity after any owner handoff.
    #[must_use]
    pub const fn journal_identity(&self) -> JournalIdentity { self.journal.identity() }

    /// Canonical query-lifecycle owners for construction of the production host.
    ///
    /// The process owner and therefore the root lock remain borrowed for the full
    /// callback. No journal, publisher, bootstrap evidence or connection registry
    /// is exposed as mutable authority through this seam.
    pub fn with_query_stores<R>(
        &mut self,
        use_stores: impl FnOnce(&mut HandleStore, &mut ContinuationStore) -> R,
    ) -> R {
        use_stores(&mut self.handles, &mut self.continuations)
    }

    /// Canonical root protected for this owner's entire lifetime.
    #[must_use]
    pub fn canonical_root(&self) -> &std::path::Path { self.root.canonical_root() }
}

impl fmt::Debug for StandaloneProcessOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneProcessOwner")
            .field("ready", &self.ready)
            .field("registered_connections", &self.connections.len())
            .field("handles", &self.handles.len())
            .field("continuations", &self.continuations.len())
            .finish_non_exhaustive()
    }
}

fn current_journal_identity(
    root: &DataRootGuard,
    stored: JournalIdentity,
) -> Result<(JournalIdentity, bool), ControlError> {
    let _ = stored.validate()?;
    let (installation_incarnation_id, data_root_id, owner_epoch) =
        root.journal_owner_inputs();
    if stored.installation_incarnation_id != installation_incarnation_id
        || stored.data_root_id != data_root_id
    {
        return Err(ControlError::IdentityMismatch);
    }
    let current = JournalIdentity {
        installation_incarnation_id,
        data_root_id,
        owner_epoch,
        ..stored
    };
    if stored.owner_epoch == owner_epoch {
        return Ok((current, false));
    }
    if stored.owner_epoch.checked_next().ok() != Some(owner_epoch) {
        return Err(ControlError::IdentityMismatch);
    }
    Ok((current, true))
}

fn derived_operation_id(domain: &[u8], parent: MutationId) -> MutationId {
    let mut hash = blake3::Hasher::new();
    hash.update(domain);
    hash.update(&parent.0);
    MutationId(*hash.finalize().as_bytes())
}

fn process_native<E>(error: NativeBindingError) -> StandaloneProcessError<E> {
    match error {
        NativeBindingError::Control(error) => StandaloneProcessError::Identity(error),
        NativeBindingError::Call(error) => StandaloneProcessError::Control(error),
        other => StandaloneProcessError::Bootstrap(StandaloneBootstrapError::Provisioning(
            other.into(),
        )),
    }
}

fn remaining_context<C: CancellationProbe + Clone>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<OperationContext<C>, NativeBindingError> {
    check(context, started, deadline)?;
    let remaining = deadline
        .get()
        .checked_sub(monotonic_millis().get())
        .filter(|left| *left > 0)
        .ok_or(NativeBindingError::Interrupted)?;
    OperationContext::new(
        context.request_id(),
        remaining,
        context.cancellation().clone(),
        context.budget_ref().clone(),
    ).map_err(|_| NativeBindingError::Interrupted)
}
