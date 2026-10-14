use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("daemon package is nested under bins/")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
        .replace("\r\n", "\n")
}

#[test]
fn native_migration_uses_one_context_and_never_dispatches_legacy_marker_effects() {
    let root = repository_root();
    let source = read(&root, "bins/eliot-searchd/src/source_migration_command.rs");
    let command = source.split("#[cfg(test)]\nimpl DataRootGuard").next().unwrap();
    for required in ["for_control_migration_plan(", "admit_existing(request)",
        "run_registration_one_shot(", "migration_input_from_bytes(", "open_existing_migration(operation)"] {
        assert!(command.contains(required), "missing original migration custody: {required}");
    }
    for forbidden in ["with_existing_lock(", "with_existing_mapping_source(", "RevisionProtector::open_existing(",
        "source_roots::migration_input(", "DirectStore::open(", "finish_in_place("] {
        assert!(!command.contains(forbidden), "legacy migration dispatch restored: {forbidden}");
    }
    let plan = read(&root, "bins/eliot-searchd/src/control_migration_plan.rs");
    let native_marker = plan.split("fn gate_native_staging(").nth(1).unwrap()
        .split("pub(super) fn temporary_staging_name(").next().unwrap();
    assert!(native_marker.contains("State::Corrupt => Err("));
    assert!(!native_marker.contains("catalog_quarantine"));
    assert!(!native_marker.contains("publish_control_cutover_marker"));
    let objects = read(&root, "bins/eliot-searchd/src/secure_direct_store_storage_io.rs");
    assert!(objects.contains("read_legacy_revision_object_with_context("));
    assert!(objects.contains("ContextualLegacyRevisionObjectPlatform<C>"));
    let context = read(&root, "bins/eliot-searchd/src/control_migration_context.rs");
    assert!(context.contains("try_borrow_mut()"));
    assert!(context.contains("unexpected_source_effect.set(true)"));
}

#[test]
fn retained_revision_unprotect_uses_contextual_dpapi_and_preserves_unknown_reasons() {
    let root = repository_root();
    let context = read(&root, "bins/eliot-searchd/src/control_migration_context.rs");
    for marker in ["impl LegacyRevisionDpapiContext for &MigrationContext",
        "LegacyRevisionDpapiWork::NativeCall => Work::Observe",
        "LegacyRevisionDpapiWork::Allocate(bytes) => Work::Allocate(bytes)",
        "LegacyRevisionDpapiWork::Process(bytes) => Work::Compare(bytes)",
        "dpapi_account_native_clear", "dpapi_clear_accounting_failed",
        "dpapi_first_refusal: Rc<Cell<Option<MigrationDpapiFirstRefusal>>>",
        "fn first_refusal(&self)", "if self.dpapi_first_refusal.get().is_none()",
        "MigrationDpapiFirstRefusal::NativeOwner(first_refusal)"] {
        assert!(context.contains(marker), "contextual DPAPI accounting lost {marker}");
    }
    let objects = read(&root, "bins/eliot-searchd/src/control_migration_objects.rs");
    assert!(objects.contains("protector.unprotect_with_context("));
    let protector = read(&root, "bins/eliot-searchd/src/revision_protection/protector/operations.rs");
    assert!(protector.matches("self.validate_unprotect(").count() >= 2);
    assert!(protector.contains("native_windows::unprotect_data_with_context("));
    assert!(protector.contains("native_windows::unprotect_data("));
    let dpapi = read(&root, "bins/eliot-searchd/src/revision_protection_windows/dpapi.rs");
    assert!(dpapi.contains("unprotect_legacy_revision_current_user_with_context("));
    assert!(dpapi.contains("ContextualLegacyRevisionDpapiError::NativeAndContext"));

    let child = read(&root, "bins/eliot-searchd/src/owner_composition/kernel/read_child.rs");
    let clear = child.split("pub(crate) const fn migration_account_native_clear(").nth(1).unwrap()
        .split("pub(crate) fn migration_deadline").next().unwrap();
    assert!(clear.contains("account_native_clear("));
    assert!(!clear.contains("with_binding("));
    assert!(!clear.contains("request.check("));
    assert!(read(&root, "bins/eliot-searchd/src/owner_composition/kernel/native_request.rs")
        .contains("pub(super) const fn account_native_clear("));

    let command = read(&root, "bins/eliot-searchd/src/source_migration_command.rs");
    for reason in ["DIRECT_MIGRATION_PLAN_PUBLISH_OUTCOME_UNKNOWN",
        "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN",
        "DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN",
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT",
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED"] {
        assert!(command.contains(reason), "public migration boundary collapsed {reason}");
    }
}

