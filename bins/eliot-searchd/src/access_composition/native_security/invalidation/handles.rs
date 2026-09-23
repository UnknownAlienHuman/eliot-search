//! Executed canonical handle invalidation for one committed restriction.

use search_access::AccessError;
use search_contracts::ReceiptRef;
use search_control_redb::security_restriction::SecurityRestrictionCommit;
use search_handles::HandleStore;
use search_ports::CancellationProbe;

use super::{HANDLE_SECURITY_DEPENDENT, MutationBudget, access_error};

pub(super) fn invalidate<C: CancellationProbe + Clone>(
    store: &mut HandleStore,
    committed: &SecurityRestrictionCommit,
    mutation_receipt: &ReceiptRef,
    budget: &MutationBudget<'_, C>,
) -> Result<ReceiptRef, AccessError> {
    let state = committed.mutation().replacement();
    // Prepare the bounded reference before touching records. It addresses this
    // process-local owner's completed pass for a real native ledger operation;
    // it does not assert durable handle deletion or retention-lease release.
    // Replaying an already-applied restriction keeps the same logical reference.
    let reference = ReceiptRef::new(format!(
        "security-dependent-v1:{HANDLE_SECURITY_DEPENDENT}:{}",
        mutation_receipt.as_str(),
    )).map_err(|_| AccessError::SecurityFailClosed)?;
    budget.check().map_err(access_error)?;
    let mut pass = store.begin_security_invalidation(
        &state.denied_memberships,
        &state.purged_memberships,
        state.policy.live_deny_generation,
    );
    loop {
        budget.check().map_err(access_error)?;
        let progress = pass.advance().map_err(|_| AccessError::SecurityFailClosed)?;
        budget.check().map_err(access_error)?;
        if progress.complete {
            break;
        }
        if progress.inspected == 0 {
            return Err(AccessError::SecurityFailClosed);
        }
    }
    let receipt = pass.finish().map_err(|_| AccessError::SecurityFailClosed)?;
    if receipt.generation() != state.policy.live_deny_generation {
        return Err(AccessError::SecurityOperationConflict);
    }
    budget.check().map_err(access_error)?;
    // This return is reachable only after the actual owner has checked every
    // record. Cancellation/error after a prefix returns no acknowledgement;
    // NativeSecurityDomain retains its pending restriction and denies access.
    Ok(reference)
}
