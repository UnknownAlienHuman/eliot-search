//! Process-local epoch and route pin registry.
//!
//! Ordinary query and ephemeral continuation pins are intentionally not
//! durable. A daemon crash drops them; durable jobs must replan from durable
//! checkpoints instead of pretending an old process-local pin survived.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use search_contracts::{
    CollectionGenerationId, CollectionRouteRevision, Epoch, OpaqueId,
};

/// Closed pin-registry failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PinError {
    /// A finite registry limit is zero or internally inconsistent.
    InvalidLimits,
    /// Requested route is not the active route.
    RouteNotActive,
    /// Requested epoch is not visible on the active route.
    EpochNotVisible,
    /// Global process-local pin capacity is exhausted.
    RegistryCapacityExceeded,
    /// One owner exceeded its pin ceiling.
    OwnerCapacityExceeded,
    /// Pin identifier space is exhausted.
    PinIdExhausted,
    /// A requested pin does not exist.
    PinNotFound,
    /// A foreign owner attempted to mutate a pin.
    OwnerMismatch,
    /// Ordinary query pins cannot be renewed.
    PinNotRenewable,
    /// Continuation expiry is stale or exceeds the configured TTL.
    InvalidExpiry,
    /// Internal registry lock is poisoned; reclaim must fail closed.
    RegistryPoisoned,
}

impl PinError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PIN_INVALID_LIMITS",
            Self::RouteNotActive => "PIN_ROUTE_NOT_ACTIVE",
            Self::EpochNotVisible => "PIN_EPOCH_NOT_VISIBLE",
            Self::RegistryCapacityExceeded => "PIN_REGISTRY_CAPACITY_EXCEEDED",
            Self::OwnerCapacityExceeded => "PIN_OWNER_CAPACITY_EXCEEDED",
            Self::PinIdExhausted => "PIN_ID_EXHAUSTED",
            Self::PinNotFound => "PIN_NOT_FOUND",
            Self::OwnerMismatch => "PIN_OWNER_MISMATCH",
            Self::PinNotRenewable => "PIN_NOT_RENEWABLE",
            Self::InvalidExpiry => "PIN_INVALID_EXPIRY",
            Self::RegistryPoisoned => "PIN_REGISTRY_POISONED",
        }
    }
}

impl fmt::Display for PinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PinError {}

/// Exact collection route identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RouteIdentity {
    /// Physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Guarded logical route revision.
    pub route_revision: CollectionRouteRevision,
}

/// Finite process-local pin limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinLimits {
    /// Maximum pins across the process.
    pub max_total_pins: usize,
    /// Maximum pins owned by one request/connection/continuation.
    pub max_pins_per_owner: usize,
    /// Maximum continuation extension from the caller-supplied current time.
    pub max_continuation_ttl_ms: u64,
}

impl PinLimits {
    /// Conservative baseline.
    pub const BASELINE: Self = Self {
        max_total_pins: 65_536,
        max_pins_per_owner: 256,
        max_continuation_ttl_ms: 15 * 60 * 1_000,
    };

