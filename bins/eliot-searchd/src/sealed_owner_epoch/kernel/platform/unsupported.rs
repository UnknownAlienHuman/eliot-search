//! Explicit non-Windows owner-epoch behavior.

use std::io;
use std::path::Path;

use super::super::codec::OwnerEpochRecord;
use super::super::model::OwnerEpochGuard;
use super::super::spec::{
    OwnerEpochError, SEALED_DIRECTORY, SEALED_SUFFIX,
};

const SEALED_EPOCH_PREFIX: &str = "owner-epoch-";

pub(super) fn acquire(
    _data_root: &Path,
) -> Result<OwnerEpochGuard, OwnerEpochError> {
    Err(OwnerEpochError::UnsupportedPlatform)
}

/// Reports a sealed epoch head without DPAPI: any owner-epoch object is
/// unverifiable on this platform and fails closed at the caller.
pub(super) fn latest_head(
    data_root: &Path,
) -> Result<Option<OwnerEpochRecord>, OwnerEpochError> {
    let directory = data_root.join(SEALED_DIRECTORY);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(OwnerEpochError::IoFailure),
    };
    for entry in entries {
        let entry = entry.map_err(|_| OwnerEpochError::IoFailure)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(epoch_id) = name.strip_suffix(SEALED_SUFFIX) else {
            continue;
        };
        if epoch_id.starts_with(SEALED_EPOCH_PREFIX) {
            return Err(OwnerEpochError::ChainInvalid);
        }
    }
    Ok(None)
}
