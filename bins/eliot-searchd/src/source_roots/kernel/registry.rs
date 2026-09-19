//! Registration-file I/O, crash recovery and migration snapshot.
//!
//! Exact legacy schema/framing belongs to `search-source-registry`; this
//! daemon module retains qualified filesystem reads, atomic replacement,
//! platform path validation and recovery composition.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use search_source_registry::{
    LegacySourceRootCatalogError, decode_legacy_source_root_catalog,
    encode_legacy_source_root_catalog,
};

use super::error::SourceRootError;
use super::model::SourceRootEntry;
use super::path::{
    canonicalize_configured_set, ensure_outside_data_root, is_reparse,
    path_text, reject_symlink, sync_directory, validate_persisted_path,
};
use super::spec::{MAX_SOURCE_ROOT_FILE_BYTES, MAX_SOURCE_ROOTS};

pub(super) fn load_configured_paths(path: &Path) -> Result<Vec<PathBuf>, SourceRootError> {
    read_configured_bytes(path)?
        .as_deref()
        .map(decode_configured_paths)
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(super) fn read_configured_bytes(
    path: &Path,
) -> Result<Option<Vec<u8>>, SourceRootError> {
    reject_symlink(path)?;
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SourceRootError::ConfigIo(error)),
    };
    let metadata = file.metadata().map_err(SourceRootError::ConfigIo)?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(SourceRootError::InvalidConfigPath);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(MAX_SOURCE_ROOT_FILE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(SourceRootError::ConfigIo)?;
    if bytes.len() > MAX_SOURCE_ROOT_FILE_BYTES {
        return Err(SourceRootError::ConfigTooLarge);
    }
    let after = file.metadata().map_err(SourceRootError::ConfigIo)?;
    if bytes.len() as u64 != metadata.len()
        || metadata.len() != after.len()
        || metadata.modified().ok() != after.modified().ok()
    {
        return Err(SourceRootError::CatalogCorrupt);
    }
    Ok(Some(bytes))
}

pub(super) fn decode_configured_paths(
    bytes: &[u8],
) -> Result<Vec<PathBuf>, SourceRootError> {
    let roots = decode_legacy_source_root_catalog(bytes).map_err(map_codec_error)?;
    let mut paths = Vec::with_capacity(roots.len());
    for root in roots {
        let path = PathBuf::from(root);
        validate_persisted_path(&path)?;
        paths.push(path);
    }
    Ok(paths)
}

pub(super) fn persist_entries(
    path: &Path,
    entries: &[SourceRootEntry],
) -> Result<(), SourceRootError> {
    if entries.len() > MAX_SOURCE_ROOTS {
        return Err(SourceRootError::RootLimitExceeded);
    }
    let parent = path.parent().ok_or(SourceRootError::InvalidConfigPath)?;
    reject_symlink(parent)?;
    fs::create_dir_all(parent).map_err(SourceRootError::ConfigIo)?;
    reject_symlink(path)?;

    let mut root_text = Vec::with_capacity(entries.len());
    let mut expected = Vec::with_capacity(entries.len());
    for entry in entries {
        validate_persisted_path(&entry.configured_path)?;
        root_text.push(path_text(&entry.configured_path)?);
        expected.push(entry.configured_path.clone());
    }
    let body = encode_legacy_source_root_catalog(root_text).map_err(map_codec_error)?;

    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    remove_plain_file_if_present(&temporary)?;
    reject_symlink(&backup)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(SourceRootError::ConfigIo)?;
    file.write_all(&body)
        .and_then(|()| file.sync_all())
        .map_err(SourceRootError::ConfigIo)?;
    drop(file);
    if load_configured_paths(&temporary)? != expected {
        return Err(SourceRootError::CatalogCorrupt);
    }
    remove_plain_file_if_present(&backup)?;
    if path.try_exists().map_err(SourceRootError::ConfigIo)? {
        fs::rename(path, &backup).map_err(SourceRootError::ConfigIo)?;
    }
    fs::rename(&temporary, path).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    #[cfg(unix)]
    sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    #[cfg(not(unix))]
    sync_directory(parent);
    if load_configured_paths(path).map_err(|_| SourceRootError::UpdateOutcomeUnknown)? != expected {
        return Err(SourceRootError::UpdateOutcomeUnknown);
    }
    remove_plain_file_if_present(&backup).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    #[cfg(unix)]
    {
        sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)
    }
    #[cfg(not(unix))]
    {
        sync_directory(parent);
        Ok(())
    }
}

