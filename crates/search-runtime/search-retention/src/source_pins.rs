//! Process-local exact source-object pins for ephemeral retained-source handles.
//!
//! This owner records only reclamation protection. A pin does not authorize a
//! read, verify a C07 link, retain native custody, or survive process restart.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use search_contracts::{
    MAX_SET_ITEMS, ObjectResidencyKeyDigest, OpaqueId, SourceId, SourceNamespaceId,
    SourceRevisionRef,
};
use search_ports::{OperationContext, PackageOpaque, Port};

use crate::sweep::{PinEvidence, SweepLimits};
use crate::RetentionError;

/// Exact C07 source object identity and its verified source/residency binding.
///
/// The producer must construct this only from a Root-verified C07 object link.
/// This metadata is not a proof of the link and grants no read/access authority.
#[derive(Clone, Eq, PartialEq)]
pub struct SourcePinTarget {
    object_id: OpaqueId,
    source_revision_ref: SourceRevisionRef,
    residency_digest: ObjectResidencyKeyDigest,
}

impl fmt::Debug for SourcePinTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourcePinTarget")
            .field("object_id", &"<opaque>")
            .field("source_revision_ref", &"<opaque>")
            .field("residency_digest", &"<opaque>")
            .finish()
    }
}

impl SourcePinTarget {
    /// Binds the exact source object ID to the Root-verified C07 source link.
    #[must_use]
    pub const fn new(
        object_id: OpaqueId,
        source_revision_ref: SourceRevisionRef,
        residency_digest: ObjectResidencyKeyDigest,
    ) -> Self {
        Self {
            object_id,
            source_revision_ref,
            residency_digest,
        }
    }

    /// Exact physical C07 object identity.
    #[must_use]
    pub const fn object_id(&self) -> &OpaqueId {
        &self.object_id
    }

    /// Exact typed source revision from the verified C07 link.
    #[must_use]
    pub const fn source_revision_ref(&self) -> SourceRevisionRef {
        self.source_revision_ref
    }

    /// Exact residency binding from the verified C07 link.
    #[must_use]
    pub const fn residency_digest(&self) -> ObjectResidencyKeyDigest {
        self.residency_digest
    }
}

/// Root-owned fence and non-source pin observation made under one original
/// operation context. The two generation values remain separate domains.
#[derive(Clone, Eq, PartialEq)]
pub struct RootFenceObservation {
    control_generation: u64,
    publication_generation: u64,
    capture_time_ms: u64,
    additional_pinned_ids: BTreeSet<OpaqueId>,
}

impl fmt::Debug for RootFenceObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootFenceObservation")
            .field("control_generation", &self.control_generation)
            .field("publication_generation", &self.publication_generation)
            .field("capture_time_ms", &self.capture_time_ms)
            .field("additional_pin_count", &self.additional_pinned_ids.len())
            .finish()
    }
}

impl RootFenceObservation {
    /// Creates one observation returned by the actual Root fence port.
    ///
    /// `capture_time_ms` is diagnostic metadata only; pin expiry uses the
    /// registry's process-monotonic `Instant` deadlines.
    #[must_use]
    pub const fn new(
        control_generation: u64,
        publication_generation: u64,
        capture_time_ms: u64,
        additional_pinned_ids: BTreeSet<OpaqueId>,
    ) -> Self {
        Self {
            control_generation,
            publication_generation,
            capture_time_ms,
            additional_pinned_ids,
        }
    }
}

/// Actual Root observation port used on both sides of one pin snapshot.
///
/// Implementations must use the supplied original operation context and return
/// the current control/publication generations plus exact active object IDs
/// from other pin owners. They must not infer freshness from a caller boolean.
pub trait RootFenceObservationPort: Port {
    /// Reads one bounded current Root/pin observation under `context`.
    fn observe_root_fence(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<RootFenceObservation, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PinRecord {
    target: SourcePinTarget,
    monotonic_deadline: Instant,
}

#[derive(Debug)]
struct RegistryState {
    next_pin_id: u64,
    pin_generation: u64,
    pins: BTreeMap<u64, PinRecord>,
    poisoned: bool,
}

/// One shared, bounded process-local owner for exact C07 source-object pins.
///
/// Clones share the same registry. A pin is RAII-only, cannot be renewed, and
/// is visible to S28 only through a fence-checked `PinEvidence` capture.
#[derive(Clone)]
pub struct SourcePinRegistry {
    max_pins: usize,
    inner: Arc<Mutex<RegistryState>>,
}

impl fmt::Debug for SourcePinRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let active_pins = self
            .inner
            .lock()
            .map_or(0, |state| state.pins.len());
        formatter
            .debug_struct("SourcePinRegistry")
            .field("active_pins", &active_pins)
            .field("max_pins", &self.max_pins)
            .finish_non_exhaustive()
    }
}