    /// Validates every finite dimension.
    pub const fn validate(self) -> Result<Self, PinError> {
        if self.max_total_pins == 0
            || self.max_pins_per_owner == 0
            || self.max_continuation_ttl_ms == 0
        {
            Err(PinError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Epoch pin purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochPinPurpose {
    /// Non-renewable ordinary query pin.
    Query,
    /// Renewable bounded continuation pin.
    Continuation {
        /// Initial finite expiry supplied by the caller's clock.
        expires_at_ms: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PinKind {
    QueryEpoch,
    ContinuationEpoch { expires_at_ms: u64 },
    Route,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PinRecord {
    owner: OpaqueId,
    route: RouteIdentity,
    epoch: Option<Epoch>,
    kind: PinKind,
}

#[derive(Debug)]
struct RegistryInner {
    active_route: RouteIdentity,
    visible_epoch: Epoch,
    limits: PinLimits,
    next_pin_id: u64,
    pins: BTreeMap<u64, PinRecord>,
    owner_counts: BTreeMap<OpaqueId, usize>,
}

impl RegistryInner {
    fn allocate_id(&mut self) -> Result<u64, PinError> {
        let id = self.next_pin_id;
        self.next_pin_id = self
            .next_pin_id
            .checked_add(1)
            .ok_or(PinError::PinIdExhausted)?;
        Ok(id)
    }

    fn insert(&mut self, record: PinRecord) -> Result<u64, PinError> {
        if self.pins.len() >= self.limits.max_total_pins {
            return Err(PinError::RegistryCapacityExceeded);
        }
        let owner_count = self.owner_counts.get(&record.owner).copied().unwrap_or(0);
        if owner_count >= self.limits.max_pins_per_owner {
            return Err(PinError::OwnerCapacityExceeded);
        }
        let id = self.allocate_id()?;
        self.owner_counts
            .insert(record.owner.clone(), owner_count.saturating_add(1));
        self.pins.insert(id, record);
        Ok(id)
    }

    fn remove(&mut self, pin_id: u64) -> bool {
        let Some(record) = self.pins.remove(&pin_id) else {
            return false;
        };
        if let Some(count) = self.owner_counts.get_mut(&record.owner) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.owner_counts.remove(&record.owner);
            }
        }
        true
    }
}

/// Linearizable process-local registry shared by guards.
#[derive(Clone, Debug)]
pub struct PinRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

impl PinRegistry {
    /// Creates a registry for one exact active route and visible epoch.
    pub fn new(
        active_route: RouteIdentity,
        visible_epoch: Epoch,
        limits: PinLimits,
    ) -> Result<Self, PinError> {
        Ok(Self {
            inner: Arc::new(Mutex::new(RegistryInner {
                active_route,
                visible_epoch,
                limits: limits.validate()?,
                next_pin_id: 1,
                pins: BTreeMap::new(),
                owner_counts: BTreeMap::new(),
            })),
        })
    }

    /// Updates active route/epoch authority. Existing pins remain attached to
    /// their original route and continue to protect reclamation.
    pub fn publish_active_route(
        &self,
        route: RouteIdentity,
        visible_epoch: Epoch,
    ) -> Result<(), PinError> {
        let mut inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        if route == inner.active_route && visible_epoch < inner.visible_epoch {
            return Err(PinError::EpochNotVisible);
        }
        inner.active_route = route;
        inner.visible_epoch = visible_epoch;
        Ok(())
    }

    /// Acquires an epoch pin after exact active-route and visible-epoch checks.
    pub fn acquire_epoch_pin(
        &self,
        route: RouteIdentity,
        epoch: Epoch,
        owner: OpaqueId,
        purpose: EpochPinPurpose,
        now_ms: u64,
    ) -> Result<EpochPinGuard, PinError> {
        let mut inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        if route != inner.active_route {
            return Err(PinError::RouteNotActive);
        }
        match purpose {
            EpochPinPurpose::Query if epoch != inner.visible_epoch => {
                return Err(PinError::EpochNotVisible);
            }
            EpochPinPurpose::Continuation { expires_at_ms }
                if epoch > inner.visible_epoch
                    || expires_at_ms <= now_ms
                    || expires_at_ms.saturating_sub(now_ms)
                        > inner.limits.max_continuation_ttl_ms =>
            {
                return Err(PinError::InvalidExpiry);
            }
            EpochPinPurpose::Query | EpochPinPurpose::Continuation { .. } => {}
        }
        let kind = match purpose {
            EpochPinPurpose::Query => PinKind::QueryEpoch,
            EpochPinPurpose::Continuation { expires_at_ms } => {
                PinKind::ContinuationEpoch { expires_at_ms }
            }
        };
        let pin_id = inner.insert(PinRecord {
            owner: owner.clone(),
            route,
            epoch: Some(epoch),
            kind,
        })?;
        Ok(EpochPinGuard {
            registry: Arc::downgrade(&self.inner),
            pin_id,
            owner,
            route,
            epoch,
            released: false,
        })
    }

    /// Acquires a route pin independently from an epoch.
    pub fn acquire_route_pin(
        &self,
        route: RouteIdentity,
        owner: OpaqueId,
    ) -> Result<RoutePinGuard, PinError> {
        let mut inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let pin_id = inner.insert(PinRecord {
            owner: owner.clone(),
            route,
            epoch: None,
            kind: PinKind::Route,
        })?;
        Ok(RoutePinGuard {
            registry: Arc::downgrade(&self.inner),
            pin_id,
            owner,
            route,
            released: false,
        })
    }

    /// Idempotently releases every pin owned by one request or connection.
    pub fn release_owner_pins(&self, owner: &OpaqueId) -> Result<PinReleaseReceipt, PinError> {
        let mut inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let pin_ids = inner
            .pins
            .iter()
            .filter(|(_, record)| &record.owner == owner)
            .map(|(pin_id, _)| *pin_id)
            .collect::<Vec<_>>();
        for pin_id in &pin_ids {
            inner.remove(*pin_id);
        }
        Ok(PinReleaseReceipt {
            released_pins: pin_ids.len(),
        })
    }

    /// Expires bounded continuation pins at an explicit caller-supplied time.
    pub fn expire_continuation_pins(
        &self,
        now_ms: u64,
        max_expirations: usize,
    ) -> Result<ExpiryReceipt, PinError> {
        if max_expirations == 0 {
            return Err(PinError::InvalidLimits);
        }
        let mut inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let expired = inner
            .pins
            .iter()
            .filter(|(_, record)| {
                matches!(
                    record.kind,
                    PinKind::ContinuationEpoch { expires_at_ms } if expires_at_ms <= now_ms
                )
            })
            .map(|(pin_id, _)| *pin_id)
            .take(max_expirations)
            .collect::<Vec<_>>();
        for pin_id in &expired {
            inner.remove(*pin_id);
        }
        Ok(ExpiryReceipt {
            expired_pins: expired.len(),
            more_expired: inner.pins.values().any(|record| {
                matches!(
                    record.kind,
                    PinKind::ContinuationEpoch { expires_at_ms } if expires_at_ms <= now_ms
                )
            }),
        })
    }

    /// Returns a client-identity-free consistent snapshot.
    pub fn snapshot(&self) -> Result<PinRegistrySnapshot, PinError> {
        let inner = self.inner.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let mut route_counts = BTreeMap::new();
        let mut epoch_counts = BTreeMap::new();
        let mut earliest_continuation_expiry_ms: Option<u64> = None;
        for record in inner.pins.values() {
            match record.kind {
                PinKind::Route => {
                    *route_counts.entry(record.route).or_insert(0_usize) += 1;
                }
                PinKind::QueryEpoch => {
                    if let Some(epoch) = record.epoch {
                        *epoch_counts.entry((record.route, epoch)).or_insert(0_usize) += 1;
                    }
                }
                PinKind::ContinuationEpoch { expires_at_ms } => {
                    if let Some(epoch) = record.epoch {
                        *epoch_counts.entry((record.route, epoch)).or_insert(0_usize) += 1;
                    }
                    earliest_continuation_expiry_ms = Some(
                        earliest_continuation_expiry_ms
                            .map_or(expires_at_ms, |current| current.min(expires_at_ms)),
                    );
                }
            }
        }
        Ok(PinRegistrySnapshot {
            active_route: inner.active_route,
            visible_epoch: inner.visible_epoch,
            total_pins: inner.pins.len(),
            route_counts,
            epoch_counts,
            earliest_continuation_expiry_ms,
        })
    }
}

/// Non-serializable epoch authority that releases exactly once.
pub struct EpochPinGuard {
    registry: Weak<Mutex<RegistryInner>>,
    pin_id: u64,
    owner: OpaqueId,
    route: RouteIdentity,
    epoch: Epoch,
    released: bool,
}

impl EpochPinGuard {
    /// Bound route.
    #[must_use]
    pub const fn route(&self) -> RouteIdentity {
        self.route
    }

    /// Bound visible epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Extends a continuation pin within its original route/epoch and TTL.
    pub fn renew_continuation_pin(
        &mut self,
        now_ms: u64,
        new_expiry_ms: u64,
    ) -> Result<PinRenewalReceipt, PinError> {
        if self.released {
            return Err(PinError::PinNotFound);
        }
        let registry = self.registry.upgrade().ok_or(PinError::PinNotFound)?;
        let mut inner = registry.lock().map_err(|_| PinError::RegistryPoisoned)?;
        let max_ttl = inner.limits.max_continuation_ttl_ms;
        let record = inner.pins.get_mut(&self.pin_id).ok_or(PinError::PinNotFound)?;
        if record.owner != self.owner || record.route != self.route || record.epoch != Some(self.epoch)
        {
            return Err(PinError::OwnerMismatch);
        }
        let PinKind::ContinuationEpoch { expires_at_ms } = &mut record.kind else {
            return Err(PinError::PinNotRenewable);
        };
        if new_expiry_ms <= *expires_at_ms
            || new_expiry_ms <= now_ms
            || new_expiry_ms.saturating_sub(now_ms) > max_ttl
        {
            return Err(PinError::InvalidExpiry);
        }
        let previous_expiry_ms = *expires_at_ms;
        *expires_at_ms = new_expiry_ms;
        Ok(PinRenewalReceipt {
            route: self.route,
            epoch: self.epoch,
            previous_expiry_ms,
            new_expiry_ms,
        })
    }

    /// Explicitly releases this guard. Drop is an idempotent fallback.
    pub fn release(mut self) -> PinReleaseReceipt {
        let released = release_weak_pin(&self.registry, self.pin_id);
        self.released = true;
        PinReleaseReceipt {
            released_pins: usize::from(released),
        }
    }
}

impl fmt::Debug for EpochPinGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpochPinGuard")
            .field("route", &self.route)
            .field("epoch", &self.epoch)
            .field("owner", &"<opaque>")
            .field("released", &self.released)
            .finish()
    }
}

impl Drop for EpochPinGuard {
    fn drop(&mut self) {
        if !self.released {
            release_weak_pin(&self.registry, self.pin_id);
            self.released = true;
        }
    }
}

/// Non-serializable route authority that releases exactly once.
pub struct RoutePinGuard {
    registry: Weak<Mutex<RegistryInner>>,
    pin_id: u64,
    owner: OpaqueId,
    route: RouteIdentity,
    released: bool,
}

impl RoutePinGuard {
    /// Bound collection route.
    #[must_use]
    pub const fn route(&self) -> RouteIdentity {
        self.route
    }

    /// Explicitly releases this guard. Drop is an idempotent fallback.
    pub fn release(mut self) -> PinReleaseReceipt {
        let released = release_weak_pin(&self.registry, self.pin_id);
        self.released = true;
        PinReleaseReceipt {
            released_pins: usize::from(released),
        }
    }
}

impl fmt::Debug for RoutePinGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutePinGuard")
            .field("route", &self.route)
            .field("owner", &"<opaque>")
            .field("released", &self.released)
            .finish()
    }
}

impl Drop for RoutePinGuard {
    fn drop(&mut self) {
        if !self.released {
            release_weak_pin(&self.registry, self.pin_id);
            self.released = true;
        }
    }
}

fn release_weak_pin(registry: &Weak<Mutex<RegistryInner>>, pin_id: u64) -> bool {
    registry
        .upgrade()
        .and_then(|registry| registry.lock().ok().map(|mut inner| inner.remove(pin_id)))
        .unwrap_or(false)
}

/// Client-identity-free pin registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinRegistrySnapshot {
    /// Current active route.
    pub active_route: RouteIdentity,
    /// Current visible epoch.
    pub visible_epoch: Epoch,
    /// Total active pins.
    pub total_pins: usize,
    /// Active route-pin counts.
    pub route_counts: BTreeMap<RouteIdentity, usize>,
    /// Active epoch-pin counts.
    pub epoch_counts: BTreeMap<(RouteIdentity, Epoch), usize>,
    /// Earliest continuation expiry, if any.
    pub earliest_continuation_expiry_ms: Option<u64>,
}

