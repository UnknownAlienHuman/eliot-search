//! Explicit non-Windows denial for sealed access inventory.

use std::collections::BTreeMap;
use std::path::Path;

use super::super::spec::SealedAccessError;

pub(super) fn discover(
    _data_root: &Path,
    _fence_id: &str,
) -> Result<BTreeMap<u64, String>, SealedAccessError> {
    Err(SealedAccessError::UnsupportedPlatform)
}
