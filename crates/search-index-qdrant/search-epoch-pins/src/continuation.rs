//! Exact continuation-to-guard bindings in the existing process-local pin owner.

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{ContinuationId, Epoch, MAX_LIST_ITEMS, OpaqueId, OpaqueRef};

use crate::{EpochPinGuard, EpochPinPurpose, PinError, PinKind, PinRegistry, PinReleaseReceipt, RouteIdentity};

impl EpochPinGuard {
    /// Releases this exact pin, retaining the guard on a registry failure.
    ///
    /// Unlike the best-effort consuming `release`/Drop path, lock poisoning is
    /// an error, not an apparent zero-count success. A prior successful release,
    /// verified absence after owner/expiry cleanup, or destruction of the whole
    /// registry is idempotent success. No other owner's pins are removed.
    pub fn try_release(&mut self) -> Result<PinReleaseReceipt, PinError> {
        if self.released {
            return Ok(PinReleaseReceipt { released_pins: 0 });
        }
        let Some(registry) = self.registry.upgrade() else {
            self.released = true;
            return Ok(PinReleaseReceipt { released_pins: 0 });
        };
        let mut inner = registry.lock().map_err(|_| PinError::RegistryPoisoned)?;
        if let Some(record) = inner.pins.get(&self.pin_id) {
            if record.owner != self.owner || record.route != self.route || record.epoch != Some(self.epoch) {
                return Err(PinError::OwnerMismatch);
            }
            if inner.owner_counts.get(&self.owner).copied().unwrap_or(0) == 0 {
                return Err(PinError::RegistryPoisoned);
            }
        }
        let removed = inner.remove(self.pin_id);
        self.released = true;
        Ok(PinReleaseReceipt { released_pins: usize::from(removed) })
    }
}

struct BoundPin {
    reference: OpaqueRef,
    guard: EpochPinGuard,
    created_at_ms: u64,
    expires_at_ms: u64,
}

/// Finite RAII guard bindings for canonical ephemeral continuations.
///
/// The supplied registry is the same registry used for reclamation, not a new
/// pin database. One non-clonable owner holds the actual guards. The caller
/// supplies a fresh qualified instance identity for each owner construction;
/// it must never restore this object or its pins from a previous process.
/// References are generated once and never parsed into guessed pin identities.
/// They remain server-owned, not authorization and not public bearer tokens.
///
/// Released entries remain until explicit retirement so a timeout after actual
/// release can retry the same binding. Both live and released entries count
/// toward the finite cap. No automatic expiry can erase retry evidence.
pub struct ContinuationPins {
    registry: PinRegistry,
    instance_id: OpaqueId,
    max_records: usize,
    next_reference: u64,
    records: BTreeMap<ContinuationId, BoundPin>,
}

impl fmt::Debug for ContinuationPins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContinuationPins")
            .field("retained_bindings", &self.records.len())
            .field("max_records", &self.max_records)
            .finish_non_exhaustive()
    }
}

impl ContinuationPins {
    /// Creates an empty binding owner over an existing registry.
    /// A nonzero retained-record cap cannot exceed the shared list ceiling.
    pub fn new(
        registry: PinRegistry,
        instance_id: OpaqueId,
        max_records: usize,
    ) -> Result<Self, PinError> {
        if max_records == 0 || max_records > MAX_LIST_ITEMS {
            return Err(PinError::InvalidLimits);
        }
        Ok(Self { registry, instance_id, max_records, next_reference: 1, records: BTreeMap::new() })
    }