/// Exact retired route/manifest visibility fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetiredVisibilityFence {
    /// Route containing retired points.
    pub route: RouteIdentity,
    /// First epoch at which those points are no longer visible.
    pub retirement_epoch_exclusive: Epoch,
}

/// Reclamation decision derived only from active process-local pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclamationWatermark {
    /// Whether no active pin can observe the retired state.
    pub reclaimable: bool,
    /// Blocking route pins.
    pub blocking_route_pins: usize,
    /// Blocking older-epoch pins.
    pub blocking_epoch_pins: usize,
}

/// Computes a fail-closed reclamation watermark.
#[must_use]
pub fn compute_reclamation_watermark(
    retired: RetiredVisibilityFence,
    snapshot: &PinRegistrySnapshot,
) -> ReclamationWatermark {
    let blocking_route_pins = snapshot
        .route_counts
        .get(&retired.route)
        .copied()
        .unwrap_or(0);
    let blocking_epoch_pins = snapshot
        .epoch_counts
        .iter()
        .filter(|((route, epoch), _)| {
            *route == retired.route && *epoch < retired.retirement_epoch_exclusive
        })
        .map(|(_, count)| *count)
        .sum();
    ReclamationWatermark {
        reclaimable: blocking_route_pins == 0 && blocking_epoch_pins == 0,
        blocking_route_pins,
        blocking_epoch_pins,
    }
}

/// Explicit pin release receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinReleaseReceipt {
    /// Number of pins released.
    pub released_pins: usize,
}

/// Continuation pin renewal receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinRenewalReceipt {
    /// Original route.
    pub route: RouteIdentity,
    /// Original epoch.
    pub epoch: Epoch,
    /// Previous expiry.
    pub previous_expiry_ms: u64,
    /// New expiry.
    pub new_expiry_ms: u64,
}

/// Bounded continuation expiry receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpiryReceipt {
    /// Pins expired in this slice.
    pub expired_pins: usize,
    /// Whether more expired pins remain for another slice.
    pub more_expired: bool,
}
