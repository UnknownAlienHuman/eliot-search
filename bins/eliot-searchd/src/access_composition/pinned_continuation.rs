//! Pinned continuation creation inside the existing native security domain.

use search_access::{AccessCheckpoint, AccessError, RequestSecurityFence};
use search_contracts::{
    BoundedSet, ContinuationId, OpaqueId, OpaqueRef, ResultFence, UtcTimestamp,
};
use search_continuation::{
    ContinuationError, ContinuationSecurityScope, ContinuationStore,
    CreateContinuationRequest, CreatedContinuation,
};
use search_epoch_pins::{ContinuationPins, EpochPinGuard, PinError};

use super::NativeSecurityDomain;

impl NativeSecurityDomain {
    /// Creates a canonical pinned continuation under the same domain as output.
    ///
    /// `influence` is the complete authenticated planner population, not a client
    /// scope or the displayed hits. It is used both for the live checkpoint and
    /// for later whole-window security invalidation. The result must name the
    /// current emission generation; neither its fence nor the grant is refreshed
    /// implicitly. The query guard must protect the exact planned route/epoch.
    ///
    /// This shared domain borrow spans pin acquisition, canonical insertion and
    /// the complete output callback. The outer runtime domain lock must cover
    /// the call. The callback must independently enforce current grant expiry,
    /// cancellation and per-write deadlines and must not reacquire that lock.
    /// On any failure after output may have begun, terminate the session rather
    /// than append an error or process the next request. Clock and transport are
    /// real injected host operations; this method supplies no successful stub.
    #[allow(clippy::too_many_arguments)]
    pub fn create_pinned_continuation<F, E>(
        &self,
        store: &mut ContinuationStore,
        pins: &mut ContinuationPins,
        continuation_id: ContinuationId,
        pin_owner: OpaqueId,
        query: &EpochPinGuard,
        influence: &RequestSecurityFence,
        result_fence: &ResultFence,
        expires_at: &UtcTimestamp,
        clock: &mut F,
        assemble: impl FnOnce(OpaqueRef) -> Result<CreateContinuationRequest, E>,
        deliver: impl FnOnce(&CreatedContinuation, &ContinuationPins, &mut F) -> Result<(), E>,
    ) -> Result<CreatedContinuation, E>
    where
        F: FnMut() -> Result<(UtcTimestamp, u64), E>,
        E: From<AccessError> + From<ContinuationError> + From<PinError>,
    {
        self.with_live_checkpoint(influence, AccessCheckpoint::BeforeResultEmission, |permit| {
            if result_fence.emission_security_fence.live_deny_generation != permit.live_generation {
                return Err(AccessError::SecurityFenceStale.into());
            }
            let memberships = BoundedSet::from_items(influence.memberships.iter().copied())
                .map_err(|_| AccessError::SecurityFailClosed)?;
            let scope = ContinuationSecurityScope::new(memberships)?;
            store.create_pinned_ephemeral(
                pins, continuation_id, pin_owner, query, result_fence,
                expires_at, scope, clock, assemble, deliver,
            )
        }).map_err(E::from)?
    }
}
