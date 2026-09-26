//! Ordered final publication, session drain and binding-dependent invalidation.

use core::fmt;

use search_contracts::ReceiptRef;
use search_continuation::{
    ContinuationCleanup, ContinuationEffect, ContinuationError, ContinuationStore,
};
use search_control_redb::{
    ControlCommitReceipt, ControlSnapshotPublisher, MutationId, PersistentControlJournal,
};
use search_handles::{HandleError, HandleStore};
use search_ports::{CancellationProbe, OperationContext};

use super::readback::remaining_context;
use super::{StandaloneProvisioningError, StandaloneRegistrationProvisioning};
use super::super::{
    BindingConnectionRegistry, BindingDrainReceipt, NativeBindingError, begin, check,
};

/// Failure while completing the ordered W8 registration publication barrier.
///
/// A returned error after snapshot publication never means rollback. The caller
/// must keep listener admission and the native mutation domain closed, restore the
/// same durable provisioning operation, and rerun this exact barrier. Handle and
/// continuation invalidation are monotonic; cleanup owners must be idempotent for
/// the immutable [`ContinuationCleanup`] identity.
#[derive(Debug)]
pub enum StandalonePublicationError<E> {
    /// Durable provisioning, publication, credential or readback failure.
    Provisioning(StandaloneProvisioningError),
    /// The canonical handle owner refused or could not finish its full-store pass.
    Handle(HandleError),
    /// The canonical continuation owner refused or retained unresolved cleanup.
    Continuation(ContinuationError),
    /// The injected pin/checkpoint owner failed or returned an uncertain result.
    Cleanup(E),
    /// The native commit receipt could not form the bounded cleanup identity.
    InvalidReceipt,
}

impl<E: fmt::Display> fmt::Display for StandalonePublicationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provisioning(error) => fmt::Display::fmt(error, formatter),
            Self::Handle(error) => fmt::Display::fmt(error, formatter),
            Self::Continuation(error) => fmt::Display::fmt(error, formatter),
            Self::Cleanup(error) => fmt::Display::fmt(error, formatter),
            Self::InvalidReceipt => formatter.write_str("STANDALONE_PUBLICATION_RECEIPT_INVALID"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for StandalonePublicationError<E> {}

/// Executed finalization evidence for one published standalone registration.
///
/// This content-free receipt is process-local evidence, not a reusable permit or
/// proof of remote receipt. Durable restart truth remains the control journal,
/// credential record and the idempotent dependent-owner state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandalonePublicationReceipt {
    operation_id: MutationId,
    published_generation: u64,
    drain: BindingDrainReceipt,
    handles_inspected: usize,
    handles_invalidated: usize,
    continuations_inspected: usize,
    continuations_invalidated: usize,
    continuation_cleanups: usize,
}

impl StandalonePublicationReceipt {
    /// Exact final registration operation.
    #[must_use]
    pub const fn operation_id(&self) -> MutationId { self.operation_id }

    /// Journal generation represented by the guarded published snapshot.
    #[must_use]
    pub const fn published_generation(&self) -> u64 { self.published_generation }

    /// Actual admission/request drain evidence for older registered sessions.
    #[must_use]
    pub const fn drain(&self) -> BindingDrainReceipt { self.drain }

    /// Handle records inspected by the completed pass.
    #[must_use]
    pub const fn handles_inspected(&self) -> usize { self.handles_inspected }

    /// Active handles invalidated by this attempt.
    #[must_use]
    pub const fn handles_invalidated(&self) -> usize { self.handles_invalidated }

    /// Continuation records inspected by the completed pass.
    #[must_use]
    pub const fn continuations_inspected(&self) -> usize { self.continuations_inspected }

    /// Active continuations invalidated by this attempt.
    #[must_use]
    pub const fn continuations_invalidated(&self) -> usize {
        self.continuations_invalidated
    }

    /// External pin/checkpoint cleanups acknowledged by this attempt.
    #[must_use]
    pub const fn continuation_cleanups(&self) -> usize { self.continuation_cleanups }
}

