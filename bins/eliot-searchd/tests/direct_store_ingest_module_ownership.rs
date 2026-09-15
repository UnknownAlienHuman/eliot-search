use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn direct_ingest_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/direct_store_ingest.rs");
    assert!(entry.contains("#[path = \"direct_store_ingest/kernel.rs\"]"));
    assert!(entry.contains("mod kernel;"));
    assert!(entry.len() < 2_000, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "fn index_paths_bounded(",
        "fn plan_snapshot(",
        "AdmissionPolicy::baseline",
        "read_file_snapshot",
        "append_drafts",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "ingestion implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/direct_store_ingest/kernel.rs");
    for module in ["batch", "entry", "plan", "policy", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn ingestion_responsibilities_are_bounded_and_vendor_free() {
    let root = crate_root();
    let owners = [
        (
            "src/direct_store_ingest/kernel/entry.rs",
            "pub(crate) fn index_file_with_writer(",
        ),
        (
            "src/direct_store_ingest/kernel/policy.rs",
            "pub(super) fn registry_view(",
        ),
        (
            "src/direct_store_ingest/kernel/plan.rs",
            "pub(super) fn plan_snapshot(",
        ),
        (
            "src/direct_store_ingest/kernel/batch.rs",
            "pub(super) fn index_paths_bounded(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant_bridge",
            "reqwest::",
            "tokio::",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired vendor/runtime token {forbidden}"
            );
        }
    }

    let entry = read(&root, "src/direct_store_ingest/kernel/entry.rs");
    assert!(entry.contains("collect_regular_files"));
    assert!(entry.contains("paths.sort_by_key"));
    assert!(!entry.contains("canonical::plan_snapshot"));
    assert!(!entry.contains("writer(self"));

    let policy = read(&root, "src/direct_store_ingest/kernel/policy.rs");
    assert!(policy.contains("AdmissionPolicy::baseline"));
    assert!(policy.contains("RegistryView::build"));
    assert!(policy.contains("self.registry.latest"));
    assert!(!policy.contains("read_file_snapshot"));
    assert!(!policy.contains("append_drafts"));

    let plan = read(&root, "src/direct_store_ingest/kernel/plan.rs");
    assert!(plan.contains("canonical::plan_snapshot"));
    assert!(plan.contains("DIRECT_DUPLICATE_SOURCE_IN_BATCH"));
    assert!(plan.contains("DIRECT_SOURCE_ID_COLLISION"));
    assert!(plan.contains("eliot-search/direct-index-operation/v2"));
    assert!(!plan.contains("read_file_snapshot"));
    assert!(!plan.contains("writer(self"));
    assert!(!plan.contains("append_drafts"));

    let batch = read(&root, "src/direct_store_ingest/kernel/batch.rs");
    assert!(batch.contains("DIRECT_CONTROL_READBACK_MISMATCH"));
    assert!(batch.contains("let policy = Self::admission_policy();"));
    assert!(batch.contains("let view = self.registry_view(&policy)?;"));
    assert!(batch.contains("planned.push(self.plan_snapshot("));
    let planning = batch.find("planned.push(self.plan_snapshot(").unwrap();
    let first_write = batch.find("writer(self, &source, &snapshot.bytes)?;").unwrap();
    assert!(planning < first_write, "writer moved before complete planning");
    assert!(batch.contains("self.append_drafts(drafts)?;"));
}

#[test]
fn ingestion_regression_corpus_remains_separate_and_complete() {
    let root = crate_root();
    let tests = read(&root, "src/direct_store_ingest/kernel/tests.rs");
    assert!(tests.len() < 30_000, "ingestion tests grew to {} bytes", tests.len());
    for case in [
        "failed_writer_cannot_publish_a_revision",
        "later_writer_failure_leaves_all_batch_metadata_unpublished",
        "aggregate_byte_limit_fails_before_any_writer_call",
        "returning_to_old_content_is_a_new_transition_not_a_conflicting_replay",
        "retired_source_can_be_reactivated_with_unchanged_bytes",
        "stale_catalog_refuses_new_writes_before_calling_storage",
        "byte_budget_counts_only_admitted_retained_bytes",
        "empty_source_is_denied_before_any_writer_call",
        "denied_sources_never_reach_cas",
        "equal_content_distinct_files_keep_distinct_stable_identities",
        "rename_preserves_stable_identity_with_new_locator_binding",
        "replacement_at_same_path_with_new_identity_does_not_reuse_closed_source",
        "restart_preserves_membership_revision_occurrences_and_lineage",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
    assert!(tests.contains("calls, 0"));
    assert!(tests.contains("fixture.log(), before"));
}
