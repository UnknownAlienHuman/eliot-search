//! Filesystem lifecycle for the canonical control-cutover marker.
//!
//! This module owns bounded readback, temporary write, durable publication,
//! replay classification and exact post-publication verification. It does not
//! arm quarantine, acquire the external data-root owner, stage a database or
//! switch a serving path. Those policy/orchestration effects remain outside the
//! package.

use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use super::output_lock::SourceImportOutputLockPlatform;
use super::{
    CONTROL_CUTOVER_MARKER_FILE, ControlCutoverMarker,
    MAX_CONTROL_CUTOVER_MARKER_BYTES,
};

/// Exact temporary marker basename inside the admitted `control/` directory.
pub const CONTROL_CUTOVER_MARKER_TEMP_FILE: &str = "control-cutover.tmp";

/// One exact decoded marker next to the canonical bytes observed on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCutoverMarkerFile {
    /// Strictly decoded cutover authority.
    pub marker: ControlCutoverMarker,
    /// Exact canonical bytes read from the marker locator.
    pub bytes: Vec<u8>,
}

/// Read-only state of the canonical marker locator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlCutoverMarkerFileState {
    /// No marker locator exists under an admitted `control/` directory.
    Absent,
    /// One exact canonical marker was read and decoded.
    Valid(Box<ControlCutoverMarkerFile>),
    /// The directory, locator, bytes, identity or marker codec failed closed.
    Corrupt,
}

/// Outcome of one no-repair marker publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCutoverMarkerPublishOutcome {
    /// This call published and read back the exact marker bytes.
    Committed,
    /// Exact canonical bytes were already committed before this call.
    ReplayIdentical,
}

/// Closed marker-artifact lifecycle failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCutoverMarkerArtifactError {
    /// Proposed bytes or the admitted control directory cannot start a write.
    CreateFailed,
    /// A different valid marker already owns the canonical locator.
    AlreadyCommitted,
    /// Existing marker state is malformed or cannot be read coherently.
    Corrupt,
    /// Marker publication may have produced an externally visible effect.
    PublishOutcomeUnknown,
    /// A successful native publication did not read back exact canonical bytes.
    ReadbackMismatch,
}

impl fmt::Display for ControlCutoverMarkerArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateFailed => "cutover marker create failed",
            Self::AlreadyCommitted => "cutover marker already committed",
            Self::Corrupt => "cutover marker corrupt",
            Self::PublishOutcomeUnknown => "cutover marker publish outcome unknown",
            Self::ReadbackMismatch => "cutover marker readback mismatch",
        })
    }
}

impl std::error::Error for ControlCutoverMarkerArtifactError {}

/// Resolves the canonical marker without mutation or repair.
///
/// Missing `control/` or marker state is reported as [`ControlCutoverMarkerFileState::Absent`].
/// Any malformed, unstable, redirected or unreadable existing state is
/// [`ControlCutoverMarkerFileState::Corrupt`].
#[must_use]
pub fn resolve_control_cutover_marker<P>(
    platform: &P,
    data_root: &Path,
) -> ControlCutoverMarkerFileState
where
    P: SourceImportOutputLockPlatform,
{
    let control = data_root.join("control");
    match fs::symlink_metadata(&control) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ControlCutoverMarkerFileState::Absent;
        }
        Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
        _ => return ControlCutoverMarkerFileState::Corrupt,
    }
    if platform.validate_directory(&control).is_err() {
        return ControlCutoverMarkerFileState::Corrupt;
    }
    let marker = control.join(CONTROL_CUTOVER_MARKER_FILE);
    match fs::symlink_metadata(&marker) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ControlCutoverMarkerFileState::Absent
        }
        Err(_) => ControlCutoverMarkerFileState::Corrupt,
        Ok(_) => read_marker(platform, &marker).map_or(
            ControlCutoverMarkerFileState::Corrupt,
            |file| ControlCutoverMarkerFileState::Valid(Box::new(file)),
        ),
    }
}

