//! Canonical content-minimized projections for source-control migration.
//!
//! The records in this module report already verified technical state. They do
//! not acquire a data-root owner, authorize a cutover, read source content,
//! mutate a journal or switch a serving path.

#![allow(clippy::module_name_repetitions)]

use core::fmt::Write as _;

use search_contracts::SourceNamespaceId;

use super::{
    CONTROL_CUTOVER_MARKER_FILE, CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA,
    ControlCutoverMarker, SourceMappingSummary,
};

/// Closed location family for one inactive source-migration plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMigrationPlanLocation {
    /// Artifact is rooted in an explicitly supplied output directory.
    ExplicitOutputDirectory,
    /// Artifact is stored under `control/migration-plans/` in the data root.
    DataRootMigrationPlans,
    /// Artifact is stored directly under `control/` for atomic cutover.
    DataRootControl,
}

impl SourceMigrationPlanLocation {
    const fn locator_prefix(self) -> &'static str {
        match self {
            Self::ExplicitOutputDirectory => "",
            Self::DataRootMigrationPlans => "control/migration-plans/",
            Self::DataRootControl => "control/",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::ExplicitOutputDirectory => "explicit_output_directory",
            Self::DataRootMigrationPlans | Self::DataRootControl => "data_root",
        }
    }
}

/// Exact verified state rendered by `source_migration_plan_staged`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMigrationStagedPlan {
    /// Imported target namespace.
    pub target: SourceNamespaceId,
    /// Complete legacy catalog snapshot bound by the plan.
    pub catalog_snapshot: [u8; 32],
    /// Final local source-map artifact basename.
    pub plan_name: String,
    /// Canonical source-map record-chain digest.
    pub plan_chain: [u8; 32],
    /// Exact encoded source-map artifact bytes.
    pub plan_bytes: u64,
    /// Whether an existing byte-identical plan was reused.
    pub plan_reused: bool,
    /// Final deterministic mapping accounting.
    pub summary: SourceMappingSummary,
    /// Closed locator/location family.
    pub location: SourceMigrationPlanLocation,
    /// Final local inactive redb basename.
    pub database_name: String,
    /// Whether an existing fully verified inactive redb artifact was reused.
    pub database_reused: bool,
    /// Final local source-content manifest basename.
    pub content_name: String,
    /// Canonical source-content manifest record-chain digest.
    pub content_chain: [u8; 32],
    /// Number of retained content objects verified by the manifest.
    pub content_records: u64,
    /// Sum of exact retained source bytes verified by the manifest.
    pub content_source_bytes: u64,
}

/// Closed read-only authority state for the cutover-status projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlCutoverStatusState {
    /// No cutover marker exists and the preserved file journal remains primary.
    FileJournal,
    /// A marker locator exists but cannot be read and decoded coherently.
    MarkerCorrupt,
    /// One valid marker names an observed staged database artifact.
    Committed {
        /// Exact canonical committed marker.
        marker: ControlCutoverMarker,
        /// Whether the named staged database is a nonempty admitted file.
        database_present: bool,
        /// Exact observed staged database byte length, or zero when absent.
        database_bytes: u64,
    },
}

/// Inputs for the historical bounded read-only cutover-status response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCutoverStatusProjection {
    /// Read-only marker/database authority state.
    pub state: ControlCutoverStatusState,
    /// Whether daemon policy currently marks the data root quarantined.
    pub quarantined: bool,
}