impl SourcePinRegistry {
    /// Creates an empty registry under the S28 and bounded-set ceilings.
    pub fn new(limits: SweepLimits) -> Result<Self, RetentionError> {
        let limits = limits.validate()?;
        Ok(Self {
            max_pins: limits.max_objects.min(MAX_SET_ITEMS),
            inner: Arc::new(Mutex::new(RegistryState {
                next_pin_id: 1,
                pin_generation: 1,
                pins: BTreeMap::new(),
                poisoned: false,
            })),
        })
    }

    /// Pins the exact object until the caller's original handle deadline.
    ///
    /// The supplied `Instant` is retained unchanged. This method never derives
    /// an expiry from UTC, request budget, or a configured/default TTL.
    pub fn pin(
        &self,
        target: SourcePinTarget,
        monotonic_deadline: Instant,
    ) -> Result<OwnedSourcePinGuard, RetentionError> {
        if !deadline_is_live(monotonic_deadline, Instant::now()) {
            return Err(RetentionError::SourcePinInactive);
        }

        let mut state = self.lock_state()?;
        prune_expired(&mut state, Instant::now())?;
        if state.pins.len() >= self.max_pins {
            return Err(RetentionError::CapacityExceeded);
        }
        let pin_id = state.next_pin_id;
        let Some(next_pin_id) = pin_id.checked_add(1) else {
            state.poisoned = true;
            return Err(RetentionError::ContractExhausted);
        };
        let next_generation = next_generation(&mut state)?;
        state.pins.insert(
            pin_id,
            PinRecord {
                target: target.clone(),
                monotonic_deadline,
            },
        );
        state.next_pin_id = next_pin_id;
        state.pin_generation = next_generation;
        drop(state);

        Ok(OwnedSourcePinGuard {
            inner: Arc::clone(&self.inner),
            pin_id,
            target,
            monotonic_deadline,
            released: false,
        })
    }

    /// Revokes every pin for one exact source revision before purge/collection.
    pub fn revoke_source_revision(
        &self,
        source_revision_ref: &SourceRevisionRef,
    ) -> Result<(), RetentionError> {
        self.revoke_matching(|record| record.target.source_revision_ref == *source_revision_ref)
    }

    /// Revokes every revision pin for one source identity before purge/collection.
    pub fn revoke_source(
        &self,
        namespace_id: SourceNamespaceId,
        source_id: SourceId,
    ) -> Result<(), RetentionError> {
        self.revoke_matching(|record| {
            record.target.source_revision_ref.source_namespace_id == namespace_id
                && record.target.source_revision_ref.source_id == source_id
        })
    }

    /// Captures S28 pin evidence around two actual Root observations.
    ///
    /// The same original `OperationContext` is passed to both Root reads. Any
    /// refusal, control/publication drift, other-pin drift, registry mutation,
    /// poison, or capacity overflow returns no evidence.
    pub fn capture_pin_evidence<P: RootFenceObservationPort>(
        &self,
        root: &mut P,
        context: &OperationContext<P::Cancellation>,
    ) -> Result<PinEvidence, RetentionError> {
        context
            .preflight()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        let before = root
            .observe_root_fence(context)
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        validate_fence_observation(&before)?;

        context
            .preflight()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        let snapshot = self.snapshot()?;
        if before.additional_pinned_ids.len() > self.max_pins {
            return Err(RetentionError::CapacityExceeded);
        }

        context
            .preflight()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        let after = root
            .observe_root_fence(context)
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        validate_fence_observation(&after)?;
        if before.control_generation != after.control_generation
            || before.publication_generation != after.publication_generation
        {
            return Err(RetentionError::RootGenerationChanged);
        }
        if before.additional_pinned_ids != after.additional_pinned_ids {
            return Err(RetentionError::PinProtectionUnknown);
        }

        self.validate_generation(snapshot.pin_generation)?;
        let mut pinned_ids = snapshot.pinned_ids;
        for id in after.additional_pinned_ids {
            pinned_ids.insert(id);
            if pinned_ids.len() > self.max_pins {
                return Err(RetentionError::CapacityExceeded);
            }
        }
        context
            .preflight()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;

        Ok(PinEvidence {
            pinned_ids,
            pin_generation: snapshot.pin_generation,
            capture_time_ms: after.capture_time_ms,
            fresh: true,
            control_generation: after.control_generation,
            publication_generation: after.publication_generation,
            source_pin_registry: self.clone(),
        })
    }

    fn revoke_matching(
        &self,
        matches: impl Fn(&PinRecord) -> bool,
    ) -> Result<(), RetentionError> {
        let mut state = self.lock_state()?;
        prune_expired(&mut state, Instant::now())?;
        if !state.pins.values().any(&matches) {
            return Ok(());
        }
        let next_generation = next_generation(&mut state)?;
        state.pins.retain(|_, record| !matches(record));
        state.pin_generation = next_generation;
        drop(state);
        Ok(())
    }