/// Publishes one canonical marker by temporary write, file sync, directory
/// sync, no-clobber hard-link publication and exact post-publication readback.
///
/// A byte-identical existing marker is an idempotent replay. A different valid
/// marker is never overwritten. After an unclassified hard-link error,
/// observing the proposed bytes cannot prove which attempt committed and
/// therefore remains outcome-unknown.
///
/// # Errors
///
/// Returns a typed failure for invalid input/directory state, a conflicting or
/// corrupt existing marker, uncertain native publication or failed exact
/// post-publication readback.
pub fn publish_control_cutover_marker<P>(
    platform: &P,
    data_root: &Path,
    expected: &[u8],
) -> Result<ControlCutoverMarkerPublishOutcome, ControlCutoverMarkerArtifactError>
where
    P: SourceImportOutputLockPlatform,
{
    if expected.is_empty()
        || expected.len() > MAX_CONTROL_CUTOVER_MARKER_BYTES
        || ControlCutoverMarker::decode(expected).is_err()
    {
        return Err(ControlCutoverMarkerArtifactError::CreateFailed);
    }

    let control = data_root.join("control");
    match fs::symlink_metadata(&control) {
        Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
        _ => return Err(ControlCutoverMarkerArtifactError::CreateFailed),
    }
    platform
        .validate_directory(&control)
        .map_err(|_| ControlCutoverMarkerArtifactError::CreateFailed)?;

    let temporary = control.join(CONTROL_CUTOVER_MARKER_TEMP_FILE);
    let marker = control.join(CONTROL_CUTOVER_MARKER_FILE);
    let _ = fs::remove_file(&temporary);
    match resolve_control_cutover_marker(platform, data_root) {
        ControlCutoverMarkerFileState::Valid(existing) => {
            return classify_existing(&existing, expected);
        }
        ControlCutoverMarkerFileState::Corrupt => {
            return Err(ControlCutoverMarkerArtifactError::Corrupt);
        }
        ControlCutoverMarkerFileState::Absent => {}
    }

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| ControlCutoverMarkerArtifactError::CreateFailed)?;
    let metadata = file
        .metadata()
        .map_err(|_| ControlCutoverMarkerArtifactError::CreateFailed)?;
    if !regular(&metadata)
        || platform.verify_locator(&file, &temporary).is_err()
    {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ControlCutoverMarkerArtifactError::CreateFailed);
    }
    if file
        .write_all(expected)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ControlCutoverMarkerArtifactError::CreateFailed);
    }
    drop(file);
    let _ = platform.sync_directory(&control);

    match read_marker(platform, &temporary) {
        Ok(staged) if staged.bytes == expected => {}
        _ => {
            let _ = fs::remove_file(&temporary);
            return Err(ControlCutoverMarkerArtifactError::CreateFailed);
        }
    }

    match fs::hard_link(&temporary, &marker) {
        Ok(()) => {
            let _ = fs::remove_file(&temporary);
            let _ = platform.sync_directory(&control);
            match read_marker(platform, &marker) {
                Ok(readback) if readback.bytes == expected => {
                    Ok(ControlCutoverMarkerPublishOutcome::Committed)
                }
                _ => Err(ControlCutoverMarkerArtifactError::ReadbackMismatch),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            classify_current(platform, data_root, expected)
        }
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            match classify_current(platform, data_root, expected) {
                Ok(ControlCutoverMarkerPublishOutcome::ReplayIdentical) => {
                    Err(ControlCutoverMarkerArtifactError::PublishOutcomeUnknown)
                }
                other => other,
            }
        }
    }
}

fn classify_current<P>(
    platform: &P,
    data_root: &Path,
    expected: &[u8],
) -> Result<ControlCutoverMarkerPublishOutcome, ControlCutoverMarkerArtifactError>
where
    P: SourceImportOutputLockPlatform,
{
    match resolve_control_cutover_marker(platform, data_root) {
        ControlCutoverMarkerFileState::Absent => {
            Err(ControlCutoverMarkerArtifactError::PublishOutcomeUnknown)
        }
        ControlCutoverMarkerFileState::Corrupt => {
            Err(ControlCutoverMarkerArtifactError::Corrupt)
        }
        ControlCutoverMarkerFileState::Valid(existing) => {
            classify_existing(&existing, expected)
        }
    }
}

fn classify_existing(
    existing: &ControlCutoverMarkerFile,
    expected: &[u8],
) -> Result<ControlCutoverMarkerPublishOutcome, ControlCutoverMarkerArtifactError> {
    if existing.bytes == expected {
        Ok(ControlCutoverMarkerPublishOutcome::ReplayIdentical)
    } else {
        Err(ControlCutoverMarkerArtifactError::AlreadyCommitted)
    }
}

fn read_marker<P>(
    platform: &P,
    path: &Path,
) -> Result<ControlCutoverMarkerFile, ControlCutoverMarkerArtifactError>
where
    P: SourceImportOutputLockPlatform,
{
    let before = fs::symlink_metadata(path)
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    if !regular(&before)
        || before.len() > u64::try_from(MAX_CONTROL_CUTOVER_MARKER_BYTES)
            .unwrap_or(u64::MAX)
    {
        return Err(ControlCutoverMarkerArtifactError::Corrupt);
    }
    let file = File::open(path)
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    let opened = file
        .metadata()
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    if !regular(&opened)
        || opened.len() != before.len()
        || opened.modified().ok() != before.modified().ok()
        || platform.verify_locator(&file, path).is_err()
    {
        return Err(ControlCutoverMarkerArtifactError::Corrupt);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened.len()).unwrap_or(MAX_CONTROL_CUTOVER_MARKER_BYTES),
    );
    let mut reader = file.take(
        u64::try_from(MAX_CONTROL_CUTOVER_MARKER_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
    );
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    let after = reader
        .get_ref()
        .metadata()
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    if bytes.len() != usize::try_from(before.len()).unwrap_or(usize::MAX)
        || bytes.len() > MAX_CONTROL_CUTOVER_MARKER_BYTES
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
        || platform.verify_locator(reader.get_ref(), path).is_err()
    {
        return Err(ControlCutoverMarkerArtifactError::Corrupt);
    }
    let marker = ControlCutoverMarker::decode(&bytes)
        .map_err(|_| ControlCutoverMarkerArtifactError::Corrupt)?;
    Ok(ControlCutoverMarkerFile { marker, bytes })
}

fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !is_link(metadata)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
mod tests;
