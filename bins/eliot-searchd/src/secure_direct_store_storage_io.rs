//! Qualified daemon adapter for package-owned legacy revision-object I/O.
//!
//! `search-revision-store` owns bounded exact reads, no-clobber publication,
//! native-identity fencing and temporary cleanup for revision objects. Generic
//! preparation artifacts retain their existing local helper until the separate
//! `search-materializer` ownership slice.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use search_revision_store::{
    LegacyRevisionObjectError, LegacyRevisionObjectPlatform,
    publish_legacy_revision_object, read_legacy_revision_object,
};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use crate::sha256;

use super::{RevisionMetadata, verify_plaintext};

const REVISION_DIRECTORY: &str = "revisions";
const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024;

pub(super) fn read_plaintext_path(
    path: &Path,
    metadata: &RevisionMetadata,
) -> Result<Vec<u8>, String> {
    let bytes = read_revision_object(
        path,
        MAX_SCAN_INPUT_BYTES,
        "DIRECT_REVISION_READ_ERROR",
    )?;
    verify_plaintext(metadata, &bytes)?;
    Ok(bytes)
}

/// Generic bounded read retained for preparation-store and other non-revision
/// artifacts. Revision-object callers use [`read_revision_object`].
pub(super) fn read_regular_file(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("{error_prefix}:{error}"))?;
    if metadata.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(format!("{error_prefix}:TOO_LARGE"));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_err(|_| format!("{error_prefix}:TOO_LARGE"))?,
    );
    File::open(path)
        .and_then(|file| {
            file.take(u64::try_from(max_bytes + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("{error_prefix}:{error}"))?;
    if bytes.len() > max_bytes
        || bytes.len() != usize::try_from(metadata.len()).unwrap_or(usize::MAX)
    {
        return Err(format!("{error_prefix}:LENGTH_MISMATCH"));
    }
    Ok(bytes)
}

pub(super) fn read_revision_object(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    read_legacy_revision_object(&DaemonRevisionObjectPlatform, path, max_bytes)
        .map(|observed| observed.into_bytes())
        .map_err(|error| read_reason(error, error_prefix))
}

/// Existing generic immutable publication retained for preparation references
/// and objects until the separate materializer ownership move.
pub(super) fn persist_immutable_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_REVISION_OBJECT_BYTES {
        return Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    ensure_child_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return verify_encoded_object(path, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()),
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_REVISION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let file_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.dpapi.tmp",
        std::process::id(),
        timestamp,
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}"))?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}"));
    }
    drop(file);
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)
                .map_err(|error| format!("DIRECT_REVISION_TEMP_CLEANUP_ERROR:{error}"))?;
            #[cfg(unix)]
            sync_directory(parent)?;
            #[cfg(not(unix))]
            sync_directory(parent);
            verify_encoded_object(path, bytes)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                verify_encoded_object(path, bytes)
            } else {
                Err(format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}"))
            }
        }
    }
}

pub(super) fn persist_revision_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_REVISION_OBJECT_BYTES {
        return Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    let final_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_REVISION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let temporary_name = format!(
        ".{file_stem}.{}.{timestamp}.dpapi.tmp",
        std::process::id()
    );
    let receipt = publish_legacy_revision_object(
        &DaemonRevisionObjectPlatform,
        parent,
        final_name,
        &temporary_name,
        bytes,
        MAX_REVISION_OBJECT_BYTES,
    )
    .map_err(publish_reason)?;
    if receipt.encoded_bytes()
        != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned());
    }
    Ok(())
}

fn verify_encoded_object(path: &Path, expected: &[u8]) -> Result<(), String> {
    let existing = read_regular_file(
        path,
        MAX_REVISION_OBJECT_BYTES,
        "DIRECT_REVISION_PROTECTED_READ_ERROR",
    )?;
    if existing == expected {
        Ok(())
    } else {
        Err("DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned())
    }
}

pub(super) fn remove_plaintext_after_readback(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    ensure_regular_file(path)?;
    fs::remove_file(path)
        .map_err(|error| format!("DIRECT_REVISION_PLAINTEXT_DELETE_ERROR:{error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    #[cfg(unix)]
    {
        sync_directory(parent)
    }
    #[cfg(not(unix))]
    {
        sync_directory(parent);
        Ok(())
    }
}

pub(super) fn legacy_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, "bin")
}

pub(super) fn protected_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, PROTECTED_OBJECT_EXTENSION)
}

