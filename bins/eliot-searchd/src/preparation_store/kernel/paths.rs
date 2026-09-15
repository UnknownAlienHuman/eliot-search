//! Preparation-tree path derivation and bounded directory admission.

use std::path::{Path, PathBuf};

use super::super::super::storage_io::{
    ensure_child_directory, ensure_directory, sync_directory,
};
use crate::sha256;

pub(crate) fn directories(
    root: &Path,
    key: &[u8; 32],
    create: bool,
) -> Result<(PathBuf, PathBuf), String> {
    ensure_directory(root)?;
    let base = root.join("preparation");
    let refs = base.join("refs");
    let objects = base.join("objects");
    let hex = sha256::hex(key);
    let shard = refs.join(&hex[..2]);
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
    Ok((shard.join(format!("{hex}.ref")), objects))
}
