//! Immutable owner snapshots and closed lifecycle states.

use search_contracts::{Blake3Digest32, NonZeroRevision, OwnerEpoch};

use crate::{
    DataRootIdentity, DrainReason, OwnerError, OwnerOperation, OwnerRecord,
    OwnerShutdownReceipt,
};

/// Cloneable fence corresponding to a process-local drain token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrainFence {
    pub(crate) operation: OwnerOperation,
    pub(crate) reason: DrainReason,
}

/// Cloneable fence corresponding to a process-local release permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseFence {
    pub(crate) dependency_receipt_digest: Blake3Digest32,
    pub(crate) receipt_count: usize,
}

/// Prepared acquisition retained across the OS/durable mutation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAcquire {
    /// Exact expected durable owner record.
    pub expected_record: OwnerRecord,
    /// Whether an exact stale record must be removed before acquisition.
    pub stale_cleanup_required: bool,
}

/// Closed lifecycle of one data-root owner supervisor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerState {
    /// No owner currently claims the root.
    Vacant {
        /// Canonical root identity managed by this supervisor.
        root: DataRootIdentity,
        /// Highest owner epoch known to have reached durable state.
        last_epoch: Option<OwnerEpoch>,
    },
    /// Acquisition is prepared but no external mutation result is accepted.
    Acquiring {
        /// Last durable epoch before the pending acquisition.
        previous_epoch: Option<OwnerEpoch>,
        /// Exact acquisition plan.
        pending: PendingAcquire,
    },
    /// Acquisition may have crossed the external mutation boundary.
    AcquireOutcomeUnknown {
        /// Last epoch known before the unresolved operation.
        previous_epoch: Option<OwnerEpoch>,
        /// Exact record whose presence must be recovered by readback.
        pending: PendingAcquire,
    },
    /// Exact active owner is installed.
    Active {
        /// Authoritative owner record.
        record: OwnerRecord,
    },
    /// Exact owner is draining and admits no new ordinary work.
    Draining {
        /// Authoritative draining record.
        record: OwnerRecord,
        /// Exact drain fence.
        drain: DrainFence,
    },
    /// Clean release is prepared and awaiting external mutation results.
    Releasing {
        /// Authoritative draining record.
        record: OwnerRecord,
        /// Exact drain fence.
        drain: DrainFence,
        /// Dependency-shutdown fence.
        permit: ReleaseFence,
        /// Immutable release operation.
        operation: OwnerOperation,
    },
    /// Release may have crossed an external mutation boundary.
    ReleaseOutcomeUnknown {
        /// Authoritative draining record before release.
        record: OwnerRecord,
        /// Exact drain fence.
        drain: DrainFence,
        /// Dependency-shutdown fence.
        permit: ReleaseFence,
        /// Immutable release operation.
        operation: OwnerOperation,
    },
    /// The last exact owner was cleanly released.
    Released {
        /// Canonical root identity.
        root: DataRootIdentity,
        /// Last durable owner epoch.
        last_epoch: OwnerEpoch,
        /// Exact shutdown receipt.
        receipt: OwnerShutdownReceipt,
    },
    /// Contradictory or incomplete evidence blocks ownership changes.
    Quarantined {
        /// Canonical root identity.
        root: DataRootIdentity,
        /// Highest safely known owner epoch.
        last_epoch: Option<OwnerEpoch>,
        /// Closed quarantine reason.
        reason: OwnerError,
    },
}

impl OwnerState {
    /// Root identity governed by this state.
    #[must_use]
    pub const fn root(&self) -> DataRootIdentity {
        match self {
            Self::Vacant { root, .. }
            | Self::Released { root, .. }
            | Self::Quarantined { root, .. } => *root,
            Self::Acquiring { pending, .. } | Self::AcquireOutcomeUnknown { pending, .. } => {
                pending.expected_record.binding().root()
            }
            Self::Active { record }
            | Self::Draining { record, .. }
            | Self::Releasing { record, .. }
            | Self::ReleaseOutcomeUnknown { record, .. } => record.binding().root(),
        }
    }

    /// Highest owner epoch represented by durable or pending state.
    #[must_use]
    pub const fn highest_epoch(&self) -> Option<OwnerEpoch> {
        match self {
            Self::Vacant { last_epoch, .. }
            | Self::Quarantined { last_epoch, .. }
            | Self::Acquiring {
                previous_epoch: last_epoch,
                ..
            }
            | Self::AcquireOutcomeUnknown {
                previous_epoch: last_epoch,
                ..
            } => *last_epoch,
            Self::Active { record }
            | Self::Draining { record, .. }
            | Self::Releasing { record, .. }
            | Self::ReleaseOutcomeUnknown { record, .. } => Some(record.binding().epoch()),
            Self::Released { last_epoch, .. } => Some(*last_epoch),
        }
    }

    /// Current authoritative record when ownership remains installed.
    #[must_use]
    pub const fn record(&self) -> Option<&OwnerRecord> {
        match self {
            Self::Active { record }
            | Self::Draining { record, .. }
            | Self::Releasing { record, .. }
            | Self::ReleaseOutcomeUnknown { record, .. } => Some(record),
            Self::Vacant { .. }
            | Self::Acquiring { .. }
            | Self::AcquireOutcomeUnknown { .. }
            | Self::Released { .. }
            | Self::Quarantined { .. } => None,
        }
    }
}

/// Immutable revisioned view of one owner state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerSnapshot {
    revision: NonZeroRevision,
    state: OwnerState,
}

impl OwnerSnapshot {
    /// Creates a vacant supervisor snapshot at revision one.
    ///
    /// # Errors
    ///
    /// Fails only if the shared non-zero revision contract cannot represent one.
    pub fn new(root: DataRootIdentity) -> Result<Self, OwnerError> {
        Ok(Self {
            revision: NonZeroRevision::new(1).map_err(|_| OwnerError::ContractExhausted)?,
            state: OwnerState::Vacant {
                root,
                last_epoch: None,
            },
        })
    }

    /// Snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Closed lifecycle state.
    #[must_use]
    pub const fn state(&self) -> &OwnerState {
        &self.state
    }

    pub(crate) fn advanced(&self, state: OwnerState) -> Result<Self, OwnerError> {
        let revision = self
            .revision
            .checked_next()
            .map_err(|_| OwnerError::ContractExhausted)?;
        Ok(Self { revision, state })
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{Blake3Digest32, DataRootId};

    use super::{OwnerSnapshot, OwnerState};
    use crate::{DataRootIdentity, DataRootLocationClass};

    fn root() -> DataRootIdentity {
        DataRootIdentity::new(
            DataRootId::from_bytes([1; 16]),
            DataRootLocationClass::LocalFixed,
            Blake3Digest32::from_bytes([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
        )
    }

    #[test]
    fn snapshots_advance_exactly_once() {
        let first = OwnerSnapshot::new(root()).expect("snapshot");
        let second = first
            .advanced(OwnerState::Vacant {
                root: root(),
                last_epoch: None,
            })
            .expect("advance");
        assert_eq!(first.revision().get(), 1);
        assert_eq!(second.revision().get(), 2);
    }
}
