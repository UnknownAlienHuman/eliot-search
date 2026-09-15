//! Process-local query and continuation pin lifecycle.

use search_contracts::{Epoch, OpaqueId};
use search_epoch_pins::{
    EpochPinGuard, EpochPinPurpose, ExpiryReceipt, PinError, PinRegistry,
    PinReleaseReceipt, RouteIdentity,
};

use super::error::RebuildError;

/// Exact query session: owner, route and visible epoch presented together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySession {
    /// Request, connection, or continuation identity owning the pin.
    pub owner: OpaqueId,
    /// Route the caller believes is active.
    pub route: RouteIdentity,
    /// Epoch the caller wants to observe.
    pub epoch: Epoch,
}

/// Acquires one bounded process-local epoch pin.
pub fn begin_pinned_query(
    registry: &PinRegistry,
    session: &QuerySession,
    purpose: EpochPinPurpose,
    now_ms: u64,
) -> Result<EpochPinGuard, RebuildError> {
    registry
        .acquire_epoch_pin(
            session.route,
            session.epoch,
            session.owner.clone(),
            purpose,
            now_ms,
        )
        .map_err(|error| match error {
            PinError::RouteNotActive => RebuildError::StaleRoute,
            PinError::EpochNotVisible => RebuildError::StaleRevision,
            PinError::RegistryCapacityExceeded
            | PinError::OwnerCapacityExceeded => RebuildError::BudgetExceeded,
            other => RebuildError::PinDenied(other),
        })
}

/// Idempotently releases all pins for one cancelled/disconnected owner.
pub fn release_owner_pins_of(
    registry: &PinRegistry,
    owner: &OpaqueId,
) -> Result<PinReleaseReceipt, RebuildError> {
    registry
        .release_owner_pins(owner)
        .map_err(RebuildError::PinDenied)
}

/// Expires continuation pins under one explicit finite sweep bound.
pub fn expire_continuation_pins_bounded(
    registry: &PinRegistry,
    now_ms: u64,
    max_expirations: usize,
) -> Result<ExpiryReceipt, RebuildError> {
    registry
        .expire_continuation_pins(now_ms, max_expirations)
        .map_err(|error| match error {
            PinError::InvalidLimits => RebuildError::InvalidLimits,
            other => RebuildError::PinDenied(other),
        })
}
