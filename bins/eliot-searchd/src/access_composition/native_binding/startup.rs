//! Restart-safe standalone registration bootstrap before listener admission.
//!
//! This owner reconstructs one exact durable provisioning command, resolves each
//! uncertain effect through its existing readback path, executes guarded final
//! publication/dependent invalidation and returns the only evidence accepted by
//! native TCP opening. It creates no listener, credential, policy or authority.

use core::fmt;

use search_contracts::BindingId;
use search_continuation::{ContinuationCleanup, ContinuationStore};
use search_control_redb::{
    ControlSnapshotPublisher, JournalIdentity, MutationId, PersistentControlJournal,
};
use search_handles::HandleStore;
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingContext, MonotonicMillis};

use super::{
    BindingConnectionRegistry, NativeBindingError, NativeBindingExpectation,
    ProviderBindingRecord, ProviderBindingStatus, StandaloneProvisioningError,
    StandaloneProvisioningPhase, StandalonePublicationError, StandalonePublicationReceipt,
    StandaloneRegistrationProvisioning, begin, check, monotonic_millis,
    publish_committed_standalone_registration,
};

/// Closed bootstrap failure. A failure after any possible write or publication
/// never means rollback; listener admission remains unavailable until the same
/// durable operation is restored and completed.
#[derive(Debug)]
pub enum StandaloneBootstrapError<E> {
    /// Durable intent, credential or final registration recovery failed.
    Provisioning(StandaloneProvisioningError),
    /// Publication, drain, invalidation or resource cleanup failed.
    Publication(StandalonePublicationError<E>),
    /// A recovery phase made no progress under the immutable operation.
    Stalled,
}

impl<E: fmt::Display> fmt::Display for StandaloneBootstrapError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provisioning(error) => fmt::Display::fmt(error, formatter),
            Self::Publication(error) => fmt::Display::fmt(error, formatter),
            Self::Stalled => formatter.write_str("STANDALONE_BOOTSTRAP_STALLED"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for StandaloneBootstrapError<E> {}

/// Non-clonable evidence that one exact native registration completed startup
/// recovery, guarded publication, old-session fencing and dependent cleanup.
///
/// This carries no key, socket, grant or source permit. Native TCP opening still
/// rereads the current binding/policy and credential under the root lock, but it
/// additionally requires this value so a caller cannot bypass startup finalization.
#[must_use = "retain while accepting native connections for this registration"]
pub struct StandaloneBootstrapReady {
    journal_identity: JournalIdentity,
    expected: NativeBindingExpectation,
    record: ProviderBindingRecord,
    receipt: StandalonePublicationReceipt,
}

impl StandaloneBootstrapReady {
    /// Content-free executed finalization receipt.
    #[must_use]
    pub const fn receipt(&self) -> &StandalonePublicationReceipt { &self.receipt }

    /// Exact replacement registration represented by this startup evidence.
    /// It is metadata for diagnostics/composition, never a reusable permit.
    #[must_use]
    pub const fn record(&self) -> &ProviderBindingRecord { &self.record }

    pub(in crate::access_composition) const fn expectation(&self) -> &NativeBindingExpectation {
        &self.expected
    }

    pub(in crate::access_composition) fn validate_context(
        &self,
        journal: &PersistentControlJournal,
        binding: &BindingContext,
    ) -> Result<(), NativeBindingError> {
        if journal.identity() != self.journal_identity
            || self.record.status != ProviderBindingStatus::Active
            || binding.binding_id() != self.record.binding_id
            || binding.incarnation() != self.record.installation_incarnation_id
            || binding.role() != self.record.peer_role
            || self.expected.pairing_generation != self.record.pairing_generation
        {
            return Err(NativeBindingError::Unavailable);
        }
        Ok(())
    }

    pub(in crate::access_composition) fn validate_opened_record(
        &self,
        journal: &PersistentControlJournal,
        record: &ProviderBindingRecord,
    ) -> Result<(), NativeBindingError> {
        let drain = self.receipt.drain();
        if journal.identity() != self.journal_identity
            || record != &self.record
            || drain.binding_id != record.binding_id
            || drain.pairing_generation != record.pairing_generation
            || drain.revocation_generation != record.revocation_generation
        {
            return Err(NativeBindingError::Unavailable);
        }
        Ok(())
    }
}

impl fmt::Debug for StandaloneBootstrapReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneBootstrapReady")
            .field("operation_id", &self.receipt.operation_id())
            .field("published_generation", &self.receipt.published_generation())
            .field("binding_status", &self.record.status)
            .finish_non_exhaustive()
    }
}

