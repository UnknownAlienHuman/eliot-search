//! Data-root policy adapter for the package-owned control-cutover marker.
//!
//! `search-control-redb::migration` owns all marker file reads, temporary
//! writes, publication classification and exact readback. This adapter retains
//! only stable daemon reason mapping and the decision to arm catalog quarantine.

use std::path::Path;

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    ControlCutoverMarkerArtifactError, publish_control_cutover_marker,
    resolve_control_cutover_marker,
};

use super::super::redb_import::DaemonImportOutputPlatform;

pub(super) use search_control_redb::migration::{
    CONTROL_CUTOVER_MARKER_TEMP_FILE as CUTOVER_MARKER_TMP,
    ControlCutoverMarkerFileState as MarkerState,
    ControlCutoverMarkerPublishOutcome as PublishOutcome,
};

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

/// Resolves the single authority file without mutation or repair.
pub(super) fn resolve_marker(data_root: &Path) -> MarkerState {
    resolve_control_cutover_marker(&DaemonImportOutputPlatform, data_root)
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

/// Publishes exactly one canonical marker through the package owner.
///
/// A lost acknowledgement may replay byte-identical bytes. Any ambiguous
/// native outcome remains `OUTCOME_UNKNOWN`; observing identical bytes after an
/// unclassified publish error cannot prove which attempt landed.
pub(super) fn publish_marker(
    data_root: &Path,
    expected: &[u8],
) -> Result<PublishOutcome, String> {
    publish_control_cutover_marker(
        &DaemonImportOutputPlatform,
        data_root,
        expected,
    )
    .map_err(|error| marker_error(data_root, error))
}

/// Explicit pre-cutover rollback action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RollbackAction {
    Noop,
}

/// Decides rollback without touching any byte except quarantine arming on a
/// corrupt marker. A committed marker is never deleted here.
pub(super) fn check_rollback(
    data_root: &Path,
) -> Result<RollbackAction, String> {
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

fn marker_error(
    data_root: &Path,
    error: ControlCutoverMarkerArtifactError,
) -> String {
    match error {
        ControlCutoverMarkerArtifactError::CreateFailed => {
            CUTOVER_CREATE_FAILED.to_owned()
        }
        ControlCutoverMarkerArtifactError::AlreadyCommitted => {
            CUTOVER_ALREADY_COMMITTED.to_owned()
        }
        ControlCutoverMarkerArtifactError::Corrupt => {
            quarantined(data_root, CUTOVER_CORRUPT)
        }
        ControlCutoverMarkerArtifactError::PublishOutcomeUnknown => {
            CUTOVER_OUTCOME_UNKNOWN.to_owned()
        }
        ControlCutoverMarkerArtifactError::ReadbackMismatch => {
            quarantined(data_root, CUTOVER_READBACK_MISMATCH)
        }
    }
}