fn revision_object_path(
    root: &Path,
    revision_id: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    if sha256::decode_digest(revision_id).is_none() {
        return Err("DIRECT_REVISION_ID_INVALID".to_owned());
    }
    Ok(root
        .join(REVISION_DIRECTORY)
        .join(&revision_id[..2])
        .join(format!("{revision_id}.{extension}")))
}

pub(super) fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

pub(super) fn ensure_child_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path)
            .map_err(|error| format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}"))?;
    }
    ensure_directory(path)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DaemonRevisionObjectPlatform;

impl LegacyRevisionObjectPlatform for DaemonRevisionObjectPlatform {
    type Identity = (u64, u64);
    type Error = String;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        ensure_directory(path)
    }

    fn verify_locator(
        &self,
        expected: &File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        let parent = path
            .parent()
            .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
        ensure_directory(parent)?;
        ensure_regular_file(path)?;
        let current = File::open(path)
            .map_err(|error| format!("DIRECT_REVISION_OBJECT_READ_FAILED:{error}"))?;
        if native_identity(&current)? != native_identity(expected)? {
            return Err("DIRECT_REVISION_OBJECT_CHANGED".to_owned());
        }
        Ok(())
    }

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        #[cfg(unix)]
        {
            sync_directory(path)
        }
        #[cfg(not(unix))]
        {
            sync_directory(path);
            Ok(())
        }
    }
}

fn native_identity(file: &File) -> Result<(u64, u64), String> {
    let metadata = file
        .metadata()
        .map_err(|_| "DIRECT_REVISION_OBJECT_CHANGED".to_owned())?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err("DIRECT_REVISION_OBJECT_CHANGED".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let observed = eliot_searchd::native_file::observe(file)
            .map_err(|_| "DIRECT_REVISION_OBJECT_CHANGED".to_owned())?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("DIRECT_REVISION_OBJECT_PLATFORM_UNSUPPORTED".to_owned())
    }
}

fn read_reason(
    error: LegacyRevisionObjectError<String>,
    prefix: &str,
) -> String {
    match error {
        LegacyRevisionObjectError::Platform(reason) => reason,
        LegacyRevisionObjectError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyRevisionObjectError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyRevisionObjectError::SizeInvalid => format!("{prefix}:TOO_LARGE"),
        LegacyRevisionObjectError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInspect(error) => {
            format!("DIRECT_FILE_METADATA_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyRevisionObjectError::ObjectRead(error) => format!("{prefix}:{error}"),
        LegacyRevisionObjectError::ReadbackMismatch
        | LegacyRevisionObjectError::IdentityChanged => {
            format!("{prefix}:LENGTH_MISMATCH")
        }
        LegacyRevisionObjectError::Create(error)
        | LegacyRevisionObjectError::Write(error)
        | LegacyRevisionObjectError::PublishOutcomeUnknown(error)
        | LegacyRevisionObjectError::Cleanup(error) => format!("{prefix}:{error}"),
        LegacyRevisionObjectError::PublishPlatformOutcomeUnknown(reason) => reason,
        LegacyRevisionObjectError::ImmutableConflict => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
    }
}

fn publish_reason(error: LegacyRevisionObjectError<String>) -> String {
    match error {
        LegacyRevisionObjectError::Platform(reason) => reason,
        LegacyRevisionObjectError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyRevisionObjectError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyRevisionObjectError::SizeInvalid => {
            "DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned()
        }
        LegacyRevisionObjectError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInspect(_) => {
            "DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()
        }
        LegacyRevisionObjectError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyRevisionObjectError::ObjectRead(error) => {
            format!("DIRECT_REVISION_PROTECTED_READ_ERROR:{error}")
        }
        LegacyRevisionObjectError::ReadbackMismatch => {
            "DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned()
        }
        LegacyRevisionObjectError::Create(error) => {
            format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::Write(error) => {
            format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}")
        }
        LegacyRevisionObjectError::PublishOutcomeUnknown(error) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}")
        }
        LegacyRevisionObjectError::PublishPlatformOutcomeUnknown(reason) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{reason}")
        }
        LegacyRevisionObjectError::ImmutableConflict
        | LegacyRevisionObjectError::IdentityChanged => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
        LegacyRevisionObjectError::Cleanup(error) => {
            format!("DIRECT_REVISION_TEMP_CLEANUP_ERROR:{error}")
        }
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
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

#[cfg(test)]
#[path = "immutable_object_tests.rs"]
mod tests;
