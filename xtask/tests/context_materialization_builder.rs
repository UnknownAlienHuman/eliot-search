use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use xtask::context_artifact::{
    ARTIFACT_FORMAT, BundleBlock, authority_map as candidate_authority,
    candidate_id, candidate_metadata_digest, expected_header, render_bundle,
};
use xtask::context_materialization::{
    DECISION_COMMIT, DECISION_MISSING, DECISION_PARTIAL_SIGNATURE,
    DECISION_SIGNATURES,
};
use xtask::context_materialization_builder::{build_plan, write_plan};
use xtask::ticket_planner::{canonical_json_bytes, exact_sha256_hex};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-materialization-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).expect("create scratch root");
        Self { root }
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        fs::create_dir_all(path.parent().expect("scratch file has parent"))
            .expect("create scratch parent");
        fs::write(path, bytes).expect("write scratch file");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Fixture {
    scratch: Scratch,
    candidate_path: String,
    bundle_path: String,
    selection_path: String,
    bundle: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let source = json!({
            "order": 0,
            "repository_path": "AGENTS.md",
            "git_blob_id": format!("sha1:{}", "1".repeat(40)),
            "exact_sha256": "2".repeat(64),
            "exact_bytes": 7,
            "materialization": "UTF8_LF",
            "materialized_sha256": exact_sha256_hex(b"# root\n"),
            "materialized_bytes": 7,
        });
        let fragment_content = canonical_json_bytes(&json!({
            "registry_path": "swarm/crates.toml",
            "selector": "package[name=search-contracts]",
            "value": {"name": "search-contracts"},
        }));
        let fragment = json!({
            "order": 0,
            "registry_path": "swarm/crates.toml",
            "selector": "package[name=search-contracts]",
            "source_git_blob_id": format!("sha1:{}", "3".repeat(40)),
            "source_exact_sha256": "4".repeat(64),
            "selector_match_count": 1,
            "fragment_sha256": exact_sha256_hex(&fragment_content),
            "fragment_bytes": fragment_content.len(),
        });
        let blocks = vec![
            BundleBlock {
                kind: "source".to_owned(),
                header: expected_header("source", &source).expect("source header"),
                metadata: source.clone(),
                content: b"# root\n".to_vec(),
            },
            BundleBlock {
                kind: "registry_fragment".to_owned(),
                header: expected_header("registry_fragment", &fragment)
                    .expect("fragment header"),
                metadata: fragment.clone(),
                content: fragment_content,
            },
        ];
        let base_commit = format!("sha1:{}", "a".repeat(40));
        let preamble = json!({
            "artifact_format": ARTIFACT_FORMAT,
            "repository": "UnknownAlienHuman/eliot-search",
            "base_commit": base_commit,
            "package": "search-contracts",
            "package_path": "crates/search-contracts",
            "stage": "W0",
            "phase": "P00",
            "wave": 0,
            "context_draft_path": "swarm/context-drafts/p00/search-contracts.toml",
            "context_draft_git_blob_id": format!("sha1:{}", "8".repeat(40)),
            "context_draft_exact_sha256": "9".repeat(64),
            "source_count": 1,
            "registry_fragment_count": 1,
            "accepted_handoff_count": 0,
            "required_unavailable_checks": ["real-toolchain"],
        });
        let bundle = render_bundle(&preamble, &blocks).expect("fixture bundle");
        let identifier = candidate_id(&bundle);
        let bundle_path = format!(
            "artifacts/context-artifact-candidates/search-contracts/{identifier}.context"
        );
        let candidate_path = format!(
            "artifacts/context-artifact-candidates/search-contracts/{identifier}.json"
        );
        let mut candidate = json!({
            "schema_version": 1,
            "record_kind": "context_artifact_candidate_v1",
            "status": "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED",
            "candidate_id": identifier,
            "repository": {
                "name": "UnknownAlienHuman/eliot-search",
                "base_commit": base_commit,
                "working_tree_used_as_input": false,
            },
            "package": {
                "name": "search-contracts",
                "path": "crates/search-contracts",
                "stage": "W0",
                "phase": "P00",
                "wave": 0,
            },
            "draft": {
                "path": "swarm/context-drafts/p00/search-contracts.toml",
                "git_blob_id": format!("sha1:{}", "8".repeat(40)),
                "exact_file_sha256": "9".repeat(64),
                "source_ceiling_class": "P00_EXACT_CONTRACT_PACK",
            },
            "artifact_candidate": {
                "relative_path": bundle_path,
                "sha256": exact_sha256_hex(&bundle),
                "bytes": bundle.len(),
                "format": ARTIFACT_FORMAT,
                "local_file_is_immutable_artifact_ref": false,
            },
            "candidate_metadata_path": candidate_path,
            "sources": [source],
            "registry_fragments": [fragment],
            "accepted_handoffs": [],
            "required_unavailable_checks": ["real-toolchain"],
            "preflight_checks": [],
            "reason_codes": [],
            "verification": {
                "source_count": 1,
                "registry_fragment_count": 1,
                "accepted_handoff_count": 0,
                "forbidden_path_scan_passed": true,
                "bundle_roundtrip_verified": true,
                "local_output_readback_required": true,
                "authoritative_artifact_store_readback_verified": false,
            },
            "manifest_projection": {
                "target_record_kind": "context_manifest_v1",
                "schema_instance": false,
                "status": "PROJECTION_REQUIRES_EXTERNAL_STORE_DUAL_SIGNATURE_AND_COMMIT",
                "known": {},
                "unresolved_fields": [],
            },
            "ordinary_artifact_writes": [bundle_path, candidate_path],
            "control_record_mutations": [],
            "authority": candidate_authority(),
        });
        let digest = candidate_metadata_digest(&candidate);
        candidate
            .as_object_mut()
            .expect("candidate object")
            .insert("candidate_sha256".to_owned(), Value::String(digest));
        scratch.write(&bundle_path, &bundle);
        scratch.write(&candidate_path, &canonical_json_bytes(&candidate));
        Self {
            scratch,
            candidate_path,
            bundle_path,
            selection_path: "artifacts/context-materialization-inputs/search-contracts.json".to_owned(),
            bundle,
        }
    }

    fn selection(&self) -> Value {
        let artifact = json!({
            "store_profile_ref": "qualified-store-v1",
            "artifact_id": "context-001",
            "bytes": self.bundle.len(),
            "sha256": exact_sha256_hex(&self.bundle),
        });
        json!({
            "context_id": "context-001",
            "created_at": "2026-08-31T00:00:00Z",
            "materializer_identity": "actor:integration:materializer-001",
            "reviewer_identity": "actor:reviewer:context-001",
            "artifact_ref": artifact,
            "artifact_readback": {
                "verified": true,
                "verifier_identity": "actor:reviewer:store-001",
                "verified_at": "2026-08-31T00:00:00Z",
                "sha256": exact_sha256_hex(&self.bundle),
                "bytes": self.bundle.len(),
            },
            "materializer_signature_ref": {"state": "ABSENT", "value": ""},
            "reviewer_signature_ref": {"state": "ABSENT", "value": ""},
        })
    }

    fn write_selection(&self, value: &Value) {
        self.scratch
            .write(&self.selection_path, &canonical_json_bytes(value));
    }

    fn build(&self, selection: Option<&str>) -> xtask::context_materialization_builder::MaterializationBuild {
        build_plan(
            &self.scratch.root,
            &self.candidate_path,
            Some(&self.bundle_path),
            selection,
            "artifacts/context-materialization-plans/test",
        )
        .expect("materialization plan builds")
    }
}

