//! Stable sealed-store operations delegating to the qualified platform owner.

use std::path::Path;

use super::model::{DeleteReceipt, SealReceipt, SensitiveBytes, VerifyReceipt};
use super::platform;
use super::spec::SealedStoreError;

/// Protects plaintext and creates one immutable sealed object.
pub fn seal_immutable(
    data_root: &Path,
    object_id: &str,
    plaintext: &SensitiveBytes,
) -> Result<SealReceipt, SealedStoreError> {
    platform::seal_immutable(data_root, object_id, plaintext)
}

/// Opens and authenticates one sealed object.
pub fn open_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<SensitiveBytes, SealedStoreError> {
    platform::open_sealed(data_root, object_id)
}

/// Authenticates one sealed object without returning plaintext to the caller.
pub fn verify_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<VerifyReceipt, SealedStoreError> {
    platform::verify_sealed(data_root, object_id)
}

/// Removes one sealed-object directory entry without claiming physical erasure.
pub fn delete_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<DeleteReceipt, SealedStoreError> {
    platform::delete_sealed(data_root, object_id)
}