/// Restore and finish one exact standalone registration before listener admission.
///
/// The administrative owner supplies the durable binding/operation identities and
/// independently resolved native expectation while holding the actual root/policy
/// mutation lock. The original context contributes one diminishing deadline and
/// cancellation probe to every recovery, credential, publication, invalidation
/// and cleanup step. No stage receives a refreshed budget.
///
/// PREPARED intent may resume credential/final registration writes only after its
/// existing recovery path proves prior uncertain effects. COMMITTED intent is
/// never treated as published or safe by itself. The final returned value is bound
/// to the exact journal identity, replacement record and executed publication
/// receipt; native TCP opening rejects any mismatch.
#[allow(clippy::too_many_arguments)]
pub fn restore_and_publish_standalone_registration<C, E, F>(
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    connections: &mut BindingConnectionRegistry,
    handles: &mut HandleStore,
    continuations: &mut ContinuationStore,
    cleanup: &mut F,
    binding_id: BindingId,
    operation_id: MutationId,
    expected: NativeBindingExpectation,
    context: &OperationContext<C>,
) -> Result<StandaloneBootstrapReady, StandaloneBootstrapError<E>>
where
    C: CancellationProbe + Clone,
    F: FnMut(&ContinuationCleanup, &OperationContext<C>) -> Result<(), E>,
{
    let (started, deadline) = begin(context).map_err(provisioning_native)?;
    let call = remaining_context(context, started, deadline).map_err(provisioning_native)?;
    let mut provisioning = StandaloneRegistrationProvisioning::restore_durable(
        journal,
        binding_id,
        operation_id,
        expected.clone(),
        &call,
    ).map_err(StandaloneBootstrapError::Provisioning)?;

    // Eight is above the finite phase count. A repeated phase is a contradiction,
    // not permission to spin, retry an uncertain write or refresh the deadline.
    for _ in 0..8 {
        let phase = provisioning.phase();
        if phase == StandaloneProvisioningPhase::Committed {
            break;
        }
        match phase {
            StandaloneProvisioningPhase::IntentPrepared
            | StandaloneProvisioningPhase::IntentDurable
            | StandaloneProvisioningPhase::CredentialReady => {
                let call = remaining_context(context, started, deadline)
                    .map_err(provisioning_native)?;
                let _ = provisioning.commit(journal, publisher, &call)
                    .map_err(StandaloneBootstrapError::Provisioning)?;
            }
            StandaloneProvisioningPhase::IntentCommitUnresolved
            | StandaloneProvisioningPhase::IntentPublicationUnresolved
            | StandaloneProvisioningPhase::CredentialUnresolved
            | StandaloneProvisioningPhase::RegistrationUnresolved => {
                let call = remaining_context(context, started, deadline)
                    .map_err(provisioning_native)?;
                let next = provisioning.recover(journal, publisher, &call)
                    .map_err(StandaloneBootstrapError::Provisioning)?;
                if next == phase {
                    return Err(StandaloneBootstrapError::Stalled);
                }
            }
            StandaloneProvisioningPhase::Committed => unreachable!(),
        }
    }
    if provisioning.phase() != StandaloneProvisioningPhase::Committed {
        return Err(StandaloneBootstrapError::Stalled);
    }

    let replacement = provisioning.committed()
        .ok_or(StandaloneBootstrapError::Provisioning(
            StandaloneProvisioningError::NotCommitted,
        ))?
        .replacement_binding()
        .map_err(StandaloneProvisioningError::from)
        .map_err(StandaloneBootstrapError::Provisioning)?;
    if replacement.binding_id != binding_id {
        return Err(StandaloneBootstrapError::Provisioning(
            NativeBindingError::Unavailable.into(),
        ));
    }

    let call = remaining_context(context, started, deadline).map_err(provisioning_native)?;
    let receipt = publish_committed_standalone_registration(
        &provisioning,
        journal,
        publisher,
        connections,
        handles,
        continuations,
        cleanup,
        &call,
    ).map_err(StandaloneBootstrapError::Publication)?;
    check(context, started, deadline).map_err(provisioning_native)?;
    if receipt.operation_id() != operation_id {
        return Err(StandaloneBootstrapError::Stalled);
    }

    Ok(StandaloneBootstrapReady {
        journal_identity: journal.identity(),
        expected,
        record: replacement,
        receipt,
    })
}

fn provisioning_native<E>(error: NativeBindingError) -> StandaloneBootstrapError<E> {
    StandaloneBootstrapError::Provisioning(error.into())
}

fn remaining_context<C: CancellationProbe + Clone>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<OperationContext<C>, NativeBindingError> {
    check(context, started, deadline)?;
    let remaining = deadline.get().checked_sub(monotonic_millis().get())
        .filter(|left| *left > 0)
        .ok_or(NativeBindingError::Interrupted)?;
    OperationContext::new(
        context.request_id(),
        remaining,
        context.cancellation().clone(),
        context.budget_ref().clone(),
    ).map_err(|_| NativeBindingError::Interrupted)
}
