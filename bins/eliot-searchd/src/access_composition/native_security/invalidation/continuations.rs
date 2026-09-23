//! Whole-continuation invalidation; acknowledgement includes resource cleanup.

use search_access::AccessError;
use search_contracts::ReceiptRef;
use search_continuation::{ContinuationCleanup, ContinuationEffect, ContinuationError, ContinuationStore};
use search_control_redb::security_restriction::SecurityRestrictionCommit;
use search_ports::{CancellationProbe, OperationContext};

use super::{
    CONTINUATION_SECURITY_DEPENDENT, MutationBudget, access_error,
};

pub(super) fn invalidate<C: CancellationProbe + Clone>(
    store: &mut ContinuationStore,
    cleanup: &mut dyn FnMut(
        &SecurityRestrictionCommit,
        &ContinuationCleanup,
        &OperationContext<C>,
    ) -> Result<(), AccessError>,
    committed: &SecurityRestrictionCommit,
    mutation_receipt: &ReceiptRef,
    budget: &MutationBudget<'_, C>,
) -> Result<ReceiptRef, AccessError> {
    let state = committed.mutation().replacement();
    let reference = ReceiptRef::new(format!(
        "security-dependent-v1:{CONTINUATION_SECURITY_DEPENDENT}:{}",
        mutation_receipt.as_str(),
    )).map_err(|_| AccessError::SecurityFailClosed)?;
    budget.check().map_err(access_error)?;
    let mut pass = store.begin_security_invalidation(
        &state.denied_memberships,
        &state.purged_memberships,
        state.policy.live_deny_generation,
        mutation_receipt,
    );
    loop {
        budget.check().map_err(access_error)?;
        let progress = pass.advance().map_err(continuation_error)?;
        budget.check().map_err(access_error)?;
        if progress.cleanup_pending {
            let mut failure = None;
            let outcome = pass.complete_cleanup(|work| {
                execute_cleanup(cleanup, committed, mutation_receipt, work, budget)
                    .map_err(|error| {
                        failure = Some(error);
                        ContinuationError::InvalidTransition
                    })
            });
            if let Err(error) = outcome {
                return Err(failure.unwrap_or_else(|| continuation_error(error)));
            }
        }
        if progress.complete { break; }
        if progress.inspected == 0 {
            return Err(AccessError::SecurityFailClosed);
        }
    }
    let receipt = pass.finish().map_err(continuation_error)?;
    if receipt.generation() != state.policy.live_deny_generation {
        return Err(AccessError::SecurityOperationConflict);
    }
    budget.check().map_err(access_error)?;
    Ok(reference)
}

fn execute_cleanup<C: CancellationProbe + Clone>(
    cleanup: &mut dyn FnMut(
        &SecurityRestrictionCommit,
        &ContinuationCleanup,
        &OperationContext<C>,
    ) -> Result<(), AccessError>,
    committed: &SecurityRestrictionCommit,
    mutation_receipt: &ReceiptRef,
    work: &ContinuationCleanup,
    budget: &MutationBudget<'_, C>,
) -> Result<(), AccessError> {
    if work.operation_receipt() != mutation_receipt
        || work.generation() != committed.mutation().replacement().policy.live_deny_generation
        || matches!(work.effect(), ContinuationEffect::RenewEpochPin { .. })
    {
        return Err(AccessError::SecurityOperationConflict);
    }
    budget.check().map_err(access_error)?;
    cleanup(committed, work, &budget.context().map_err(access_error)?)?;
    // A late/uncertain external result does not erase the retained obligation.
    // The same operation retries the immutable target through its real owner.
    budget.check().map_err(access_error)
}

const fn continuation_error(error: ContinuationError) -> AccessError {
    match error {
        ContinuationError::OperationConflict => AccessError::SecurityOperationConflict,
        _ => AccessError::SecurityFailClosed,
    }
}
