//! Bounded existing owner-object reads with exact same-handle readback.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

use search_runtime_owner::OwnerError;

use super::codec::is_reparse;

pub(super) fn read_existing_bytes(
    path: &Path,
    ceiling: usize,
) -> Result<Option<Vec<u8>>, OwnerError> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(OwnerError::OwnerRecoveryQuarantined),
    };
    if !before.is_file()
        || before.file_type().is_symlink()
        || is_reparse(&before)
        || before.len() > ceiling as u64
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let mut file = open_read_existing(path)?;
    let opened = file
        .metadata()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !opened.is_file() || is_reparse(&opened) || opened.len() > ceiling as u64 {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let limit = u64::try_from(ceiling)
        .map_err(|_| OwnerError::ContractExhausted)?
        .checked_add(1)
        .ok_or(OwnerError::ContractExhausted)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    let after = file
        .metadata()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if bytes.len() > ceiling
        || bytes.len() as u64 != opened.len()
        || after.len() != opened.len()
        || after.modified().ok() != opened.modified().ok()
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let named = fs::symlink_metadata(path).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !named.is_file() || named.file_type().is_symlink() || is_reparse(&named) {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let current = open_read_existing(path)?;
    let current_metadata = current
        .metadata()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !current_metadata.is_file() || is_reparse(&current_metadata) {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    verify_same_object(&file, &current)?;
    Ok(Some(bytes))
}

fn open_read_existing(path: &Path) -> Result<File, OwnerError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    options
        .open(path)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn verify_same_object(original: &File, current: &File) -> Result<(), OwnerError> {
    #[cfg(windows)]
    {
        let original = eliot_searchd::native_file::observe(original)
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let current = eliot_searchd::native_file::observe(current)
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        if original.volume_serial != current.volume_serial
            || original.file_index != current.file_index
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let original = original
            .metadata()
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let current = current
            .metadata()
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        if original.dev() != current.dev() || original.ino() != current.ino() {
            return Err(OwnerError::OwnerGuardMismatch);
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (original, current);
        return Err(OwnerError::DataRootInvalid);
    }
    Ok(())
}

/// Retains the physical root; Windows sharing denies renaming/deletion while
/// admitted children are live. Other targets still verify exact native identity.
pub(crate) fn open_bound_directory(path: &Path) -> Result<File, OwnerError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| OwnerError::DataRootInvalid)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(OwnerError::DataRootInvalid);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0220_0000).share_mode(0x3);
    }
    let file = options
        .open(path)
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let opened = file.metadata().map_err(|_| OwnerError::DataRootInvalid)?;
    if !opened.is_dir() || is_reparse(&opened) {
        return Err(OwnerError::DataRootInvalid);
    }
    Ok(file)
}

pub(crate) fn verify_bound_directory(file: &File, path: &Path) -> Result<(), OwnerError> {
    let current = open_bound_directory(path)?;
    verify_same_object(file, &current)
}

/// Checks the named exclusion without reading or changing its locked bytes.
pub(crate) fn verify_existing_locator(file: &File, path: &Path) -> Result<(), OwnerError> {
    let named = fs::symlink_metadata(path).map_err(|_| OwnerError::OwnerGuardMismatch)?;
    let held = file
        .metadata()
        .map_err(|_| OwnerError::OwnerGuardMismatch)?;
    if !named.is_file()
        || named.file_type().is_symlink()
        || is_reparse(&named)
        || !held.is_file()
        || is_reparse(&held)
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let current = open_read_existing(path)?;
    verify_same_object(file, &current)
}
