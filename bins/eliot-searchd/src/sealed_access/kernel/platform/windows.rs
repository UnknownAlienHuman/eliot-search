//! Windows sealed-object inventory for one access-fence chain.

use std::collections::BTreeMap;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

use super::super::spec::{
    MAX_ACCESS_FENCE_GENERATIONS, SEALED_DIRECTORY, SEALED_SUFFIX,
    SealedAccessError,
};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

pub(super) fn discover(
    data_root: &Path,
    fence_id: &str,
) -> Result<BTreeMap<u64, String>, SealedAccessError> {
    let directory = data_root.join(SEALED_DIRECTORY);
    if !directory.exists() {
        return Ok(BTreeMap::new());
    }
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|_| SealedAccessError::IoFailure)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(SealedAccessError::ChainInvalid);
    }
    let prefix = format!("access-fence-{fence_id}-");
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(&directory)
        .map_err(|_| SealedAccessError::IoFailure)?
    {
        let entry = entry.map_err(|_| SealedAccessError::IoFailure)?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }
        let entry_metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| SealedAccessError::IoFailure)?;
        if !entry_metadata.is_file()
            || entry_metadata.file_type().is_symlink()
            || entry_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let object_id = file_name
            .strip_suffix(SEALED_SUFFIX)
            .ok_or(SealedAccessError::ChainInvalid)?;
        let generation_text = object_id
            .strip_prefix(&prefix)
            .ok_or(SealedAccessError::ChainInvalid)?;
        if generation_text.len() != 20
            || !generation_text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let generation = generation_text
            .parse::<u64>()
            .map_err(|_| SealedAccessError::ChainInvalid)?;
        if generation == 0
            || format!("{generation:020}") != generation_text
            || records.insert(generation, object_id.to_owned()).is_some()
            || records.len() > MAX_ACCESS_FENCE_GENERATIONS
        {
            return Err(SealedAccessError::ChainInvalid);
        }
    }
    Ok(records)
}
