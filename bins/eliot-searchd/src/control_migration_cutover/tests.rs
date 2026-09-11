use search_contracts::{DataRootId, InstallationIncarnationId, SourceNamespaceId};
use search_control_redb::migration::{
    CONTROL_CUTOVER_MARKER_FILE as CUTOVER_MARKER_FILE,
    CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA as STAGED_DATABASE_SCHEMA,
    ControlCutoverMarker as CutoverMarker,
};

use super::marker_io::{
    CUTOVER_MARKER_TMP, MarkerState, PublishOutcome, RollbackAction,
    check_rollback, gate_staging_against_marker, publish_marker, resolve_marker,
};
use super::status::cutover_status_json;

fn sample_marker() -> CutoverMarker {
    CutoverMarker {
        target: SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
            .expect("valid target"),
        incarnation: InstallationIncarnationId::from_bytes([0x11; 16]),
        root: DataRootId::from_bytes([0x22; 16]),
        epoch: 3,
        catalog_snapshot: [0x33; 32],
        plan_chain: [0x44; 32],
        content_chain: [0x55; 32],
        database_file_name: format!("{}.source-map.v2.redb", "66".repeat(32)),
        database_schema: STAGED_DATABASE_SCHEMA.to_owned(),
    }
}

struct Scratch {
    root: std::path::PathBuf,
}

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-cutover-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("control")).expect("create scratch control");
        Self { root }
    }

    fn control(&self) -> std::path::PathBuf {
        self.root.join("control")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn publish_then_lost_ack_retry_is_identical() {
    let scratch = Scratch::new();
    let marker = sample_marker();
    let bytes = marker.encode();
    assert_eq!(
        publish_marker(&scratch.root, &bytes).expect("first publication"),
        PublishOutcome::Committed
    );
    assert_eq!(
        publish_marker(&scratch.root, &bytes).expect("identical replay"),
        PublishOutcome::ReplayIdentical
    );
    assert_eq!(
        std::fs::read(scratch.control().join(CUTOVER_MARKER_FILE))
            .expect("read marker"),
        bytes
    );
    assert!(
        !scratch.control().join(CUTOVER_MARKER_TMP).exists(),
        "no tmp residue"
    );
}

#[test]
fn truncated_marker_is_corrupt_never_partial_authority() {
    let scratch = Scratch::new();
    let bytes = sample_marker().encode();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        &bytes[..bytes.len() / 2],
    )
    .expect("write torn marker");
    assert_eq!(resolve_marker(&scratch.root), MarkerState::Corrupt);
    let status = cutover_status_json(&scratch.root);
    assert!(
        status.contains("\"authority\":\"marker-corrupt\""),
        "{status}"
    );
    assert!(status.contains("\"complete\":false"), "{status}");
}

#[test]
fn gate_absent_marker_allows_staging() {
    let scratch = Scratch::new();
    let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
        .expect("valid target");
    assert!(
        gate_staging_against_marker(&scratch.root, target, [0x33; 32]).is_ok()
    );
    assert!(!scratch.control().join("catalog-quarantine.marker").exists());
}

#[test]
fn gate_matching_marker_allows_identical_replan() {
    let scratch = Scratch::new();
    let marker = sample_marker();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        marker.encode(),
    )
    .expect("write marker");
    assert!(
        gate_staging_against_marker(
            &scratch.root,
            marker.target,
            marker.catalog_snapshot,
        )
        .is_ok()
    );
}

#[test]
fn gate_diverged_marker_supersedes_without_quarantine() {
    let scratch = Scratch::new();
    let marker = sample_marker();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        marker.encode(),
    )
    .expect("write marker");
    let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
        .expect("valid target");
    assert_eq!(
        gate_staging_against_marker(&scratch.root, target, [0x78; 32]),
        Err("DIRECT_MIGRATION_CUTOVER_SUPERSEDED".to_owned())
    );
    assert!(!scratch.control().join("catalog-quarantine.marker").exists());
}

