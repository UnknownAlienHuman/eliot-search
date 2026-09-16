//! Preparation-tree platform admission around package-owned locator names.

use std::path::{Path, PathBuf};

use search_materializer::api::{
    LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_OBJECTS_DIRECTORY,
    LEGACY_PREPARATION_REFERENCES_DIRECTORY,
    legacy_preparation_reference_file_name, legacy_preparation_shard,
};

use super::super::super::storage_io::{
    ensure_child_directory, ensure_directory, sync_directory,
};

pub(crate) fn directories(
    root: &Path,
    key: &[u8; 32],
    create: bool,
) -> Result<(PathBuf, PathBuf), String> {
    ensure_directory(root)?;
    let base = root.join(LEGACY_PREPARATION_DIRECTORY);
    let refs = base.join(LEGACY_PREPARATION_REFERENCES_DIRECTORY);
    let objects = base.join(LEGACY_PREPARATION_OBJECTS_DIRECTORY);
    let shard = refs.join(legacy_preparation_shard(key));
    for path in [&base, &refs, &objects, &shard] {
        if create {
            ensure_child_directory(path)?;
            #[cfg(unix)]
            sync_directory(
                path.parent()
                    .ok_or_else(|| {
                        "DIRECT_PREPARATION_PARENT_INVALID".to_owned()
                    })?,
            )?;
            #[cfg(not(unix))]
            sync_directory(
                path.parent()
                    .ok_or_else(|| {
                        "DIRECT_PREPARATION_PARENT_INVALID".to_owned()
                    })?,
            );
        } else {
            ensure_directory(path)?;
        }
    }
    Ok((
        shard.join(legacy_preparation_reference_file_name(key)),
        objects,
    ))
}
