//! Canonical creation and expansion using actual, continuously held epoch pins.

use search_contracts::{
    Blake3Digest32, BoundedList, ContinuationId, ContinuationRecord,
    HandleTokenDigest, MAX_LIST_ITEMS, OpaqueId, OpaqueRef, ResultFence, UtcTimestamp,
};
use search_epoch_pins::{ContinuationPins, EpochPinGuard, PinError, RouteIdentity};

use crate::{
    ContinuationCredential, ContinuationError, ContinuationPayload, ContinuationPermit,
    ContinuationSecurityScope, ContinuationStore, CreateContinuationRequest,
    CreatedContinuation, LiveContinuationState, ResumePlan,
};

impl ContinuationStore {
    /// Creates and delivers an ephemeral continuation while continuously pinned.
    ///
    /// `query` must still protect the exact indexed snapshot in `result_fence`.
    /// The same registry checks that guard while acquiring the continuation pin.
    /// `assemble` receives the REAL new pin reference, not a placeholder. Its
    /// request must retain the supplied continuation ID, fence and expiry; all
    /// existing creation validation and quotas still apply.
    ///
    /// `clock` supplies fresh trusted UTC and monotonic milliseconds from one
    /// process clock domain. The pin deadline is fixed once from the remaining
    /// UTC lifetime, rounded DOWN. Less than one millisecond is refused. Later
    /// observations can invalidate the token but can never extend that deadline.
    ///
    /// `deliver` runs under exclusive store/pin-owner borrows. It must recheck
    /// current grants, cancellation, expiry and actual output completion, using
    /// the supplied clock and pin owner as needed at each write boundary. The
    /// caller must hold its security/output barrier throughout this method.
    /// Any output error after bytes may have escaped requires session shutdown;
    /// removing the token cannot undo already-written bytes.
    ///
    /// Failure or unwind removes only this new record and token-index entry and
    /// abandons its pin for checked release. A failed release remains recoverable
    /// through `ContinuationPins::release_abandoned`. No existing record, token,
    /// issued history or query guard is changed. Success retains the new binding
    /// for the normal canonical lifecycle; it does not consume the query guard.
    #[allow(clippy::too_many_arguments)]
    pub fn create_pinned_ephemeral<F, E>(
        &mut self,
        pins: &mut ContinuationPins,
        continuation_id: ContinuationId,
        pin_owner: OpaqueId,
        query: &EpochPinGuard,
        result_fence: &ResultFence,
        expires_at: &UtcTimestamp,
        scope: ContinuationSecurityScope,
        clock: &mut F,
        assemble: impl FnOnce(OpaqueRef) -> Result<CreateContinuationRequest, E>,
        deliver: impl FnOnce(&CreatedContinuation, &ContinuationPins, &mut F) -> Result<(), E>,
    ) -> Result<CreatedContinuation, E>
    where
        F: FnMut() -> Result<(UtcTimestamp, u64), E>,
        E: From<ContinuationError> + From<PinError>,
    {
        let (route, epoch) = indexed_route(result_fence)?;
        if query.route() != route || query.epoch() != epoch {
            return Err(ContinuationError::SnapshotExpired.into());
        }
        if self.records.contains_key(&continuation_id) {
            return Err(ContinuationError::IdentityCollision.into());
        }
        let (started_at, started_ms) = clock()?;
        let remaining_ms = crate::lifetime::duration_micros(&started_at, expires_at)
            .ok_or(ContinuationError::SnapshotExpired)? / 1_000;
        if remaining_ms == 0 {
            return Err(ContinuationError::SnapshotExpired.into());
        }
        let pin_expires_ms = started_ms.checked_add(remaining_ms)
            .ok_or(ContinuationError::InvalidTtl)?;
        pins.with_query_pin(
            continuation_id, pin_owner, query, started_ms, pin_expires_ms,
            |reference, pins| {
                let request = assemble(reference.clone())?;
                let ContinuationRecord::EphemeralWindow(record) = &request.record else {
                    return Err(ContinuationError::DurabilityMismatch.into());
                };
                if !matches!(&request.payload, ContinuationPayload::Ephemeral { .. }) {
                    return Err(ContinuationError::DurabilityMismatch.into());
                }
                if record.continuation_id != continuation_id
                    || &record.epoch_pin_ref != reference
                    || &record.result_fence != result_fence
                    || &record.expires_at != expires_at
                {
                    return Err(ContinuationError::SnapshotExpired.into());
                }
                let (prepared_at, prepared_ms) = clock()?;
                if prepared_at < record.created_at || prepared_at >= record.expires_at {
                    return Err(ContinuationError::SnapshotExpired.into());
                }
                check_pin(&request.record, pins, prepared_ms)?;
                let digest = record.token_digest;
                let created = self.create_with_security_scope(request, scope)?;
                // No fallible work is allowed between successful insertion and
                // installing this exact rollback guard. The callback cannot
                // access the borrowed store or replace the newly inserted row.
                let mut unpublished = UnpublishedRecord {
                    store: self, id: continuation_id, digest, accepted: false,
                };
                check_created(&created, pins, clock)?;
                deliver(&created, pins, clock)?;
                check_created(&created, pins, clock)?;
                unpublished.accepted = true;
                Ok(created)
            },
        )
    }