    fn snapshot(&self) -> Result<SourcePinSnapshot, RetentionError> {
        let mut state = self.lock_state()?;
        prune_expired(&mut state, Instant::now())?;
        if state.pins.len() > self.max_pins {
            return Err(RetentionError::CapacityExceeded);
        }
        let pinned_ids = state
            .pins
            .values()
            .map(|record| record.target.object_id.clone())
            .collect();
        Ok(SourcePinSnapshot {
            pinned_ids,
            pin_generation: state.pin_generation,
        })
    }

    pub(crate) fn validate_generation(&self, expected: u64) -> Result<(), RetentionError> {
        let mut state = self.lock_state()?;
        prune_expired(&mut state, Instant::now())?;
        if state.pin_generation != expected {
            return Err(RetentionError::RootGenerationChanged);
        }
        drop(state);
        Ok(())
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, RegistryState>, RetentionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        if state.poisoned {
            return Err(RetentionError::PinProtectionUnknown);
        }
        Ok(state)
    }
}

struct SourcePinSnapshot {
    pinned_ids: BTreeSet<OpaqueId>,
    pin_generation: u64,
}

/// Owned exact pin retained by an ephemeral source handle.
///
/// This guard carries no Root borrow, read budget, source bytes, or native
/// custody. It is not an access permit. Drop releases only its exact registry
/// entry; expiration is always checked against its original process `Instant`.
pub struct OwnedSourcePinGuard {
    inner: Arc<Mutex<RegistryState>>,
    pin_id: u64,
    target: SourcePinTarget,
    monotonic_deadline: Instant,
    released: bool,
}

impl OwnedSourcePinGuard {
    /// Exact pinned C07/source/residency binding.
    #[must_use]
    pub const fn target(&self) -> &SourcePinTarget {
        &self.target
    }

    /// The exact process-monotonic deadline supplied at acquisition.
    #[must_use]
    pub const fn monotonic_deadline(&self) -> Instant {
        self.monotonic_deadline
    }

    /// Confirms this exact pin remains registered and before its original deadline.
    pub fn validate_active(&self) -> Result<(), RetentionError> {
        if !deadline_is_live(self.monotonic_deadline, Instant::now()) {
            return Err(RetentionError::SourcePinInactive);
        }
        let state = self
            .inner
            .lock()
            .map_err(|_| RetentionError::PinProtectionUnknown)?;
        validate_guard_state(
            &state,
            self.pin_id,
            &self.target,
            self.monotonic_deadline,
            Instant::now(),
        )
    }
}

impl PackageOpaque for OwnedSourcePinGuard {
    fn owner_package(&self) -> &'static str {
        "search-retention"
    }
}

impl fmt::Debug for OwnedSourcePinGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedSourcePinGuard")
            .field("target", &"<opaque>")
            .field("released", &self.released)
            .finish_non_exhaustive()
    }
}

impl Drop for OwnedSourcePinGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let Some(record) = state.pins.get(&self.pin_id) else {
            return;
        };
        if record.target != self.target || record.monotonic_deadline != self.monotonic_deadline {
            state.poisoned = true;
            return;
        }
        state.pins.remove(&self.pin_id);
        if state.poisoned {
            return;
        }
        match state.pin_generation.checked_add(1) {
            Some(next) => state.pin_generation = next,
            None => state.poisoned = true,
        }
    }
}

const fn validate_fence_observation(
    observation: &RootFenceObservation,
) -> Result<(), RetentionError> {
    if observation.control_generation == 0 {
        return Err(RetentionError::RootGenerationChanged);
    }
    Ok(())
}

fn deadline_is_live(deadline: Instant, now: Instant) -> bool {
    deadline > now
}

fn validate_guard_state(
    state: &RegistryState,
    pin_id: u64,
    target: &SourcePinTarget,
    monotonic_deadline: Instant,
    now: Instant,
) -> Result<(), RetentionError> {
    if !deadline_is_live(monotonic_deadline, now) {
        return Err(RetentionError::SourcePinInactive);
    }
    if state.poisoned {
        return Err(RetentionError::PinProtectionUnknown);
    }
    let record = state
        .pins
        .get(&pin_id)
        .ok_or(RetentionError::SourcePinInactive)?;
    if &record.target != target || record.monotonic_deadline != monotonic_deadline {
        return Err(RetentionError::PinProtectionUnknown);
    }
    Ok(())
}

const fn next_generation(state: &mut RegistryState) -> Result<u64, RetentionError> {
    if let Some(next) = state.pin_generation.checked_add(1) {
        Ok(next)
    } else {
        state.poisoned = true;
        Err(RetentionError::ContractExhausted)
    }
}

fn prune_expired(state: &mut RegistryState, now: Instant) -> Result<(), RetentionError> {
    if !state
        .pins
        .values()
        .any(|record| !deadline_is_live(record.monotonic_deadline, now))
    {
        return Ok(());
    }
    let next_generation = next_generation(state)?;
    state
        .pins
        .retain(|_, record| deadline_is_live(record.monotonic_deadline, now));
    state.pin_generation = next_generation;
    Ok(())
}

#[cfg(test)]
#[path = "source_pins/tests.rs"]
mod tests;