#[test]
fn gate_corrupt_marker_quarantines_and_preserves_bytes() {
    let scratch = Scratch::new();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        b"corrupt-and-preserved",
    )
    .expect("write corrupt marker");
    let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
        .expect("valid target");
    assert_eq!(
        gate_staging_against_marker(&scratch.root, target, [0x33; 32]),
        Err("DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned())
    );
    assert!(scratch.control().join("catalog-quarantine.marker").exists());
    assert_eq!(
        std::fs::read(scratch.control().join(CUTOVER_MARKER_FILE))
            .expect("read preserved marker"),
        b"corrupt-and-preserved"
    );
}

#[test]
fn status_without_marker_reports_file_journal_authority() {
    let scratch = Scratch::new();
    let status = cutover_status_json(&scratch.root);
    assert!(
        status.contains("\"authority\":\"file-journal\""),
        "{status}"
    );
    assert!(status.contains("\"marker_present\":false"), "{status}");
    assert!(status.contains("\"read_only\":true"), "{status}");
}

#[test]
fn status_is_read_only_across_ten_thousand_reads() {
    let scratch = Scratch::new();
    let marker = sample_marker();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        marker.encode(),
    )
    .expect("write marker");
    std::fs::write(
        scratch.control().join(&marker.database_file_name),
        b"staged-redb-stand-in",
    )
    .expect("write staged database stand-in");

    let mut before: Vec<(String, std::time::SystemTime)> =
        std::fs::read_dir(scratch.control())
            .expect("read control directory")
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry
                    .file_name()
                    .into_string()
                    .expect("UTF-8 scratch name");
                let modified = std::fs::symlink_metadata(scratch.control().join(&name))
                    .expect("stat scratch entry")
                    .modified()
                    .expect("modification time");
                (name, modified)
            })
            .collect();
    before.sort();

    let first = cutover_status_json(&scratch.root);
    assert!(
        first.contains("\"authority\":\"redb-control-marker-v1\""),
        "{first}"
    );
    assert!(first.contains("\"complete\":true"), "{first}");
    for _ in 0..10_000 {
        assert_eq!(cutover_status_json(&scratch.root), first);
    }

    let mut after: Vec<(String, std::time::SystemTime)> =
        std::fs::read_dir(scratch.control())
            .expect("read control directory")
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry
                    .file_name()
                    .into_string()
                    .expect("UTF-8 scratch name");
                let modified = std::fs::symlink_metadata(scratch.control().join(&name))
                    .expect("stat scratch entry")
                    .modified()
                    .expect("modification time");
                (name, modified)
            })
            .collect();
    after.sort();
    assert_eq!(before, after, "status reads write nothing durable");
}

#[test]
fn rollback_without_marker_is_a_verified_noop() {
    let scratch = Scratch::new();
    assert_eq!(
        check_rollback(&scratch.root).expect("absent marker rollback"),
        RollbackAction::Noop
    );
    assert!(!scratch.control().join("catalog-quarantine.marker").exists());
}

#[test]
fn rollback_refuses_a_committed_marker() {
    let scratch = Scratch::new();
    std::fs::write(
        scratch.control().join(CUTOVER_MARKER_FILE),
        sample_marker().encode(),
    )
    .expect("write marker");
    assert_eq!(
        check_rollback(&scratch.root),
        Err("DIRECT_MIGRATION_CUTOVER_ALREADY_COMMITTED".to_owned())
    );
}

#[test]
fn rollback_on_corrupt_marker_quarantines() {
    let scratch = Scratch::new();
    std::fs::write(scratch.control().join(CUTOVER_MARKER_FILE), b"torn")
        .expect("write torn marker");
    assert_eq!(
        check_rollback(&scratch.root),
        Err("DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned())
    );
    assert!(scratch.control().join("catalog-quarantine.marker").exists());
}
