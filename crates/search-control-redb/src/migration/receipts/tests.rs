use search_contracts::{
    DataRootId, InstallationIncarnationId, SourceNamespaceId,
};

use super::*;

fn target() -> SourceNamespaceId {
    SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
        .expect("target")
}

fn staged(location: SourceMigrationPlanLocation) -> SourceMigrationStagedPlan {
    SourceMigrationStagedPlan {
        target: target(),
        catalog_snapshot: [0x11; 32],
        plan_name: format!("{}.source-map.v1", "22".repeat(32)),
        plan_chain: [0x22; 32],
        plan_bytes: 321,
        plan_reused: true,
        summary: SourceMappingSummary {
            events: 7,
            sources: 3,
            occurrences: 4,
            path_only_events: 2,
            retirements: 1,
        },
        location,
        database_name: format!("{}.source-map.v2.redb", "33".repeat(32)),
        database_reused: false,
        content_name: format!("{}.source-content.v1", "44".repeat(32)),
        content_chain: [0x44; 32],
        content_records: 4,
        content_source_bytes: 987,
    }
}

fn marker() -> ControlCutoverMarker {
    ControlCutoverMarker {
        target: target(),
        incarnation: InstallationIncarnationId::from_bytes([0x55; 16]),
        root: DataRootId::from_bytes([0x66; 16]),
        epoch: 9,
        catalog_snapshot: [0x11; 32],
        plan_chain: [0x22; 32],
        content_chain: [0x44; 32],
        database_file_name: format!("{}.source-map.v2.redb", "33".repeat(32)),
        database_schema: CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA.to_owned(),
    }
}

#[test]
fn staged_plan_projection_is_byte_exact_for_each_location_family() {
    let explicit = staged(SourceMigrationPlanLocation::ExplicitOutputDirectory)
        .render_json();
    assert!(explicit.contains(
        "\"plan_locator\":\"2222222222222222222222222222222222222222222222222222222222222222.source-map.v1\""
    ));
    assert!(explicit.contains("\"plan_location\":\"explicit_output_directory\""));

    let migration_plans = staged(SourceMigrationPlanLocation::DataRootMigrationPlans)
        .render_json();
    assert!(migration_plans.contains(
        "\"plan_locator\":\"control/migration-plans/2222222222222222222222222222222222222222222222222222222222222222.source-map.v1\""
    ));
    assert!(migration_plans.contains("\"plan_location\":\"data_root\""));

    let control = staged(SourceMigrationPlanLocation::DataRootControl).render_json();
    assert!(control.contains(
        "\"staged_database_locator\":\"control/3333333333333333333333333333333333333333333333333333333333333333.source-map.v2.redb\""
    ));
    assert!(control.ends_with(
        "\"redb_imported\":false,\"active_control_imported\":false,\"cutover_authorized\":false}"
    ));
    assert!(!control.contains('\n'));
}

#[test]
fn cutover_commit_projection_is_byte_exact_and_content_minimized() {
    let marker = marker();
    let receipt = render_control_cutover_committed_receipt(&marker, true, false);

    assert!(receipt.starts_with(
        "{\"event\":\"control_cutover_committed\",\"schema\":\"eliot.control-cutover.v1\""
    ));
    assert!(receipt.contains(
        "\"marker_locator\":\"control/control-cutover.v1\""
    ));
    assert!(receipt.contains(
        "\"staged_database_reused\":true,\"installation_incarnation_id\":"
    ));
    assert!(receipt.contains("\"owner_epoch\":9,\"replayed\":false"));
    assert!(receipt.ends_with(
        "\"primary_authority\":\"redb-control-marker-v1\",\"serve_path_reroute\":\"pending-integration-wiring\"}"
    ));
    assert!(!receipt.contains("source_body"));
    assert!(!receipt.contains("query"));
}

#[test]
fn rollback_projection_is_byte_exact() {
    let receipt = render_control_cutover_rollback_receipt(&[0xab; 32]);
    assert_eq!(
        receipt,
        format!(
            concat!(
                "{{\"event\":\"control_cutover_rollback\",\"schema\":\"eliot.control-cutover.v1\",",
                "\"marker_present\":false,\"action\":\"noop-verified-file-snapshot\",",
                "\"catalog_snapshot_sha256\":\"{}\",\"cutover_authorized\":false,",
                "\"primary_authority\":\"file-journal\"}}"
            ),
            "ab".repeat(32),
        )
    );
}

#[test]
fn cutover_status_projection_preserves_closed_states() {
    let absent = ControlCutoverStatusProjection {
        state: ControlCutoverStatusState::FileJournal,
        quarantined: false,
    }
    .render_json();
    assert!(absent.contains("\"authority\":\"file-journal\""));
    assert!(absent.contains("\"marker_present\":false"));
    assert!(absent.contains("\"complete\":true"));

    let corrupt = ControlCutoverStatusProjection {
        state: ControlCutoverStatusState::MarkerCorrupt,
        quarantined: true,
    }
    .render_json();
    assert!(corrupt.contains("\"authority\":\"marker-corrupt\""));
    assert!(corrupt.contains("\"marker_present\":true"));
    assert!(corrupt.contains("\"complete\":false"));

    let committed = ControlCutoverStatusProjection {
        state: ControlCutoverStatusState::Committed {
            marker: marker(),
            database_present: true,
            database_bytes: 4096,
        },
        quarantined: false,
    }
    .render_json();
    assert!(committed.contains(
        "\"authority\":\"redb-control-marker-v1\""
    ));
    assert!(committed.contains("\"staged_database_present\":true"));
    assert!(committed.contains("\"staged_database_bytes\":4096"));
    assert!(committed.contains("\"complete\":true"));
    assert!(committed.ends_with("\"read_only\":true}"));
}