    /// Resumes an ephemeral continuation only while its original pin still exists.
    ///
    /// Current credential/security failures take precedence. A caller-supplied
    /// `epoch_pin_valid = true` cannot bypass the actual registry check. Expired
    /// or released pins cause refusal, never renewal/replacement. This does not
    /// support a DIRECT record without an indexed route or a durable replan.
    pub fn resume_pinned(
        &self,
        credential: &ContinuationCredential,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
        now_ms: u64,
        max_items: usize,
        pins: &ContinuationPins,
    ) -> Result<ResumePlan, ContinuationError> {
        if max_items == 0 || max_items > self.limits.max_expansion_items {
            return Err(ContinuationError::InvalidLimits);
        }
        let stored = self.authorized(credential)?;
        Self::revalidate(stored, live, now)?;
        check_pin(&stored.record, pins, now_ms)?;
        self.resume(credential, live, now, max_items)
    }

    /// Checks the bound page, current authority and actual pin before output.
    /// Nothing is marked issued here. Hold the existing security/output barrier
    /// and use the original `commit_emission` only after successful delivery.
    pub fn revalidate_pinned_emission(
        &self,
        permit: &ContinuationPermit,
        emitted: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
        now_ms: u64,
        pins: &ContinuationPins,
    ) -> Result<(), ContinuationError> {
        self.revalidate_emission(permit, emitted, live, now)?;
        let stored = self.records.get(&permit.continuation_id())
            .ok_or(ContinuationError::StalePermit)?;
        check_pin(&stored.record, pins, now_ms)
    }
}

fn indexed_route(fence: &ResultFence) -> Result<(RouteIdentity, search_contracts::Epoch), ContinuationError> {
    let snapshot = &fence.planned_snapshot;
    snapshot.validate().map_err(|_| ContinuationError::SnapshotExpired)?;
    let collection_generation_id = snapshot.collection_generation_id
        .ok_or(ContinuationError::EpochPinUnavailable)?;
    let epoch = snapshot.visible_epoch.ok_or(ContinuationError::EpochPinUnavailable)?;
    Ok((RouteIdentity {
        collection_generation_id,
        route_revision: snapshot.collection_route_revision,
    }, epoch))
}

fn check_pin(
    record: &ContinuationRecord,
    pins: &ContinuationPins,
    now_ms: u64,
) -> Result<(), ContinuationError> {
    let ContinuationRecord::EphemeralWindow(record) = record else {
        return Err(ContinuationError::DurabilityMismatch);
    };
    let (route, epoch) = indexed_route(&record.result_fence)?;
    pins.validate(record.continuation_id, &record.epoch_pin_ref, route, epoch, now_ms)
        .map_err(|_| ContinuationError::EpochPinUnavailable)
}

fn check_created<F, E>(
    created: &CreatedContinuation,
    pins: &ContinuationPins,
    clock: &mut F,
) -> Result<(), E>
where
    F: FnMut() -> Result<(UtcTimestamp, u64), E>,
    E: From<ContinuationError>,
{
    let (now, now_ms) = clock()?;
    let ContinuationRecord::EphemeralWindow(record) = &created.record else {
        return Err(ContinuationError::DurabilityMismatch.into());
    };
    if now < record.created_at || now >= record.expires_at {
        return Err(ContinuationError::SnapshotExpired.into());
    }
    check_pin(&created.record, pins, now_ms).map_err(Into::into)
}

struct UnpublishedRecord<'a> {
    store: &'a mut ContinuationStore,
    id: ContinuationId,
    digest: HandleTokenDigest,
    accepted: bool,
}

impl Drop for UnpublishedRecord<'_> {
    fn drop(&mut self) {
        if !self.accepted {
            // This unique borrow spans insertion through output; neither ID
            // can have been replaced or disclosed through a concurrent resume.
            // No revision allocation or external effect is needed to unpublish.
            self.store.records.remove(&self.id);
            self.store.token_index.remove(&self.digest);
        }
    }
}