/// Publish one committed registration, fence old sessions, invalidate every
/// binding-owned handle/continuation and only then return acknowledgement.
///
/// Ordering follows the W8 revocation contract:
///
/// `durable commit -> live snapshot -> connection drain/request cancellation ->`
/// `handle/continuation invalidation and cleanup -> final readback`.
///
/// The caller must hold the actual root/binding/policy mutation lock across this
/// whole call and must not start or admit another connection until success. This
/// is particularly important after process restart: if a prior invocation may
/// have published before failing, restore the exact provisioning operation and
/// rerun this function before listener startup. No in-memory success is inferred
/// from a COMMITTED intent or historical transaction receipt.
///
/// `cleanup` must execute the exact epoch-pin release or durable checkpoint
/// deletion, or verify its prior completion. It must not widen the target, renew a
/// pin, hide outcome uncertainty or reacquire the already-held native domain lock.
/// One diminishing deadline and the original cancellation probe cover every step.
#[allow(clippy::too_many_arguments)]
pub fn publish_committed_standalone_registration<C, E, F>(
    provisioning: &StandaloneRegistrationProvisioning,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    connections: &mut BindingConnectionRegistry,
    handles: &mut HandleStore,
    continuations: &mut ContinuationStore,
    cleanup: &mut F,
    context: &OperationContext<C>,
) -> Result<StandalonePublicationReceipt, StandalonePublicationError<E>>
where
    C: CancellationProbe + Clone,
    F: FnMut(&ContinuationCleanup, &OperationContext<C>) -> Result<(), E>,
{
    let (started, deadline) = begin(context).map_err(native_error)?;
    let committed = provisioning.committed().ok_or_else(|| {
        StandalonePublicationError::Provisioning(StandaloneProvisioningError::NotCommitted)
    })?;
    let replacement = committed.replacement_binding()
        .map_err(|error| provisioning_error(error.into()))?;
    let receipt = committed.receipt().clone();
    let cleanup_identity = registration_receipt_ref(&receipt)
        .ok_or(StandalonePublicationError::InvalidReceipt)?;
    check(context, started, deadline).map_err(native_error)?;

    // Publication is the live denial point for the previous registration. Any
    // later error is fail-closed recovery work, never permission to republish old
    // state or acknowledge the administrative operation.
    let call = remaining_context(context, started, deadline).map_err(native_error)?;
    let _published = journal.publish_committed_snapshot_with_context(
        &receipt, publisher, &call,
    ).map_err(|error| provisioning_error(error.into()))?;
    let call = remaining_context(context, started, deadline).map_err(native_error)?;
    committed.confirm_published(journal, publisher, &call)
        .map_err(|error| provisioning_error(error.into()))?;

    // The monotonic signal closes admission immediately and is part of every
    // request cancellation probe. Mutable socket owners may still be delivering
    // terminal frames; that does not reopen work or block dependent invalidation.
    let drain = connections.fence_prior_generations(&replacement);
    check(context, started, deadline).map_err(native_error)?;

    let generation = replacement.revocation_generation.get();
    let mut handle_pass = handles.begin_binding_invalidation(
        replacement.binding_id, generation,
    );
    loop {
        check(context, started, deadline).map_err(native_error)?;
        let progress = handle_pass.advance().map_err(StandalonePublicationError::Handle)?;
        check(context, started, deadline).map_err(native_error)?;
        if progress.complete { break; }
        if progress.inspected == 0 {
            return Err(StandalonePublicationError::Handle(HandleError::InvalidTransition));
        }
    }
    let handle_receipt = handle_pass.finish().map_err(StandalonePublicationError::Handle)?;
    if handle_receipt.generation() != generation {
        return Err(StandalonePublicationError::Handle(HandleError::InvalidTransition));
    }

    let mut continuation_pass = continuations.begin_binding_invalidation(
        replacement.binding_id, generation, &cleanup_identity,
    );
    loop {
        check(context, started, deadline).map_err(native_error)?;
        let progress = continuation_pass.advance()
            .map_err(StandalonePublicationError::Continuation)?;
        check(context, started, deadline).map_err(native_error)?;
        if progress.cleanup_pending {
            let call = remaining_context(context, started, deadline).map_err(native_error)?;
            let mut cleanup_failure = None;
            let outcome = continuation_pass.complete_cleanup(|work| {
                if work.operation_receipt() != &cleanup_identity
                    || work.generation() != generation
                    || matches!(work.effect(), ContinuationEffect::RenewEpochPin { .. })
                {
                    return Err(ContinuationError::OperationConflict);
                }
                cleanup(work, &call).map_err(|error| {
                    cleanup_failure = Some(error);
                    ContinuationError::InvalidTransition
                })
            });
            if let Err(error) = outcome {
                if let Some(error) = cleanup_failure {
                    return Err(StandalonePublicationError::Cleanup(error));
                }
                return Err(StandalonePublicationError::Continuation(error));
            }
            check(context, started, deadline).map_err(native_error)?;
        }
        if progress.complete { break; }
        if progress.inspected == 0 && !progress.cleanup_pending {
            return Err(StandalonePublicationError::Continuation(
                ContinuationError::InvalidTransition,
            ));
        }
    }
    let continuation_receipt = continuation_pass.finish()
        .map_err(StandalonePublicationError::Continuation)?;
    if continuation_receipt.generation() != generation {
        return Err(StandalonePublicationError::Continuation(
            ContinuationError::OperationConflict,
        ));
    }

    // Bracket the completed dependents with the existing exact credential and
    // three-row publication checks. A changed credential or metadata head denies
    // acknowledgement even though earlier invalidations remain applied.
    let call = remaining_context(context, started, deadline).map_err(native_error)?;
    provisioning.confirm_published(journal, publisher, &call)
        .map_err(StandalonePublicationError::Provisioning)?;
    check(context, started, deadline).map_err(native_error)?;

    Ok(StandalonePublicationReceipt {
        operation_id: receipt.operation_id,
        published_generation: receipt.after_generation,
        drain,
        handles_inspected: handle_receipt.inspected(),
        handles_invalidated: handle_receipt.invalidated(),
        continuations_inspected: continuation_receipt.inspected(),
        continuations_invalidated: continuation_receipt.invalidated(),
        continuation_cleanups: continuation_receipt.cleanups(),
    })
}

fn native_error<E>(error: NativeBindingError) -> StandalonePublicationError<E> {
    StandalonePublicationError::Provisioning(error.into())
}

fn provisioning_error<E>(error: StandaloneProvisioningError) -> StandalonePublicationError<E> {
    StandalonePublicationError::Provisioning(error)
}

fn registration_receipt_ref(receipt: &ControlCommitReceipt) -> Option<ReceiptRef> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::from("control-binding-v1:");
    for byte in receipt.operation_id.0 {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 15)]));
    }
    value.push(':');
    value.push_str(&receipt.after_generation.to_string());
    ReceiptRef::new(value).ok()
}