#[test]
fn cutover_marker_semantics_and_file_lifecycle_belong_to_control_owner() {
    let root = repository_root();
    let schema_owner = read(
        &root,
        "crates/search-control-redb/src/migration/cutover.rs",
    );
    assert!(schema_owner.contains("pub struct ControlCutoverMarker"));
    assert!(schema_owner.contains("ELIOT-SEARCH-CONTROL-CUTOVER-V1"));
    assert!(schema_owner.contains("classify_control_cutover_replay"));
    assert!(!schema_owner.contains("std::fs"));
    assert!(!schema_owner.contains("catalog_quarantine"));
    assert!(!schema_owner.contains("impl DirectStore"));

    let artifact_owner = read(
        &root,
        "crates/search-control-redb/src/migration/cutover_artifact.rs",
    );
    for required in [
        "pub enum ControlCutoverMarkerFileState",
        "pub fn resolve_control_cutover_marker",
        "pub fn publish_control_cutover_marker",
        "OpenOptions",
        "write_all",
        "fs::hard_link",
        "read_to_end",
    ] {
        assert!(
            artifact_owner.contains(required),
            "control owner is missing cutover artifact behavior {required}"
        );
    }
    assert!(!artifact_owner.contains("fs::rename"));
    assert!(!artifact_owner.contains("catalog_quarantine"));
    assert!(!artifact_owner.contains("impl DirectStore"));

    let facade = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover.rs",
    );
    assert!(
        facade.len() < 4_096,
        "cutover facade grew to {} bytes",
        facade.len()
    );
    assert!(!facade.contains("struct CutoverMarker"));
    assert!(!facade.contains("ELIOT-SEARCH-CONTROL-CUTOVER-V1"));

    let marker_io = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/marker_io.rs",
    );
    assert!(marker_io.contains("resolve_control_cutover_marker"));
    assert!(marker_io.contains("publish_control_cutover_marker"));
    assert!(marker_io.contains("catalog_quarantine"));
    assert!(!marker_io.contains("impl DirectStore"));
    for forbidden in [
        "OpenOptions",
        "write_all",
        "fs::rename",
        "fs::hard_link",
        "fs::read",
        "sync_all",
        "read_to_end",
    ] {
        assert!(
            !marker_io.contains(forbidden),
            "daemon restored package-owned marker I/O: {forbidden}"
        );
    }

    let operation = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/operation.rs",
    );
    assert!(operation.contains("impl DirectStore"));
    assert!(operation.contains("ControlCutoverMarker as CutoverMarker"));
    assert!(!operation.contains("OpenOptions"));
    assert!(!operation.contains("MARKER_MAGIC"));

    let status = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/status.rs",
    );
    assert!(status.contains("ControlCutoverStatusProjection"));
    assert!(status.contains("ControlCutoverStatusState"));
    assert!(!status.contains("control_cutover_status"));
    assert!(!status.contains("read_only"));
    assert!(!status.contains("OpenOptions"));
    assert!(!status.contains("impl DirectStore"));
}

#[test]
fn content_manifest_schema_belongs_to_control_owner() {
    let root = repository_root();
    let owner = read(
        &root,
        "crates/search-control-redb/src/migration/content.rs",
    );
    for required in [
        "pub struct SourceContentManifestEncoder",
        "pub struct SourceContentManifestHeader",
        "pub struct SourceContentObjectReadback",
        "pub struct SourceContentManifestSummary",
        "source_content_profile_digest",
        "source_content_header",
        "source_content_readback",
        "source_content_end",
    ] {
        assert!(
            owner.contains(required),
            "control owner is missing content-manifest boundary {required}"
        );
    }
    assert!(!owner.contains("RevisionProtector"));
    assert!(!owner.contains("read_import_revision"));
    assert!(!owner.contains("blake3::Hasher"));

    let adapter = read(
        &root,
        "bins/eliot-searchd/src/control_migration_content.rs",
    );
    assert!(adapter.contains("SourceContentManifestEncoder"));
    assert!(adapter.contains("SourceImportRecordArtifact"));
    assert!(adapter.contains("RevisionProtector"));
    assert!(adapter.contains("read_import_revision"));
    assert!(adapter.contains("blake3::Hasher"));
    for forbidden in [
        "source_content_header",
        "source_content_readback",
        "source_content_end",
        "content_digest_algorithm",
        "const PROFILE",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "daemon restored package-owned manifest schema: {forbidden}"
        );
    }
}

