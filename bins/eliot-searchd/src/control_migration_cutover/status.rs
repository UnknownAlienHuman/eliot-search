//! Read-only cutover status observation and package-owned projection.

use std::fs;
use std::path::Path;

use search_control_redb::migration::{
    ControlCutoverStatusProjection, ControlCutoverStatusState,
};

use super::marker_io::{MarkerState, resolve_marker};

/// Renders bounded read-only authority status.
///
/// The operation performs one package-owned marker read plus at most one staged
/// database stat. It never replays the journal and never writes or repairs state.
pub(super) fn cutover_status_json(data_root: &Path) -> String {
    let quarantined = crate::catalog_quarantine::is_quarantined(data_root);
    let state = match resolve_marker(data_root) {
        MarkerState::Absent => ControlCutoverStatusState::FileJournal,
        MarkerState::Corrupt => ControlCutoverStatusState::MarkerCorrupt,
        MarkerState::Valid(committed) => {
            let committed = *committed;
            let database_path = data_root
                .join("control")
                .join(&committed.marker.database_file_name);
            let (database_present, database_bytes) =
                match fs::symlink_metadata(&database_path) {
                    Ok(metadata) if regular(&metadata) && metadata.len() > 0 => {
                        (true, metadata.len())
                    }
                    _ => (false, 0),
                };
            ControlCutoverStatusState::Committed {
                marker: committed.marker,
                database_present,
                database_bytes,
            }
        }
    };
    ControlCutoverStatusProjection { state, quarantined }.render_json()
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
