//! Durable provider bindings on the existing technical journal (W8 section 3).
//! Binding metadata is separate from grant policy, credentials and source scope.

mod codec;
mod journal_open;
mod opening;
mod process;
mod registration;
mod startup;

pub use journal_open::{
    StandaloneProcessOpenError, restore_existing_standalone_process,
};
pub use opening::{
    BindingConnectionRegistry, BindingConnectionRegistryError, BindingDrainReceipt,
    MAX_REGISTERED_BINDING_CONNECTIONS, NativeBindingExpectation,
    NativePairingCredentialError, NativePairingCredentialIntent, NativeTcpOpenError,
};
pub use process::{StandaloneProcessError, StandaloneProcessOwner};
pub use registration::{
    StandaloneProvisioningError, StandaloneProvisioningPhase,
    StandalonePublicationError, StandalonePublicationReceipt, StandaloneRegistrationCommit,
    StandaloneRegistrationMutation, StandaloneRegistrationProvisioning,
    StandaloneRegistrationReadback, publish_committed_standalone_registration,
};
pub use search_contracts::{ProviderBindingRecord, ProviderBindingStatus};
pub use startup::{
    StandaloneBootstrapError, StandaloneBootstrapReady,
    restore_and_publish_standalone_registration,
};

use search_contracts::{BindingId, InstallationIncarnationId};
use search_control_redb::{
    ControlCallError, ControlError, ControlKey, ControlSnapshotPublisher, JournalIdentity,
    JournalLimits, PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingContext, MonotonicMillis, ProtocolError};

use crate::provider_composition::monotonic_millis;
use super::{GrantUseError, SystemGrantClock};

/// Record pinned by a successful native session open, not a reusable permit.
/// No Clone or public constructor: an arbitrary historical record cannot be
/// attached to a session. Each use must reread the published row under the
/// original journal/policy lock; any changed field requires new pairing.
pub struct NativeBindingPin {
    journal_identity: JournalIdentity,
    context: BindingContext,
    record: ProviderBindingRecord,
}

impl NativeBindingPin {
    /// Original registration metadata. Retaining this reference does not keep
    /// its lifetime or authority alive and cannot resolve disclosure by itself.
    #[must_use]
    pub const fn record(&self) -> &ProviderBindingRecord { &self.record }

    /// Authenticated binding context retained when this exact session opened.
    ///
    /// This read-only value identifies the paired protocol session for daemon
    /// composition. It is not a grant, access permit or replacement for
    /// [`NativeBindingPin::revalidate`].
    #[must_use]
    pub const fn binding_context(&self) -> BindingContext { self.context }