    /// Acquires and retains a real continuation pin with an immutable deadline.
    ///
    /// Store the returned reference in the same continuation's server record.
    /// If continuation creation fails, release and retire this exact binding.
    /// Caller time and route/owner inputs must come from their authoritative
    /// owners. No guard is returned before the registry has accepted the pin.
    #[allow(clippy::too_many_arguments)]
    pub fn acquire(
        &mut self,
        continuation_id: ContinuationId,
        owner: OpaqueId,
        route: RouteIdentity,
        epoch: Epoch,
        now_ms: u64,
        expires_at_ms: u64,
    ) -> Result<OpaqueRef, PinError> {
        if self.records.contains_key(&continuation_id) {
            return Err(PinError::OwnerMismatch);
        }
        if self.records.len() >= self.max_records {
            return Err(PinError::RegistryCapacityExceeded);
        }
        let next = self.next_reference.checked_add(1).ok_or(PinError::PinIdExhausted)?;
        let reference = OpaqueRef::new(format!(
            "continuation-pin-v1:{}:{}:{}",
            self.instance_id.as_str().len(), self.instance_id.as_str(), self.next_reference,
        )).map_err(|_| PinError::InvalidLimits)?;
        let guard = self.registry.acquire_epoch_pin(
            route, epoch, owner, EpochPinPurpose::Continuation { expires_at_ms }, now_ms,
        )?;
        let binding = BoundPin { reference: reference.clone(), guard, created_at_ms: now_ms, expires_at_ms };
        self.next_reference = next;
        self.records.insert(continuation_id, binding);
        Ok(reference)
    }

    /// Checks the exact original pin and lifetime against the real registry.
    ///
    /// Expiry, release, owner cleanup and registry failure are refusals, not
    /// reasons to acquire a replacement or extend TTL. The observation is not
    /// a lease: serving must still use the existing security/output barrier.
    pub fn validate(
        &self,
        continuation_id: ContinuationId,
        reference: &OpaqueRef,
        route: RouteIdentity,
        epoch: Epoch,
        now_ms: u64,
    ) -> Result<(), PinError> {
        let binding = self.binding(continuation_id, reference)?;
        if binding.guard.released {
            return Err(PinError::PinNotFound);
        }
        if binding.guard.route != route || binding.guard.epoch != epoch {
            return Err(PinError::OwnerMismatch);
        }
        if now_ms < binding.created_at_ms || now_ms >= binding.expires_at_ms {
            return Err(PinError::InvalidExpiry);
        }
        let inner = self.registry.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let record = inner.pins.get(&binding.guard.pin_id).ok_or(PinError::PinNotFound)?;
        if record.owner != binding.guard.owner || record.route != route || record.epoch != Some(epoch) {
            return Err(PinError::OwnerMismatch);
        }
        if record.kind != (PinKind::ContinuationEpoch { expires_at_ms: binding.expires_at_ms }) {
            return Err(PinError::InvalidExpiry);
        }
        Ok(())
    }

    /// Releases only the guard bound to this continuation and opaque reference.
    ///
    /// The binding is retained on both success and error. An unknown reference
    /// is never treated as evidence of prior cleanup. A late cancellation may
    /// safely repeat this operation; it cannot release a newer binding.
    pub fn release(
        &mut self,
        continuation_id: ContinuationId,
        reference: &OpaqueRef,
    ) -> Result<PinReleaseReceipt, PinError> {
        self.binding(continuation_id, reference)?;
        self.records.get_mut(&continuation_id).ok_or(PinError::PinNotFound)?.guard.try_release()
    }

    /// Retires known successful release evidence after the caller has removed
    /// the continuation and all its cleanup obligations. Never call on timeout
    /// or before the continuation owner acknowledges cleanup.
    /// A later acquisition uses a new reference even for the same continuation ID.
    pub fn retire_released(
        &mut self,
        continuation_id: ContinuationId,
        reference: &OpaqueRef,
    ) -> Result<(), PinError> {
        if !self.binding(continuation_id, reference)?.guard.released {
            return Err(PinError::OwnerMismatch);
        }
        self.records.remove(&continuation_id);
        Ok(())
    }

    fn binding(&self, id: ContinuationId, reference: &OpaqueRef) -> Result<&BoundPin, PinError> {
        let binding = self.records.get(&id).ok_or(PinError::PinNotFound)?;
        if &binding.reference != reference {
            return Err(PinError::OwnerMismatch);
        }
        Ok(binding)
    }
}