fn signature(actor: &str, suffix: char, digest: &str) -> Value {
    json!({
        "state": "PRESENT",
        "value": {
            "approval_profile_ref": "qualified-approval-v1",
            "approval_artifact_ref": {
                "store_profile_ref": "qualified-approval-store-v1",
                "artifact_id": format!("approval-{suffix}"),
                "bytes": 1,
                "sha256": suffix.to_string().repeat(64),
            },
            "signed_payload_sha256": digest,
            "actor_identity": actor,
        },
    })
}

#[test]
fn decisions_and_operation_identity_follow_the_two_phase_contract() {
    let fixture = Fixture::new();
    let missing = fixture.build(None);
    assert_eq!(
        missing.plan()["decision"].as_str(),
        Some(DECISION_MISSING)
    );
    assert!(missing.payload_bytes().is_none());

    let mut selection = fixture.selection();
    fixture.write_selection(&selection);
    let unsigned = fixture.build(Some(&fixture.selection_path));
    assert_eq!(
        unsigned.plan()["decision"].as_str(),
        Some(DECISION_SIGNATURES)
    );
    let payload_digest = unsigned.plan()["prospective_manifest"]
        ["signed_payload_sha256"]
        .as_str()
        .expect("payload digest")
        .to_owned();
    let operation_id = unsigned.plan()["operation"]["operation_id"]
        .as_str()
        .expect("operation ID")
        .to_owned();
    assert_eq!(
        exact_sha256_hex(unsigned.payload_bytes().expect("payload bytes")),
        payload_digest
    );
    assert!(unsigned.manifest_bytes().is_none());

    selection["materializer_signature_ref"] = signature(
        "actor:integration:materializer-001",
        'c',
        &payload_digest,
    );
    fixture.write_selection(&selection);
    let partial = fixture.build(Some(&fixture.selection_path));
    assert_eq!(
        partial.plan()["decision"].as_str(),
        Some(DECISION_PARTIAL_SIGNATURE)
    );
    assert!(partial.manifest_bytes().is_none());

    selection["reviewer_signature_ref"] = signature(
        "actor:reviewer:context-001",
        'd',
        &payload_digest,
    );
    fixture.write_selection(&selection);
    let complete = fixture.build(Some(&fixture.selection_path));
    assert_eq!(complete.plan()["decision"].as_str(), Some(DECISION_COMMIT));
    assert_eq!(
        complete.plan()["operation"]["operation_id"].as_str(),
        Some(operation_id.as_str())
    );
    let manifest = complete.manifest_bytes().expect("complete manifest");
    let parsed: toml::Value = toml::from_str(
        std::str::from_utf8(manifest).expect("manifest UTF-8"),
    )
    .expect("manifest TOML");
    assert_eq!(parsed.get("status").and_then(toml::Value::as_str), Some("MATERIALIZED"));
    assert_eq!(
        parsed
            .get("signature")
            .and_then(toml::Value::as_table)
            .and_then(|signature| signature.get("record_sha256"))
            .and_then(toml::Value::as_str),
        Some(payload_digest.as_str())
    );
}

