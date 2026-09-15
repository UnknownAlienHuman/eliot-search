use std::path::Path;

use crate::sealed_owner_epoch::OwnerEpochGuard;

use super::super::report::SealedRecoveryReport;
use super::super::spec::SealedRecoveryError;

pub(super) fn recover_all(
    _data_root: &Path,
    _owner: &OwnerEpochGuard,
) -> Result<SealedRecoveryReport, SealedRecoveryError> {
    Err(SealedRecoveryError::UnsupportedPlatform)
}
