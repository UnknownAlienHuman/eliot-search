//! Parity tests for the T41 family-E port (`xtask` vs Python).
//!
//! Mirrors `qualification/accepted-evidence/test_accepted_evidence_digest_v1.py`
//! and `qualification/accepted-evidence/cases-v1.toml` (10 cases: 4 PASS,
//! 6 REJECT), plus byte vectors in `fixtures/tooling/accepted-evidence/`.

use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::accepted_evidence::{
    MAGIC, accepted_evidence_digest_json, canonical_json_bytes, render_evidence_manifest_json,
    result_record_json,
};
use xtask::compute_accepted_evidence::compute_from_record_file;
use xtask::validate_accepted_evidence::{
    exit_code, render_report_json, validate_accepted_evidence_digest,
};

fn record(requirement: &str, suffix: &str) -> Value {
    let artifact_sha = suffix.repeat(64);
    json!({
        "requirement_id": requirement,
        "evidence_class": "CONTRACT_TEST",
        "artifact_ref": {
            "store_profile_ref": "qualified-store-v1",
            "artifact_id": format!("artifact-{requirement}"),
            "bytes": 12,
            "sha256": artifact_sha,
        },
        "artifact_sha256": artifact_sha,
        "raw_outcome_digest": "a".repeat(64),
        "availability": "AVAILABLE",
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn hex_decode(hex: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let digit = |b: u8| {
            if b.is_ascii_digit() {
                b - b'0'
            } else {
                b - b'a' + 10
            }
        };
        out.push(digit(bytes[i]) << 4 | digit(bytes[i + 1]));
        i += 2;
    }
    out
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/accepted-evidence/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

// empty_evidence: PASS.
#[test]
fn empty_evidence_pass() {
    let empty = Value::Array(Vec::new());
    assert_eq!(
        render_evidence_manifest_json(&empty).expect("empty renders"),
        MAGIC
    );
    assert_eq!(
        accepted_evidence_digest_json(&empty).expect("empty digests"),
        vectors()["empty_digest"].as_str().expect("vector digest")
    );
}

// deterministic_repeat: PASS.
#[test]
fn deterministic_repeat_pass() {
    let value = json!([record("one", "1"), record("two", "2")]);
    assert_eq!(
        accepted_evidence_digest_json(&value).expect("first"),
        accepted_evidence_digest_json(&value).expect("second")
    );
    assert_eq!(result_record_json(&value).expect("result").record_count, 2);
}

// exact_magic_and_canonical_json: PASS.
#[test]
fn exact_magic_and_canonical_json_pass() {
    let vectors = vectors();
    assert_eq!(
        MAGIC,
        hex_decode(vectors["magic_hex"].as_str().expect("magic vector")).as_slice()
    );
    let one = record("one", "1");
    let manifest = render_evidence_manifest_json(&json!([one])).expect("manifest renders");
    assert_eq!(
        manifest,
        hex_decode(
            vectors["one_manifest_hex"]
                .as_str()
                .expect("manifest vector")
        )
        .as_slice()
    );
    assert_eq!(
        accepted_evidence_digest_json(&json!([one])).expect("digest"),
        vectors["one_digest"].as_str().expect("digest vector")
    );
}

// evidence_order_changes_digest: PASS.
#[test]
fn evidence_order_changes_digest_pass() {
    let vectors = vectors();
    let forward = accepted_evidence_digest_json(&json!([record("one", "1"), record("two", "2")]))
        .expect("forward");
    let swapped = accepted_evidence_digest_json(&json!([record("two", "2"), record("one", "1")]))
        .expect("swapped");
    assert_eq!(
        forward,
        vectors["two_digest"].as_str().expect("forward vector")
    );
    assert_eq!(
        swapped,
        vectors["two_swapped_digest"]
            .as_str()
            .expect("swapped vector")
    );
    assert_ne!(forward, swapped);
}

// result record fields match the Python vectors.
#[test]
fn result_record_matches_python_vectors() {
    let vectors = vectors();
    let one = record("one", "1");
    let result = result_record_json(&json!([one])).expect("result");
    assert_eq!(result.to_compact_json(), vectors["one_result"].to_string());
    assert_eq!(result.manifest_bytes, 467);
    assert_eq!(
        result.evidence_digest,
        vectors["one_result"]["evidence_digest"]
            .as_str()
            .expect("digest")
    );
}

// duplicate_requirement_id: REJECT.
#[test]
fn duplicate_requirement_id_rejected() {
    let value = json!([record("same", "1"), record("same", "2")]);
    assert!(render_evidence_manifest_json(&value).is_err());
}

// unknown_evidence_field: REJECT.
#[test]
fn unknown_evidence_field_rejected() {
    let mut value = record("one", "1");
    value["extra"] = Value::Bool(true);
    assert!(render_evidence_manifest_json(&json!([value])).is_err());
}

// artifact_digest_mismatch: REJECT.
#[test]
fn artifact_digest_mismatch_rejected() {
    let mut value = record("one", "1");
    value["artifact_sha256"] = Value::String("b".repeat(64));
    assert!(render_evidence_manifest_json(&json!([value])).is_err());
}

// invalid_identifier: REJECT.
#[test]
fn invalid_identifier_rejected() {
    assert!(render_evidence_manifest_json(&json!([record("bad/id", "1")])).is_err());
}

// null_or_float: REJECT (both sub-cases).
#[test]
fn null_and_float_rejected() {
    let mut float_case = record("one", "1");
    float_case["artifact_ref"]["bytes"] = json!(1.5);
    assert!(render_evidence_manifest_json(&json!([float_case])).is_err());
    let mut null_case = record("one", "1");
    null_case["availability"] = Value::Null;
    assert!(render_evidence_manifest_json(&json!([null_case])).is_err());
}

// evidence_count_overflow: REJECT.
#[test]
fn evidence_count_overflow_rejected() {
    let items: Vec<Value> = (0..257).map(|i| record(&format!("r{i}"), "1")).collect();
    assert!(render_evidence_manifest_json(&Value::Array(items)).is_err());
}

// Canonical JSON of a normalized record matches the Python bytes exactly.
#[test]
fn canonical_json_bytes_match_python() {
    let vectors = vectors();
    let one = record("one", "1");
    let manifest = render_evidence_manifest_json(&json!([one])).expect("renders");
    let expected_magic = hex_decode(vectors["magic_hex"].as_str().expect("magic"));
    let expected_canonical = hex_decode(vectors["one_canonical_hex"].as_str().expect("canonical"));
    assert_eq!(&manifest[..expected_magic.len()], expected_magic.as_slice());
    assert_eq!(
        &manifest[expected_magic.len()..],
        expected_canonical.as_slice()
    );
    let _ = canonical_json_bytes;
}

// Validator passes on the clean repository.
#[test]
fn validator_passes_on_clean_repo() {
    let report = validate_accepted_evidence_digest(&repo_root());
    assert_eq!(exit_code(&report), 0, "errors: {:?}", report.errors);
    assert_eq!(report.status, "PASS");
    assert_eq!(report.cases, 10);
    assert!(render_report_json(&report).contains("\"status\": \"PASS\""));
}

// Zero-fixture runners fail: missing root cannot pass.
#[test]
fn validator_fails_without_fixtures() {
    let missing = repo_root().join("target/__xtask_definitely_missing_root__");
    let report = validate_accepted_evidence_digest(&missing);
    assert_eq!(exit_code(&report), 1);
    assert_eq!(report.status, "FAIL");
    assert!(!report.errors.is_empty());
}

// Compute rejects invalid records with the same causal prefix behavior.
#[test]
fn compute_rejects_invalid_record() {
    let missing = repo_root().join("target/__xtask_definitely_missing_record__.toml");
    assert!(compute_from_record_file(&missing, false).is_err());
    assert!(compute_from_record_file(&missing, true).is_err());
}

// Compute accepts a JSON evidence array and matches the Python digest.
#[test]
fn compute_accepts_json_array() {
    let vectors = vectors();
    let dir = std::env::temp_dir().join(format!("xtask-parity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("evidence.json");
    let one = record("one", "1");
    std::fs::write(&path, serde_json::to_string(&json!([one])).expect("json")).expect("write");
    let output = compute_from_record_file(&path, true).expect("computes");
    let parsed: Value = serde_json::from_str(&output).expect("compact json");
    assert_eq!(
        parsed["evidence_digest"].as_str().expect("digest"),
        vectors["one_digest"].as_str().expect("vector")
    );
    assert_eq!(parsed["manifest_bytes"].as_u64().expect("bytes"), 467);
    assert_eq!(parsed["record_count"].as_u64().expect("count"), 1);
    std::fs::remove_file(&path).ok();
    std::fs::remove_dir(&dir).ok();
}