pub(super) fn recover_interrupted_update(path: &Path) -> Result<(), SourceRootError> {
    let backup = path.with_extension("bak");
    let temporary = path.with_extension("tmp");
    reject_symlink(path)?;
    reject_symlink(&backup)?;
    reject_symlink(&temporary)?;
    let current_exists = path.try_exists().map_err(SourceRootError::ConfigIo)?;
    let backup_exists = backup.try_exists().map_err(SourceRootError::ConfigIo)?;
    if current_exists {
        // Never replace a corrupt current catalog with a silently older one.
        load_configured_paths(path)?;
        remove_plain_file_if_present(&backup)?;
    } else if backup_exists {
        load_configured_paths(&backup)?;
        fs::rename(&backup, path).map_err(SourceRootError::ConfigIo)?;
        #[cfg(unix)]
        sync_directory(path.parent().ok_or(SourceRootError::InvalidConfigPath)?)?;
        #[cfg(not(unix))]
        sync_directory(path.parent().ok_or(SourceRootError::InvalidConfigPath)?);
    }
    remove_plain_file_if_present(&temporary)
}

fn remove_plain_file_if_present(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
            Err(SourceRootError::SymlinkDenied)
        }
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(path).map_err(SourceRootError::ConfigIo)
        }
        Ok(_) => Err(SourceRootError::InvalidConfigPath),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

fn map_codec_error(error: LegacySourceRootCatalogError) -> SourceRootError {
    match error {
        LegacySourceRootCatalogError::TooManyRoots => SourceRootError::RootLimitExceeded,
        LegacySourceRootCatalogError::FileTooLarge => SourceRootError::ConfigTooLarge,
        LegacySourceRootCatalogError::NotUtf8 => SourceRootError::ConfigNotUtf8,
        LegacySourceRootCatalogError::MissingFinalNewline
        | LegacySourceRootCatalogError::InvalidHeader
        | LegacySourceRootCatalogError::DuplicateRoot => SourceRootError::CatalogCorrupt,
    }
}

/// Exact persisted registration input. No current-path probes, recovery or
/// new owner. Paths are retained internally for a future importer.
#[derive(Eq, PartialEq)]
pub struct RootMigrationInput {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) file_bytes: Option<Vec<u8>>,
}

/// Reads registration without invoking catalog recovery or acquiring an owner.
pub fn migration_input(data_root: &Path) -> Result<RootMigrationInput, SourceRootError> {
    reject_symlink(data_root)?;
    let canonical = fs::canonicalize(data_root).map_err(SourceRootError::RootIo)?;
    let control = canonical.join("control");
    reject_symlink(&control)?;
    if !fs::symlink_metadata(&control)
        .map_err(SourceRootError::ConfigIo)?
        .is_dir()
        || fs::canonicalize(&control).map_err(SourceRootError::ConfigIo)? != control
    {
        return Err(SourceRootError::InvalidConfigPath);
    }
    let path = control.join("source-roots.v1");
    for pending in [path.with_extension("tmp"), path.with_extension("bak")] {
        match fs::symlink_metadata(pending) {
            Ok(_) => return Err(SourceRootError::UpdateOutcomeUnknown),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(SourceRootError::ConfigIo(error)),
        }
    }
    let file_bytes = read_configured_bytes(&path)?;
    let mut paths = file_bytes
        .as_deref()
        .map(decode_configured_paths)
        .transpose()?
        .unwrap_or_default();
    canonicalize_configured_set(&mut paths)?;
    for path in &paths {
        ensure_outside_data_root(path, &canonical)?;
    }
    Ok(RootMigrationInput { paths, file_bytes })
}
