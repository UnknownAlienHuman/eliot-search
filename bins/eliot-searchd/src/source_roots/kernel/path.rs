//! Root/path validation, overlap policy and platform reparse handling.

use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};

use search_source_registry::SourceRootState;

use super::error::SourceRootError;
use super::spec::{MAX_SOURCE_ROOT_PATH_BYTES, MAX_SOURCE_ROOTS};

pub(super) fn canonicalize_configured_set(
    paths: &mut Vec<PathBuf>,
) -> Result<(), SourceRootError> {
    if paths.len() > MAX_SOURCE_ROOTS {
        return Err(SourceRootError::RootLimitExceeded);
    }
    paths.sort();
    paths.dedup();
    for (index, path) in paths.iter().enumerate() {
        validate_persisted_path(path)?;
        ensure_no_overlap(paths[..index].iter(), path)?;
    }
    Ok(())
}

pub(super) fn ensure_no_overlap<'a>(
    existing: impl IntoIterator<Item = &'a PathBuf>,
    candidate: &Path,
) -> Result<(), SourceRootError> {
    if existing.into_iter().any(|root| {
        candidate != root.as_path()
            && (candidate.starts_with(root) || root.starts_with(candidate))
    }) {
        Err(SourceRootError::RootOverlap)
    } else {
        Ok(())
    }
}

pub(super) fn ensure_outside_data_root(
    candidate: &Path,
    data_root: &Path,
) -> Result<(), SourceRootError> {
    if candidate.starts_with(data_root) || data_root.starts_with(candidate) {
        Err(SourceRootError::DataRootOverlap)
    } else {
        Ok(())
    }
}

pub(super) fn canonicalize_new_root(path: &Path) -> Result<PathBuf, SourceRootError> {
    reject_symlink(path)?;
    let canonical = fs::canonicalize(path).map_err(SourceRootError::RootIo)?;
    reject_symlink(&canonical)?;
    if !fs::metadata(&canonical)
        .map_err(SourceRootError::RootIo)?
        .is_dir()
    {
        return Err(SourceRootError::RootNotDirectory);
    }
    validate_persisted_path(&canonical)?;
    Ok(canonical)
}

pub(super) fn probe_root(path: &Path) -> SourceRootState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return SourceRootState::Missing;
        }
        Err(_) => return SourceRootState::Unverifiable,
    };
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return SourceRootState::Unsafe;
    }
    if !metadata.is_dir() {
        return SourceRootState::NotDirectory;
    }
    match fs::canonicalize(path) {
        Ok(canonical) if canonical == path => SourceRootState::Available,
        Ok(_) => SourceRootState::Unsafe,
        Err(_) => SourceRootState::Unverifiable,
    }
}

pub(super) fn validate_persisted_path(path: &Path) -> Result<(), SourceRootError> {
    if !path.is_absolute() {
        return Err(SourceRootError::RootPathNotAbsolute);
    }
    let value = path_text(path)?;
    if value.is_empty()
        || value.len() > MAX_SOURCE_ROOT_PATH_BYTES
        || value.chars().any(char::is_control)
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(SourceRootError::InvalidRootPath);
    }
    Ok(())
}

pub(super) fn path_text(path: &Path) -> Result<&str, SourceRootError> {
    path.to_str().ok_or(SourceRootError::RootPathNotUtf8)
}

pub(super) fn reject_symlink(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
            Err(SourceRootError::SymlinkDenied)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

#[cfg(windows)]
pub(super) fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), SourceRootError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(SourceRootError::ConfigIo)
}

#[cfg(not(unix))]
pub(super) const fn sync_directory(_path: &Path) {
    // Windows power-loss durability needs native qualification; no such
    // receipt is emitted by this observation-registration adapter.
}
