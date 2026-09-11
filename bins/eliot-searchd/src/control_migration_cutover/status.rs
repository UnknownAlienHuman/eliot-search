//! Read-only cutover status projection.

use std::fs;
use std::path::Path;

use super::marker_io::{MarkerState, regular, resolve_marker};
use super::super::{json_string, sha256};

/// Renders bounded read-only authority status.
///
/// The operation performs one marker read plus at most one staged-database
/// stat. It never replays the journal and never writes or repairs state.
pub(super) fn cutover_status_json(data_root: &Path) -> String {
    let quarantined = crate::catalog_quarantine::is_quarantined(data_root);
    let (
        authority,
        marker_present,
        marker_digest,
        snapshot,
        plan,
        content,
        locator,
        database_present,
        database_bytes,
        complete,
    ) = match resolve_marker(data_root) {
        MarkerState::Absent => (
            "file-journal",
            false,
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            false,
            0,
            !quarantined,
        ),
        MarkerState::Corrupt => (
            "marker-corrupt",
            true,
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            false,
            0,
            false,
        ),
        MarkerState::Valid(committed) => {
            let database_path = data_root
                .join("control")
                .join(&committed.marker.database_file_name);
            let (present, bytes) = match fs::symlink_metadata(&database_path) {
                Ok(metadata) if regular(&metadata) && metadata.len() > 0 => {
                    (true, metadata.len())
                }
                _ => (false, 0),
            };
            let locator = [
                "control/",
                committed.marker.database_file_name.as_str(),
            ]
            .concat();
            (
                "redb-control-marker-v1",
                true,
                json_string(&sha256::hex(&committed.marker.record_digest())),
                json_string(&sha256::hex(&committed.marker.catalog_snapshot)),
                json_string(&sha256::hex(&committed.marker.plan_chain)),
                json_string(&sha256::hex(&committed.marker.content_chain)),
                json_string(&locator),
                present,
                bytes,
                present && !quarantined,
            )
        }
    };
    format!(
        concat!(
            "{{\"event\":\"control_cutover_status\",\"schema\":\"control-cutover-status-v1\",",
            "\"authority\":{},\"marker_present\":{},\"marker_digest\":{},",
            "\"catalog_snapshot_sha256\":{},\"plan_chain_sha256\":{},",
            "\"content_manifest_chain_sha256\":{},\"staged_database_locator\":{},",
            "\"staged_database_present\":{},\"staged_database_bytes\":{},",
            "\"quarantined\":{},\"complete\":{},\"read_only\":true}}"
        ),
        json_string(authority),
        marker_present,
        marker_digest,
        snapshot,
        plan,
        content,
        locator,
        database_present,
        database_bytes,
        quarantined,
        complete,
    )
}
