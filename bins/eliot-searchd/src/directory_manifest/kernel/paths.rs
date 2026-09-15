//! Manifest-root containment, discovery and platform path identity.

use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::spec::{
    CONTROL_DIRECTORY, MANIFEST_DIRECTORY, MAX_MANIFEST_FILES,
};

/// ASCII case-insensitive file-name suffix check.
fn has_ascii_suffix(name: &str, suffix: &str) -> bool {
    name.len() >= suffix.len()
        && name.as_bytes()[name.len() - suffix.len()..]
            .eq_ignore_ascii_case(suffix.as_bytes())
}

pub(super) fn existing_manifest_root(
    data_root: &Path,
) -> Result<Option<PathBuf>, String> {
    ensure_directory(data_root)?;
    let control = data_root.join(CONTROL_DIRECTORY);
    ensure_directory(&control)?;
    let root = control.join(MANIFEST_DIRECTORY);
    match fs::symlink_metadata(&root) {
        Ok(_) => {
            ensure_directory(&root)?;
            Ok(Some(root))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("DIRECT_MANIFEST_DIRECTORY_READ_ERROR".to_owned()),
    }
}

pub(super) fn manifest_root(data_root: &Path) -> Result<PathBuf, String> {
    let control = data_root.join(CONTROL_DIRECTORY);
    ensure_directory(&control)?;
    let root = control.join(MANIFEST_DIRECTORY);
    if !root.exists() {
        fs::create_dir(&root).map_err(|error| {
            format!("DIRECT_MANIFEST_DIRECTORY_CREATE_ERROR:{error}")
        })?;
        #[cfg(unix)]
        sync_manifest_directory(&control)?;
        #[cfg(not(unix))]
        sync_manifest_directory(&control);
    }
    ensure_directory(&root)?;
    Ok(root)
}

pub(super) fn manifest_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    list_manifest_files(root, MAX_MANIFEST_FILES, false, None)
}

/// Migration never creates a missing directory or silently drops pending writes.
pub(super) fn migration_files(
    data_root: &Path,
    maximum: usize,
    deadline: Instant,
) -> Result<(bool, Vec<PathBuf>), String> {
    match existing_manifest_root(data_root)? {
        Some(root) => Ok((
            true,
            list_manifest_files(&root, maximum, true, Some(deadline))?,
        )),
        None => Ok((false, Vec::new())),
    }
}

fn list_manifest_files(
    root: &Path,
    maximum: usize,
    reject_pending: bool,
    deadline: Option<Instant>,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(root)
        .map_err(|_| "DIRECT_MANIFEST_DIRECTORY_READ_ERROR".to_owned())?;
    for (index, entry) in entries.enumerate() {
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
        }
        if index >= maximum.min(MAX_MANIFEST_FILES) {
            return Err("DIRECT_MANIFEST_FILE_LIMIT_EXCEEDED".to_owned());
        }
        let entry = entry
            .map_err(|_| "DIRECT_MANIFEST_DIRECTORY_READ_ERROR".to_owned())?;
        let path = entry.path();
        ensure_regular_file(&path)?;
        let file_name = entry.file_name();
        let name = file_name
            .to_str()
            .ok_or_else(|| "DIRECT_MANIFEST_FILENAME_INVALID".to_owned())?;
        if name.starts_with('.') && has_ascii_suffix(name, ".tmp") {
            if reject_pending {
                return Err("DIRECT_MIGRATION_MANIFEST_PENDING".to_owned());
            }
            continue;
        }
        if !has_ascii_suffix(name, ".manifest") {
            return Err("DIRECT_MANIFEST_UNEXPECTED_OBJECT".to_owned());
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

pub(super) fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_MANIFEST_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

pub(super) fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_MANIFEST_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
pub fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
pub fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
pub fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(windows)]
pub(super) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn sync_manifest_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            format!("DIRECT_MANIFEST_DIRECTORY_SYNC_ERROR:{error}")
        })
}

#[cfg(not(unix))]
pub(super) const fn sync_manifest_directory(_path: &Path) {}
