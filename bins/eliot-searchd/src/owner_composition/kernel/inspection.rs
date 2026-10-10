//! Side-effect-free owner classification under the existing native exclusions.

use std::path::Path;

use search_contracts::{DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_runtime_owner::{
    DataRootOpenMode, OwnerError, RootOpenDecision, RootOpenState, classify_root_open,
};

use super::installation::load_existing_installation;
use super::observation::{observe_executable, observe_physical_root};
use super::record::DurableOwnerRecord;
use super::slots::newest_valid;
use super::spec::LifecycleState;

/// Exact existing durable owner binding. This observation cannot authorize
/// succession or mutate a root; its constructor performs existing reads only.
pub struct ExistingOwnerSnapshot {
    record: Box<DurableOwnerRecord>,
    epoch: OwnerEpoch,
}

impl ExistingOwnerSnapshot {
    pub(super) fn matches_record(&self, record: &DurableOwnerRecord) -> bool {
        self.record.as_ref() == record
    }

    /// Returns a read binding only for an exact cleanly released predecessor.
    pub(crate) fn require_released(&self) -> Result<(), OwnerError> {
        self.require_decision(
            DataRootOpenMode::InspectExistingReadOnly,
            RootOpenDecision::ReadExisting,
        )
    }

    pub(crate) fn require_existing_mutation(&self) -> Result<(), OwnerError> {
        self.require_decision(
            DataRootOpenMode::OpenExistingMutating,
            RootOpenDecision::RequireLiveOwner,
        )
    }

    fn require_decision(
        &self,
        mode: DataRootOpenMode,
        expected: RootOpenDecision,
    ) -> Result<(), OwnerError> {
        let state = if self.record.lifecycle == LifecycleState::Released {
            RootOpenState::Initialized
        } else {
            RootOpenState::OutcomeUnknown
        };
        // Acquiring the OS lock does not resolve an abandoned operation.
        if classify_root_open(mode, state) == expected {
            Ok(())
        } else {
            Err(OwnerError::OwnerRecoveryQuarantined)
        }
    }

    /// Exact incarnation/root/epoch of the existing record, without source data.
    pub(crate) fn journal_owner_inputs(
        &self,
    ) -> (InstallationIncarnationId, DataRootId, OwnerEpoch) {
        (
            InstallationIncarnationId::from_bytes(self.record.installation_incarnation_id),
            DataRootId::from_bytes(self.record.data_root_id),
            self.epoch,
        )
    }

    /// Exact existing durable owner-record generation.
    pub(crate) const fn generation(&self) -> u64 {
        self.record.generation
    }

    /// Refuses externally changed durable state before a read capability ends.
    pub(crate) fn verify_unchanged(&self, root: &Path) -> Result<(), OwnerError> {
        if inspect_existing_owner(root)?.record == self.record {
            Ok(())
        } else {
            Err(OwnerError::OwnerGuardMismatch)
        }
    }
}

/// Reads and verifies existing installation, physical root, executable and
/// durable owner slots. Never creates files, advances epochs or repairs state.
/// The caller must co-hold the existing primary and sealed OS exclusions.
pub(crate) fn inspect_existing_owner(root: &Path) -> Result<ExistingOwnerSnapshot, OwnerError> {
    let installation = load_existing_installation(root)?;
    let observed = observe_physical_root(root)?;
    let executable = observe_executable()?;
    let (_, prior) = newest_valid(root)?;
    let record = prior.ok_or(OwnerError::OwnerRecoveryEvidenceMissing)?;
    record.validate_shape()?;
    if record.installation_id != installation.installation_id
        || record.installation_incarnation_id != installation.installation_incarnation_id
        || record.data_root_id != *observed.data_root_id.as_bytes()
        || record.canonical_path_digest != observed.canonical_path_digest
        || record.volume_identity_digest != observed.volume_identity_digest
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    if record.executable_digest != executable {
        return Err(OwnerError::OwnerExecutableIdentityMismatch);
    }
    let epoch = OwnerEpoch::new(record.epoch).map_err(|_| OwnerError::OwnerIdentityAmbiguous)?;
    Ok(ExistingOwnerSnapshot { record, epoch })
}
