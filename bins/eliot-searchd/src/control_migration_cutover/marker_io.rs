//! Data-root I/O for the canonical control-cutover marker.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    CONTROL_CUTOVER_MARKER_FILE as CUTOVER_MARKER_FILE,
    ControlCutoverMarker as CutoverMarker,
    MAX_CONTROL_CUTOVER_MARKER_BYTES as MAX_MARKER_BYTES,
};

pub(super) const CUTOVER_MARKER_TMP: &str = "control-cutover.tmp";
pub(super) const CUTOVER_ALREADY_COMMITTED: &str =
    "DIRECT_MIGRATION_CUTOVER_ALREADY_COMMITTED";
pub(super) const CUTOVER_SUPERSEDED: &str =
    "DIRECT_MIGRATION_CUTOVER_SUPERSEDED";
pub(super) const CUTOVER_CORRUPT: &str = "DIRECT_MIGRATION_CUTOVER_CORRUPT";
pub(super) const CUTOVER_CREATE_FAILED: &str =
    "DIRECT_MIGRATION_CUTOVER_CREATE_FAILED";
pub(super) const CUTOVER_OUTCOME_UNKNOWN: &str =
    "DIRECT_MIGRATION_CUTOVER_PUBLISH_OUTCOME_UNKNOWN";
pub(super) const CUTOVER_READBACK_MISMATCH: &str =
    "DIRECT_MIGRATION_CUTOVER_READBACK_MISMATCH";

/// Read-only marker resolution. It never writes or repairs. The caller decides
/// whether a corrupt result may arm quarantine on a mutating path or must stay
/// read-only for status inspection.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum MarkerState {
    Absent,
    Valid(Box<ValidMarker>),
    Corrupt,
}

/// Exact committed bytes next to their decoded authority.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidMarker {
    pub(super) marker: CutoverMarker,
    pub(super) bytes: Vec<u8>,
}

/// Resolves the single authority file without mutation.
pub(super) fn resolve_marker(data_root: &Path) -> MarkerState {
    let path = data_root.join("control").join(CUTOVER_MARKER_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return MarkerState::Absent;
        }
        Err(_) => return MarkerState::Corrupt,
    };
    if !regular(&metadata) || metadata.len() > MAX_MARKER_BYTES as u64 {
        return MarkerState::Corrupt;
    }
    fs::read(&path).map_or(MarkerState::Corrupt, |bytes| {
        CutoverMarker::decode(&bytes).map_or(MarkerState::Corrupt, |marker| {
            MarkerState::Valid(Box::new(ValidMarker { marker, bytes }))
        })
    })
}

/// A committed marker freezes the migrated history. Restaging an identical
/// target/snapshot reproduces evidence; a different history is superseded and
/// a torn marker quarantines rather than being overwritten.
pub(super) fn gate_staging_against_marker(
    data_root: &Path,
    target: SourceNamespaceId,
    snapshot: [u8; 32],
) -> Result<(), String> {
    match resolve_marker(data_root) {
        MarkerState::Absent => Ok(()),
        MarkerState::Valid(committed) => {
            if committed.marker.target == target
                && committed.marker.catalog_snapshot == snapshot
            {
                Ok(())
            } else {
                Err(CUTOVER_SUPERSEDED.to_owned())
            }
        }
        MarkerState::Corrupt => Err(quarantined(data_root, CUTOVER_CORRUPT)),
    }
}

/// Outcome of one marker publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublishOutcome {
    /// This call published the marker.
    Committed,
    /// Exact bytes were already committed; the logical operation ran once.
    ReplayIdentical,
}

/// Publishes exactly one marker using temp/write/sync/rename/readback.
///
/// A lost acknowledgement may replay byte-identical bytes. Any ambiguous
/// native outcome remains `OUTCOME_UNKNOWN`; observing identical bytes after a
/// failed rename cannot prove which attempt landed.
pub(super) fn publish_marker(
    data_root: &Path,
    expected: &[u8],
) -> Result<PublishOutcome, String> {
    if expected.is_empty() || expected.len() > MAX_MARKER_BYTES {
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    let control = data_root.join("control");
    match fs::symlink_metadata(&control) {
        Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
        _ => return Err(CUTOVER_CREATE_FAILED.to_owned()),
    }
    let tmp = control.join(CUTOVER_MARKER_TMP);
    let marker = control.join(CUTOVER_MARKER_FILE);
    let _ = fs::remove_file(&tmp);
    if fs::symlink_metadata(&marker).is_ok() {
        let _ = fs::remove_file(&tmp);
        return classify_existing(&marker, expected, data_root);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|_| CUTOVER_CREATE_FAILED.to_owned())?;
    if file.metadata().map_or(true, |metadata| !regular(&metadata)) {
        let _ = fs::remove_file(&tmp);
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    if file
        .write_all(expected)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&tmp);
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    drop(file);
    sync_directory(&control);
    match fs::rename(&tmp, &marker) {
        Ok(()) => {
            sync_directory(&control);
            match fs::read(&marker) {
                Ok(bytes) if bytes == expected => Ok(PublishOutcome::Committed),
                _ => Err(quarantined(data_root, CUTOVER_READBACK_MISMATCH)),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp);
            classify_existing(&marker, expected, data_root)
        }
        Err(_) => {
            let _ = fs::remove_file(&tmp);
            if fs::symlink_metadata(&marker).is_ok() {
                let outcome = classify_existing(&marker, expected, data_root);
                if outcome == Ok(PublishOutcome::ReplayIdentical) {
                    return Err(CUTOVER_OUTCOME_UNKNOWN.to_owned());
                }
                return outcome;
            }
            Err(CUTOVER_OUTCOME_UNKNOWN.to_owned())
        }
    }
}

fn classify_existing(
    marker: &Path,
    expected: &[u8],
    data_root: &Path,
) -> Result<PublishOutcome, String> {
    match fs::read(marker) {
        Ok(bytes) if bytes == expected => Ok(PublishOutcome::ReplayIdentical),
        Ok(bytes) => match CutoverMarker::decode(&bytes) {
            Ok(_) => Err(CUTOVER_ALREADY_COMMITTED.to_owned()),
            Err(_) => Err(quarantined(data_root, CUTOVER_CORRUPT)),
        },
        Err(_) => Err(CUTOVER_OUTCOME_UNKNOWN.to_owned()),
    }
}

/// Explicit pre-cutover rollback action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RollbackAction {
    Noop,
}

/// Decides rollback without touching any byte except quarantine arming on a
/// corrupt marker. A committed marker is never deleted here.
pub(super) fn check_rollback(data_root: &Path) -> Result<RollbackAction, String> {
    match resolve_marker(data_root) {
        MarkerState::Absent => Ok(RollbackAction::Noop),
        MarkerState::Valid(_) => Err(CUTOVER_ALREADY_COMMITTED.to_owned()),
        MarkerState::Corrupt => Err(quarantined(data_root, CUTOVER_CORRUPT)),
    }
}

/// Arms quarantine and reports `code`. A failed quarantine arm supersedes the
/// original error so fail-closed behavior never masquerades as successful.
pub(super) fn quarantined(data_root: &Path, code: &str) -> String {
    if crate::catalog_quarantine::arm(data_root).is_err() {
        return crate::catalog_quarantine::QUARANTINE_ARM_FAILED.to_owned();
    }
    code.to_owned()
}

pub(super) fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !is_link(metadata)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}
