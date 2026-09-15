//! Explicit fail-closed non-Windows platform boundary.

use std::path::Path;

use super::super::model::{
    DeleteReceipt, SealReceipt, SensitiveBytes, VerifyReceipt,
};
use super::super::spec::SealedStoreError;

pub(crate) fn seal_immutable(
    _data_root: &Path,
    _object_id: &str,
    _plaintext: &SensitiveBytes,
) -> Result<SealReceipt, SealedStoreError> {
    Err(SealedStoreError::UnsupportedPlatform)
}

pub(crate) fn open_sealed(
    _data_root: &Path,
    _object_id: &str,
) -> Result<SensitiveBytes, SealedStoreError> {
    Err(SealedStoreError::UnsupportedPlatform)
}

pub(crate) fn verify_sealed(
    _data_root: &Path,
    _object_id: &str,
) -> Result<VerifyReceipt, SealedStoreError> {
    Err(SealedStoreError::UnsupportedPlatform)
}

pub(crate) fn delete_sealed(
    _data_root: &Path,
    _object_id: &str,
) -> Result<DeleteReceipt, SealedStoreError> {
    Err(SealedStoreError::UnsupportedPlatform)
}
