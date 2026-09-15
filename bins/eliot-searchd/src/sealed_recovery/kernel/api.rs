//! Owner-guard admission for one bounded startup recovery pass.

use std::path::Path;

use crate::sealed_owner_epoch::OwnerEpochGuard;

use super::platform;
use super::report::SealedRecoveryReport;
use super::spec::SealedRecoveryError;

/// Performs exact bounded reconciliation under the current owner guard.
pub fn recover_all(
    data_root: &Path,
    owner: &OwnerEpochGuard,
) -> Result<SealedRecoveryReport, SealedRecoveryError> {
    if !owner.root_lock_held() || owner.epoch() == 0 {
        return Err(SealedRecoveryError::OwnerGuardRequired);
    }
    platform::recover_all(data_root, owner)
}
