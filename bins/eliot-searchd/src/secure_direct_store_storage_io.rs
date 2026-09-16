//! Qualified daemon adapters for package-owned immutable object I/O.
//!
//! `search-revision-store` owns revision-object reads/publication and
//! `search-materializer` owns preparation-object/reference reads/publication.
//! This module retains legacy path derivation, native platform observations,
//! temporary-name entropy and stable DIRECT reason mapping.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::fs::{self, File, Metadata};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use search_materializer::{
    LegacyPreparationArtifactError, LegacyPreparationArtifactPlatform,
    publish_legacy_preparation_artifact, read_legacy_preparation_artifact,
};
use search_revision_store::{
    LegacyRevisionObjectError, LegacyRevisionObjectPlatform,
    publish_legacy_revision_object, read_legacy_revision_object,
};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use crate::sha256;

use super::{
    MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, RevisionMetadata,
    verify_plaintext,
};

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

pub(super) fn read_regular_file(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    read_legacy_preparation_artifact(&DaemonImmutableObjectPlatform, path, max_bytes)
        .map(|observed| observed.into_bytes())
        .map_err(|error| preparation_read_reason(error, error_prefix))
}

pub(super) fn read_revision_object(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    read_legacy_revision_object(&DaemonImmutableObjectPlatform, path, max_bytes)
        .map(|observed| observed.into_bytes())
        .map_err(|error| revision_read_reason(error, error_prefix))
}

pub(super) fn persist_immutable_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
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
    let temporary_name = temporary_name(file_stem)?;
    let receipt = publish_legacy_preparation_artifact(
        &DaemonImmutableObjectPlatform,
        parent,
        final_name,
        &temporary_name,
        bytes,
        MAX_REVISION_OBJECT_BYTES,
    )
    .map_err(preparation_publish_reason)?;
    if receipt.encoded_bytes()
        != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned());
    }
    Ok(())
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
    let temporary_name = temporary_name(file_stem)?;
    let receipt = publish_legacy_revision_object(
        &DaemonImmutableObjectPlatform,
        parent,
        final_name,
        &temporary_name,
        bytes,
        MAX_REVISION_OBJECT_BYTES,
    )
    .map_err(revision_publish_reason)?;
    if receipt.encoded_bytes()
        != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned());
    }
    Ok(())
}

fn temporary_name(file_stem: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_REVISION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    Ok(format!(
        ".{file_stem}.{}.{timestamp}.dpapi.tmp",
        std::process::id()
    ))
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
struct DaemonImmutableObjectPlatform;

impl LegacyRevisionObjectPlatform for DaemonImmutableObjectPlatform {
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
        verify_locator(expected, path)
    }

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        sync_directory_result(path)
    }
}

impl LegacyPreparationArtifactPlatform for DaemonImmutableObjectPlatform {
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
        verify_locator(expected, path)
    }

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        sync_directory_result(path)
    }
}

fn verify_locator(expected: &File, path: &Path) -> Result<(), String> {
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

fn revision_read_reason(
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
        LegacyRevisionObjectError::ReadbackMismatch => {
            format!("{prefix}:LENGTH_MISMATCH")
        }
        LegacyRevisionObjectError::IdentityChanged => {
            "DIRECT_REVISION_OBJECT_CHANGED".to_owned()
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

fn revision_publish_reason(error: LegacyRevisionObjectError<String>) -> String {
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

fn preparation_read_reason(
    error: LegacyPreparationArtifactError<String>,
    prefix: &str,
) -> String {
    match error {
        LegacyPreparationArtifactError::Platform(reason) => reason,
        LegacyPreparationArtifactError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyPreparationArtifactError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::SizeInvalid => format!("{prefix}:TOO_LARGE"),
        LegacyPreparationArtifactError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInspect(error) => {
            format!("DIRECT_FILE_METADATA_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyPreparationArtifactError::ObjectRead(error) => format!("{prefix}:{error}"),
        LegacyPreparationArtifactError::ReadbackMismatch => {
            format!("{prefix}:LENGTH_MISMATCH")
        }
        LegacyPreparationArtifactError::IdentityChanged => {
            "DIRECT_REVISION_OBJECT_CHANGED".to_owned()
        }
        LegacyPreparationArtifactError::Create(error)
        | LegacyPreparationArtifactError::Write(error)
        | LegacyPreparationArtifactError::PublishOutcomeUnknown(error)
        | LegacyPreparationArtifactError::Cleanup(error) => format!("{prefix}:{error}"),
        LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown(reason) => reason,
        LegacyPreparationArtifactError::ImmutableConflict => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
    }
}

fn preparation_publish_reason(
    error: LegacyPreparationArtifactError<String>,
) -> String {
    match error {
        LegacyPreparationArtifactError::Platform(reason) => reason,
        LegacyPreparationArtifactError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyPreparationArtifactError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::SizeInvalid => {
            "DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInspect(_) => {
            "DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()
        }
        LegacyPreparationArtifactError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyPreparationArtifactError::ObjectRead(error) => {
            format!("DIRECT_REVISION_PROTECTED_READ_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ReadbackMismatch => {
            "DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned()
        }
        LegacyPreparationArtifactError::Create(error) => {
            format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::Write(error) => {
            format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::PublishOutcomeUnknown(error) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}")
        }
        LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown(reason) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{reason}")
        }
        LegacyPreparationArtifactError::ImmutableConflict
        | LegacyPreparationArtifactError::IdentityChanged => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
        LegacyPreparationArtifactError::Cleanup(error) => {
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

fn sync_directory_result(path: &Path) -> Result<(), String> {
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
