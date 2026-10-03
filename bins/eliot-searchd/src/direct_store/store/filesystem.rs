//! Bounded filesystem and path mechanics for the legacy DIRECT store.

use std::fs::{self, Metadata};
#[cfg(unix)]
use std::fs::File;
use std::path::{Path, PathBuf};

use super::super::model::{MAX_DIRECTORY_DEPTH, MAX_DIRECTORY_FILES};

#[cfg(unix)]
pub(in super::super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
pub(in super::super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
pub(in super::super) fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

pub(in super::super) fn collect_regular_files(
    directory: &Path,
    data_root: &Path,
    depth: usize,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if depth > MAX_DIRECTORY_DEPTH {
        return Err("DIRECT_DIRECTORY_DEPTH_EXCEEDED".to_owned());
    }
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_LINK_OR_TYPE_DENIED".to_owned());
    }
    let canonical = fs::canonicalize(directory)
        .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
    if canonical == data_root {
        return Ok(());
    }
    let mut entries = fs::read_dir(&canonical)
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?;
    entries.sort_by_key(|entry| path_identity_bytes(&entry.path()));
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("DIRECT_DIRECTORY_ENTRY_ERROR:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_DIRECTORY_LINK_DENIED".to_owned());
        }
        if metadata.is_dir() {
            let canonical_child = fs::canonicalize(&path)
                .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
            if canonical_child == data_root || canonical_child.starts_with(data_root) {
                continue;
            }
            collect_regular_files(&canonical_child, data_root, depth + 1, output)?;
        } else if metadata.is_file() {
            if output.len() >= MAX_DIRECTORY_FILES {
                return Err("DIRECT_DIRECTORY_FILE_LIMIT_EXCEEDED".to_owned());
            }
            output.push(path);
        } else {
            return Err("DIRECT_DIRECTORY_SPECIAL_OBJECT_DENIED".to_owned());
        }
    }
    Ok(())
}

pub(in super::super) fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

pub(super) fn ensure_child_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path).map_err(|error| format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}"))?;
    }
    ensure_directory(path)
}

pub(in super::super) fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
pub(in super::super) fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(in super::super) fn is_reparse(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
pub(super) const fn sync_directory(_path: &Path) {}
