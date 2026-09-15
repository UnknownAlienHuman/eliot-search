//! Process-local live owner authority and guarded drain/release lifecycle.

use std::path::PathBuf;

use search_contracts::{DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_runtime_owner::{DrainReason, OwnerError};

use super::record::DurableOwnerRecord;
use super::slots::publish_transition;
use super::spec::{DrainReasonText, LifecycleState};

/// Process-local live owner authority: non-cloneable by construction.
pub struct LiveOwner {
    pub(super) canonical_root: PathBuf,
    pub(super) installation_incarnation_id: InstallationIncarnationId,
    pub(super) data_root_id: DataRootId,
    pub(super) epoch: OwnerEpoch,
    pub(super) record: DurableOwnerRecord,
    pub(super) recovered_previous_active: bool,
    pub(super) poisoned: bool,
}

impl core::fmt::Debug for LiveOwner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LiveOwner")
            .field("data_root_id", &self.data_root_id)
            .field("owner_epoch", &self.epoch)
            .field("generation", &self.record.generation)
            .field("lifecycle", &self.record.lifecycle)
            .field("recovered_previous_active", &self.recovered_previous_active)
            .field("owner_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Content-free proof that one exact owner released cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReceipt {
    /// Released owner epoch.
    pub epoch: OwnerEpoch,
    /// Final durable record generation.
    pub generation: u64,
    /// Digest of the exact `RELEASED` record bytes.
    pub record_digest: [u8; 32],
}

impl LiveOwner {
    /// Bound monotone owner epoch.
    #[must_use]
    pub const fn epoch(&self) -> OwnerEpoch {
        self.epoch
    }

    /// Whether the predecessor held an unreleased (`ACTIVE`/`DRAINING`)
    /// record; a clean `RELEASED` tombstone reports false.
    #[must_use]
    pub const fn recovered_previous_active(&self) -> bool {
        self.recovered_previous_active
    }

    /// Exact owner-side journal identity inputs for the redb follow-up.
    #[must_use]
    pub const fn journal_owner_inputs(
        &self,
    ) -> (InstallationIncarnationId, DataRootId, OwnerEpoch) {
        (
            self.installation_incarnation_id,
            self.data_root_id,
            self.epoch,
        )
    }

    /// Persists `DRAINING` intent under the live lock; idempotent.
    pub(crate) fn begin_drain(&mut self, reason: DrainReason) -> Result<(), OwnerError> {
        if self.poisoned {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        if self.record.lifecycle == LifecycleState::Draining {
            return Ok(());
        }
        if self.record.lifecycle != LifecycleState::Active {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        let next = self.transition_record(
            LifecycleState::Draining,
            DrainReasonText::from_policy(reason),
        )?;
        match publish_transition(&self.canonical_root, &self.record, &next) {
            Ok(()) => {
                self.record = next;
                Ok(())
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    /// Persists the `RELEASED` tombstone under the live lock.
    ///
    /// Release without a prior drain, a poisoned guard, or an unrecoverable
    /// durable contradiction fails closed; a possible write whose readback
    /// cannot be proven reports the unknown outcome.
    pub(crate) fn release_cleanly(&mut self) -> Result<ShutdownReceipt, OwnerError> {
        if self.poisoned {
            return Err(OwnerError::OwnerReleaseOutcomeUnknown);
        }
        if self.record.lifecycle != LifecycleState::Draining {
            return Err(OwnerError::OwnerDrainRequired);
        }
        let next = self.transition_record(LifecycleState::Released, DrainReasonText::None)?;
        match publish_transition(&self.canonical_root, &self.record, &next) {
            Ok(()) => {
                self.record = next.clone();
                Ok(ShutdownReceipt {
                    epoch: self.epoch,
                    generation: next.generation,
                    record_digest: next.record_digest,
                })
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn transition_record(
        &self,
        lifecycle: LifecycleState,
        drain_reason: DrainReasonText,
    ) -> Result<DurableOwnerRecord, OwnerError> {
        let generation = self
            .record
            .generation
            .checked_add(1)
            .ok_or(OwnerError::ContractExhausted)?;
        let mut next = self.record.clone();
        next.lifecycle = lifecycle;
        next.drain_reason = drain_reason;
        next.generation = generation;
        next.refresh_digest();
        Ok(next)
    }
}
