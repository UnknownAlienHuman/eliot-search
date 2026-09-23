//! Transfer continuous snapshot protection into an unpublished continuation.

use std::sync::Arc;

use search_contracts::{ContinuationId, OpaqueId, OpaqueRef};

use super::{BoundPin, ContinuationPins};
use crate::{EpochPinGuard, PinError, PinKind, PinRecord, PinReleaseReceipt};

impl ContinuationPins {
    /// Holds a real new pin while its caller creates and delivers a continuation.
    ///
    /// The original query guard must still exist in this exact registry. Its
    /// validation and acquisition of the continuation pin share one registry
    /// lock: an old epoch is never pinned again after losing its protection.
    /// The query guard is borrowed, not consumed or renewed.
    ///
    /// Return `Ok` from `publish` only after the continuation owner has accepted
    /// the exact reference and completed delivery under its output barrier.
    /// Errors or unwinding release only the unpublished pin. A failed release
    /// remains marked abandoned, counts toward capacity, and can be retried with
    /// `release_abandoned`; it can never validate as a usable continuation pin.
    /// This method does not implement transport, grant validation or a clock.
    #[allow(clippy::too_many_arguments)]
    pub fn with_query_pin<T, E>(
        &mut self,
        continuation_id: ContinuationId,
        owner: OpaqueId,
        query: &EpochPinGuard,
        now_ms: u64,
        expires_at_ms: u64,
        publish: impl FnOnce(&OpaqueRef, &Self) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<PinError>,
    {
        let reference = self.acquire_from_query(
            continuation_id, owner, query, now_ms, expires_at_ms,
        )?;
        let mut pending = UnpublishedPin {
            pins: self,
            continuation_id,
            reference,
            published: false,
        };
        let output = publish(&pending.reference, pending.pins)?;
        pending.published = true;
        Ok(output)
    }

    /// Retries checked releases left by failed/unwound unpublished creation.
    ///
    /// Only abandoned bindings are selected, never published or security-cleanup
    /// evidence. The entire inventory is bounded by the owner's retained-record
    /// limit; at most `max_items` releases are attempted. An error can follow a
    /// released prefix, but every unfinished binding remains marked for retry.
    /// No tokens or source data are retained in these abandoned entries.
    pub fn release_abandoned(&mut self, max_items: usize) -> Result<PinReleaseReceipt, PinError> {
        if max_items == 0 || max_items > self.max_records {
            return Err(PinError::InvalidLimits);
        }
        let ids = self.records.iter()
            .filter(|(_, binding)| binding.abandoned)
            .map(|(id, _)| *id)
            .take(max_items)
            .collect::<Vec<_>>();
        let mut released_pins = 0;
        for id in ids {
            let binding = self.records.get_mut(&id).ok_or(PinError::PinNotFound)?;
            released_pins += binding.guard.try_release()?.released_pins;
            self.records.remove(&id);
        }
        Ok(PinReleaseReceipt { released_pins })
    }

    fn acquire_from_query(
        &mut self,
        continuation_id: ContinuationId,
        owner: OpaqueId,
        query: &EpochPinGuard,
        now_ms: u64,
        expires_at_ms: u64,
    ) -> Result<OpaqueRef, PinError> {
        if self.records.contains_key(&continuation_id) {
            return Err(PinError::OwnerMismatch);
        }
        if self.records.len() >= self.max_records {
            return Err(PinError::RegistryCapacityExceeded);
        }
        if query.released {
            return Err(PinError::PinNotFound);
        }
        let registry = query.registry.upgrade().ok_or(PinError::PinNotFound)?;
        if !Arc::ptr_eq(&registry, &self.registry.inner) {
            return Err(PinError::OwnerMismatch);
        }
        let next = self.next_reference.checked_add(1).ok_or(PinError::PinIdExhausted)?;
        let reference = OpaqueRef::new(format!(
            "continuation-pin-v1:{}:{}:{}",
            self.instance_id.as_str().len(), self.instance_id.as_str(), self.next_reference,
        )).map_err(|_| PinError::InvalidLimits)?;
        let mut inner = registry.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let original = inner.pins.get(&query.pin_id).ok_or(PinError::PinNotFound)?;
        if original.owner != query.owner || original.route != query.route
            || original.epoch != Some(query.epoch) || original.kind != PinKind::QueryEpoch
        {
            return Err(PinError::OwnerMismatch);
        }
        if inner.owner_counts.get(&query.owner).copied().unwrap_or(0) == 0 {
            return Err(PinError::RegistryPoisoned);
        }
        if query.route != inner.active_route {
            return Err(PinError::RouteNotActive);
        }
        if query.epoch > inner.visible_epoch {
            return Err(PinError::EpochNotVisible);
        }
        if expires_at_ms <= now_ms
            || expires_at_ms - now_ms > inner.limits.max_continuation_ttl_ms
        {
            return Err(PinError::InvalidExpiry);
        }
        let pin_id = inner.insert(PinRecord {
            owner: owner.clone(),
            route: query.route,
            epoch: Some(query.epoch),
            kind: PinKind::ContinuationEpoch { expires_at_ms },
        })?;
        // Construct the RAII owner before releasing the registry lock.
        let guard = EpochPinGuard {
            registry: Arc::downgrade(&registry), pin_id, owner,
            route: query.route, epoch: query.epoch, released: false,
        };
        drop(inner);
        self.next_reference = next;
        self.records.insert(continuation_id, BoundPin {
            reference: reference.clone(), guard, created_at_ms: now_ms,
            expires_at_ms, abandoned: false,
        });
        Ok(reference)
    }
}

struct UnpublishedPin<'a> {
    pins: &'a mut ContinuationPins,
    continuation_id: ContinuationId,
    reference: OpaqueRef,
    published: bool,
}

impl Drop for UnpublishedPin<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Some(binding) = self.pins.records.get_mut(&self.continuation_id) {
            // No mutable owner is exposed to the callback, so this is still the
            // exact binding installed above. Mark it before entering the registry.
            binding.abandoned = true;
        }
        if self.pins.release(self.continuation_id, &self.reference).is_ok() {
            // Checked release already established success. Retain on any error;
            // never turn a poisoned registry into apparent cleanup completion.
            let _ = self.pins.retire_released(self.continuation_id, &self.reference);
        }
    }
}