#[test]
fn plan_output_is_idempotent_and_conflicts_fail_closed() {
    let fixture = Fixture::new();
    let build = fixture.build(None);
    let first = write_plan(&fixture.scratch.root, &build).expect("first write");
    let second = write_plan(&fixture.scratch.root, &build).expect("equal replay");
    assert_eq!(first, second);
    fs::write(&first[0], b"different\n").expect("tamper ordinary output");
    let error = write_plan(&fixture.scratch.root, &build).expect_err("conflict rejects");
    assert_eq!(error.reason(), "MATERIALIZATION_OUTPUT_CONFLICT");
}

#[test]
fn actor_and_artifact_mismatches_reject() {
    let fixture = Fixture::new();
    let mut selection = fixture.selection();
    selection["reviewer_identity"] = selection["materializer_identity"].clone();
    fixture.write_selection(&selection);
    assert_eq!(
        build_plan(
            &fixture.scratch.root,
            &fixture.candidate_path,
            None,
            Some(&fixture.selection_path),
            "artifacts/context-materialization-plans/test",
        )
        .expect_err("actor conflict rejects")
        .reason(),
        "MATERIALIZATION_ACTOR_CONFLICT"
    );

    let mut selection = fixture.selection();
    selection["artifact_ref"]["sha256"] = Value::String("f".repeat(64));
    fixture.write_selection(&selection);
    assert_eq!(
        build_plan(
            &fixture.scratch.root,
            &fixture.candidate_path,
            None,
            Some(&fixture.selection_path),
            "artifacts/context-materialization-plans/test",
        )
        .expect_err("artifact mismatch rejects")
        .reason(),
        "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH"
    );
}