impl ControlCutoverStatusProjection {
    /// Renders the historical byte-stable read-only cutover-status JSON.
    #[must_use]
    pub fn render_json(&self) -> String {
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
        ) = match &self.state {
            ControlCutoverStatusState::FileJournal => (
                "file-journal",
                false,
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                false,
                0,
                !self.quarantined,
            ),
            ControlCutoverStatusState::MarkerCorrupt => (
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
            ControlCutoverStatusState::Committed {
                marker,
                database_present,
                database_bytes,
            } => {
                let locator = [
                    "control/",
                    marker.database_file_name.as_str(),
                ]
                .concat();
                (
                    "redb-control-marker-v1",
                    true,
                    json_string(&hex(&marker.record_digest())),
                    json_string(&hex(&marker.catalog_snapshot)),
                    json_string(&hex(&marker.plan_chain)),
                    json_string(&hex(&marker.content_chain)),
                    json_string(&locator),
                    *database_present,
                    *database_bytes,
                    *database_present && !self.quarantined,
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
            self.quarantined,
            complete,
        )
    }
}

impl SourceMigrationStagedPlan {
    /// Renders the historical byte-stable staged-plan JSON receipt.
    #[must_use]
    pub fn render_json(&self) -> String {
        let prefix = self.location.locator_prefix();
        let plan_locator = [prefix, self.plan_name.as_str()].concat();
        let database_locator = [prefix, self.database_name.as_str()].concat();
        let content_locator = [prefix, self.content_name.as_str()].concat();
        format!(
            concat!(
                "{{\"event\":\"source_migration_plan_staged\",\"schema\":\"eliot.source-mapping.v1\",",
                "\"target_namespace_id\":\"{}\",\"catalog_snapshot_sha256\":\"{}\",",
                "\"plan_locator\":{},\"plan_chain_sha256\":\"{}\",\"digest_scheme\":\"sha256-record-chain-v1\",\"plan_bytes\":{},\"reused\":{},",
                "\"events\":{},\"sources\":{},\"revision_occurrences\":{},\"retained_revision_events\":{},",
                "\"retirements\":{},\"all_source_events_mapped\":true,\"canonical_records_materialized\":false,",
                "\"plan_location\":\"{}\",\"staged_database_locator\":{},\"staged_database_reused\":{},",
                "\"source_mapping_imported_to_redb\":true,\"staged_database_verified\":true,",
                "\"staged_database_schema\":\"{}\",\"content_manifest_bound_to_redb\":true,",
                "\"content_manifest_locator\":{},\"content_manifest_chain_sha256\":\"{}\",",
                "\"content_objects_verified\":{},\"content_bytes_verified\":{},\"content_blake3_verified\":true,",
                "\"redb_imported\":false,\"active_control_imported\":false,\"cutover_authorized\":false}}"
            ),
            self.target,
            hex(&self.catalog_snapshot),
            json_string(&plan_locator),
            hex(&self.plan_chain),
            self.plan_bytes,
            self.plan_reused,
            self.summary.events,
            self.summary.sources,
            self.summary.occurrences,
            self.summary.path_only_events,
            self.summary.retirements,
            self.location.label(),
            json_string(&database_locator),
            self.database_reused,
            CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA,
            json_string(&content_locator),
            hex(&self.content_chain),
            self.content_records,
            self.content_source_bytes,
        )
    }
}

/// Renders the historical byte-stable successful control-cutover receipt.
#[must_use]
pub fn render_control_cutover_committed_receipt(
    marker: &ControlCutoverMarker,
    staged_database_reused: bool,
    replayed: bool,
) -> String {
    let locator = ["control/", marker.database_file_name.as_str()].concat();
    format!(
        concat!(
            "{{\"event\":\"control_cutover_committed\",\"schema\":\"eliot.control-cutover.v1\",",
            "\"target_namespace_id\":\"{}\",\"catalog_snapshot_sha256\":\"{}\",",
            "\"plan_chain_sha256\":\"{}\",\"content_manifest_chain_sha256\":\"{}\",",
            "\"marker_locator\":\"control/{}\",\"marker_digest\":\"{}\",",
            "\"staged_database_locator\":{},\"staged_database_schema\":{},",
            "\"staged_database_reused\":{},\"installation_incarnation_id\":\"{}\",",
            "\"data_root_id\":\"{}\",\"owner_epoch\":{},\"replayed\":{},",
            "\"cutover_authorized\":true,\"file_journal_preserved\":true,",
            "\"primary_authority\":\"redb-control-marker-v1\",",
            "\"serve_path_reroute\":\"pending-integration-wiring\"}}"
        ),
        marker.target,
        hex(&marker.catalog_snapshot),
        hex(&marker.plan_chain),
        hex(&marker.content_chain),
        CONTROL_CUTOVER_MARKER_FILE,
        hex(&marker.record_digest()),
        json_string(&locator),
        json_string(&marker.database_schema),
        staged_database_reused,
        marker.incarnation,
        marker.root,
        marker.epoch,
        replayed,
    )
}

/// Renders the historical byte-stable verified no-op rollback receipt.
#[must_use]
pub fn render_control_cutover_rollback_receipt(
    catalog_snapshot: &[u8; 32],
) -> String {
    format!(
        concat!(
            "{{\"event\":\"control_cutover_rollback\",\"schema\":\"eliot.control-cutover.v1\",",
            "\"marker_present\":false,\"action\":\"noop-verified-file-snapshot\",",
            "\"catalog_snapshot_sha256\":\"{}\",\"cutover_authorized\":false,",
            "\"primary_authority\":\"file-journal\"}}"
        ),
        hex(catalog_snapshot),
    )
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests;