#[test]
fn source_import_record_chain_and_artifact_lifecycle_have_one_owner() {
    let root = repository_root();
    let chain_owner = read(
        &root,
        "crates/search-control-redb/src/migration/record_chain.rs",
    );
    for required in [
        "pub struct SourceImportRecordChain",
        "MAX_SOURCE_IMPORT_RECORD_BYTES",
        "MAX_SOURCE_IMPORT_ROW_BYTES",
        "eliot-search/source-map-chain/v1",
        "eliot-search/source-map-row/v1",
        "eliot-search/source-map-end/v1",
    ] {
        assert!(
            chain_owner.contains(required),
            "control owner is missing record-chain boundary {required}"
        );
    }

    let artifact_owner = read(
        &root,
        "crates/search-control-redb/src/migration/record_artifact.rs",
    );
    for required in [
        "pub struct SourceImportRecordArtifact",
        "pub struct SourceImportRecordReadback",
        "inspect_source_import_record_artifact",
        "SourceImportRecordArtifactPublication",
        "classify_source_import_record_artifact",
        "resolve_same_process",
        "sync_and_verify_final",
        "fs::hard_link",
        "read_until",
    ] {
        assert!(
            artifact_owner.contains(required),
            "control owner is missing immutable artifact behavior {required}"
        );
    }

    for forbidden in ["fs::remove_file", "Drop for OwnedStaging", "cleanup_armed", "proof.v1", "ELIOT-RECORD-OP"] {
        assert!(!artifact_owner.contains(forbidden), "record artifact restored unqualified cleanup/restart authority: {forbidden}");
    }
    let adapter = read(&root, "bins/eliot-searchd/src/control_migration_redb.rs");
    assert!(adapter.contains("impl SourceImportRecordArtifactPlatform"));
    assert!(adapter.contains("sync_record_artifact_directory"));
    assert!(adapter.contains("0x0200_0000"));

    let plan = read(
        &root,
        "bins/eliot-searchd/src/control_migration_plan.rs",
    );
    let content = read(
        &root,
        "bins/eliot-searchd/src/control_migration_content.rs",
    );
    assert!(plan.contains("SourceImportRecordArtifact"));
    assert!(content.contains("SourceImportRecordArtifact"));
    for daemon in [&plan, &content] {
        for forbidden in [
            "struct PlanDigest",
            "struct StagingFile",
            "SourceImportRecordChain::new",
            "fs::hard_link",
            "fs::remove_file",
            "OpenOptions",
            "BufReader",
            "BufWriter",
            "read_until",
            "eliot-search/source-map-chain/v1",
            "eliot-search/source-map-row/v1",
            "eliot-search/source-map-end/v1",
        ] {
            assert!(
                !daemon.contains(forbidden),
                "daemon restored package-owned record-artifact logic: {forbidden}"
            );
        }
    }
}

#[test]
fn staged_plan_and_cutover_receipt_schemas_belong_to_control_owner() {
    let root = repository_root();
    let owner = read(
        &root,
        "crates/search-control-redb/src/migration/receipts.rs",
    );
    for required in [
        "pub struct SourceMigrationStagedPlan",
        "pub enum SourceMigrationPlanLocation",
        "pub struct ControlCutoverStatusProjection",
        "pub enum ControlCutoverStatusState",
        "source_migration_plan_staged",
        "control_cutover_status",
        "control_cutover_committed",
        "control_cutover_rollback",
        "render_control_cutover_committed_receipt",
        "render_control_cutover_rollback_receipt",
    ] {
        assert!(
            owner.contains(required),
            "control owner is missing migration receipt boundary {required}"
        );
    }
    assert!(!owner.contains("impl DirectStore"));
    assert!(!owner.contains("catalog_quarantine"));
    assert!(!owner.contains("RevisionProtector"));

    let plan = read(
        &root,
        "bins/eliot-searchd/src/control_migration_plan.rs",
    );
    assert!(plan.contains("SourceMigrationStagedPlan"));
    assert!(plan.contains("plan.render_json()"));
    assert!(!plan.contains("struct StagedPlan"));
    assert!(!plan.contains("source_migration_plan_staged"));
    assert!(!plan.contains("fn staged_plan_json"));

    let operation = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/operation.rs",
    );
    assert!(operation.contains("render_control_cutover_committed_receipt"));
    assert!(operation.contains("render_control_cutover_rollback_receipt"));
    assert!(!operation.contains("control_cutover_committed"));
    assert!(!operation.contains("control_cutover_rollback"));
    assert!(!operation.contains("fn cutover_receipt"));

    let status = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/status.rs",
    );
    assert!(status.contains("ControlCutoverStatusProjection"));
    assert!(status.contains("ControlCutoverStatusState"));
    assert!(!status.contains("control_cutover_status"));
    assert!(!status.contains("control-cutover-status-v1"));
}