    /// Revalidate the exact record and originating pairing against current disk
    /// publication, then check UTC using the retained rollback-fenced clock.
    /// The optional deadline is binding expiry only; callers must intersect it
    /// with the original request/grant limits before backend work or output.
    /// Keep the actual native lock held through those operations.
    pub fn revalidate<C: CancellationProbe>(
        &self,
        binding: &BindingContext,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<Option<MonotonicMillis>, NativeBindingError> {
        let (started, deadline) = begin(context)?;
        if binding != &self.context || journal.identity() != self.journal_identity {
            return Err(NativeBindingError::Unavailable);
        }
        let current = read_published_binding(
            journal,
            publisher,
            binding.binding_id(),
            context,
        )?
        .ok_or(NativeBindingError::Unavailable)?;
        if current != self.record || current.status != ProviderBindingStatus::Active {
            return Err(NativeBindingError::Unavailable);
        }
        let expiry = clock.check_policy_window(&current.issued_at, current.expires_at.as_ref())?;
        check(context, started, deadline)?;
        Ok(expiry)
    }
}

/// Closed native errors; no peer names, profiles, credentials or record bytes.
#[derive(Debug)]
pub enum NativeBindingError {
    /// Malformed, unsupported, oversized or contradictory record/transition.
    InvalidRecord,
    /// Missing, changed, inactive, foreign or incorrectly registered peer.
    Unavailable,
    /// Cancellation, deadline overflow/expiry or a regressed native clock.
    Interrupted,
    /// Original journal identity/condition failure.
    Control(ControlError),
    /// Original native call failure, including possible commit information.
    Call(ControlCallError),
    /// Actual pairing/key/session verification failed.
    Protocol(ProtocolError),
    /// Canonical lifetime or rollback-fenced UTC validation failed.
    Clock(GrantUseError),
}

impl std::fmt::Display for NativeBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecord => formatter.write_str("DAEMON_BINDING_RECORD_INVALID"),
            Self::Unavailable => formatter.write_str("DAEMON_BINDING_UNAVAILABLE"),
            Self::Interrupted => formatter.write_str("DAEMON_BINDING_INTERRUPTED"),
            Self::Control(error) => std::fmt::Display::fmt(error, formatter),
            Self::Call(error) => std::fmt::Display::fmt(error, formatter),
            Self::Protocol(error) => formatter.write_str(error.code()),
            Self::Clock(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}
impl std::error::Error for NativeBindingError {}
impl From<ControlError> for NativeBindingError {
    fn from(error: ControlError) -> Self { Self::Control(error) }
}
impl From<ControlCallError> for NativeBindingError {
    fn from(error: ControlCallError) -> Self { Self::Call(error) }
}
impl From<ProtocolError> for NativeBindingError {
    fn from(error: ProtocolError) -> Self { Self::Protocol(error) }
}
impl From<GrantUseError> for NativeBindingError {
    fn from(error: GrantUseError) -> Self { Self::Clock(error) }
}

pub(super) fn validate_binding_record(
    record: &ProviderBindingRecord,
) -> Result<(), NativeBindingError> {
    record
        .validate_shape()
        .map_err(|_| NativeBindingError::InvalidRecord)
}

/// Inspect one exact current disk-published binding row, including terminal rows.
///
/// This compatibility reader remains for non-standalone client-adapter profiles.
/// Standalone binding/policy authority uses the atomic typed redb pair reader.
pub(super) fn read_published_binding<C: CancellationProbe>(
    journal: &PersistentControlJournal,
    publisher: &ControlSnapshotPublisher,
    binding: BindingId,
    context: &OperationContext<C>,
) -> Result<Option<ProviderBindingRecord>, NativeBindingError> {
    let (started, deadline) = begin(context)?;
    let incarnation = journal.identity().installation_incarnation_id;
    let key = binding_key(incarnation, binding)?;
    let value = journal.read_published_record(publisher, &key, context)?;
    let record = value.as_ref().map(codec::decode).transpose()?;
    if record.as_ref().is_some_and(|record| {
        record.binding_id != binding || record.installation_incarnation_id != incarnation
    }) {
        return Err(NativeBindingError::InvalidRecord);
    }
    check(context, started, deadline)?;
    Ok(record)
}

fn binding_key(
    incarnation: InstallationIncarnationId,
    binding: BindingId,
) -> Result<ControlKey, ControlError> {
    let mut key = b"eliot.control.provider-binding.v1\0".to_vec();
    key.extend_from_slice(incarnation.as_bytes());
    key.extend_from_slice(binding.as_bytes());
    ControlKey::new(key, JournalLimits::BASELINE)
}

fn begin<C: CancellationProbe>(
    context: &OperationContext<C>,
) -> Result<(MonotonicMillis, MonotonicMillis), NativeBindingError> {
    let started = monotonic_millis();
    let deadline = started
        .get()
        .checked_add(context.relative_deadline_ms().get())
        .map(MonotonicMillis::new)
        .ok_or(NativeBindingError::Interrupted)?;
    check(context, started, deadline)?;
    Ok((started, deadline))
}

fn check<C: CancellationProbe>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<(), NativeBindingError> {
    let now = monotonic_millis();
    if context.cancellation().is_cancelled() || now < started || now >= deadline {
        return Err(NativeBindingError::Interrupted);
    }
    Ok(())
}
