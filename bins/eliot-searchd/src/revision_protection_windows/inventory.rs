//! Bounded protected-object inventory used before credential creation.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

const MAX_OBJECT_SCAN: usize = 2_000_000;

pub(super) fn contains_protected_objects(root: &Path) -> Result<bool, String> {
    if !root.exists() {
        return Ok(false);
    }
    let mut observed = 0_usize;
    for shard in fs::read_dir(root)
        .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
    {
        let shard = shard
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        let metadata = fs::symlink_metadata(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_REVISION_DIRECTORY_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            continue;
        }
        for entry in fs::read_dir(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
        {
            let entry = entry
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            observed = observed.saturating_add(1);
            if observed > MAX_OBJECT_SCAN {
                return Err("DIRECT_REVISION_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err("DIRECT_REVISION_OBJECT_LINK_DENIED".to_owned());
            }
            if path.extension().is_some_and(|value| {
                value == OsStr::new(super::super::PROTECTED_OBJECT_EXTENSION)
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
