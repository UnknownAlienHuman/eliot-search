//! Parity tests for the T41 context-materialization slice (`xtask` vs Python).
//!
//! Covers only the ported IO-free helpers from
//! `tools/context_materialization_planner_v1/core.py`. Vectors in
//! `fixtures/tooling/context-materialization/vectors.json` were captured from
//! `CPython` 3.12 driving the real Python functions. Filesystem
//! reads/writes, candidate assembly (shared `context_artifact_builder_v1`
//! lib), TOML rendering (`manifest.py`) and plan assembly (`plan.py`, both
//! entrypoints) remain Python-owned.

use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::context_materialization::{
    AUTHORITY_FIELDS, DECISION_COMMIT, DECISION_MISSING, DECISION_PARTIAL_SIGNATURE,
    DECISION_SIGNATURES, INSTANCE_STATUS, OPERATION_DOMAIN, PLAN_DOMAIN, PLAN_ROOT,
    REASON_MISSING_SELECTION, REASON_PARTIAL_SIGNATURE, RECORD_KIND, REPOSITORY, SCHEMA_VERSION,
    STATUS, actor_identity_valid, advisory_output_target, authority_map, opaque_id_valid,
    operation_id, plan_digest, require_actor, require_opaque, require_rfc3339, require_sha,
    require_u64, rfc3339_valid, sha256_hex_valid, validate_artifact_ref,
    validate_optional_signature,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/context-materialization/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

fn hex_decode(hex: &str) -> Vec<u8> {
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let digit = |b: u8| {
        if b.is_ascii_digit() {
            b - b'0'
        } else {
            b - b'a' + 10
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        out.push(digit(bytes[i]) << 4 | digit(bytes[i + 1]));
        i += 2;
    }
    out
}

const fn reason_of(
    result: &Result<String, xtask::context_materialization::MaterializationPlanError>,
) -> &str {
    match result {
        Ok(_) => "OK",
        Err(err) => err.reason(),
    }
}

// Closed constants: identity, status, roots, decisions, reasons, domains, registry.
#[test]
fn consts_match_python_pass() {
    let v = vectors();
    assert_eq!(SCHEMA_VERSION, v["schema_version"].as_i64().unwrap());
    assert_eq!(RECORD_KIND, v["record_kind"].as_str().unwrap());
    assert_eq!(STATUS, v["status"].as_str().unwrap());
    assert_eq!(PLAN_ROOT, v["plan_root"].as_str().unwrap());
    assert_eq!(INSTANCE_STATUS, v["instance_status"].as_str().unwrap());
    assert_eq!(REPOSITORY, v["repository"].as_str().unwrap());
    assert_eq!(
        DECISION_MISSING,
        v["decisions"]["missing"].as_str().unwrap()
    );
    assert_eq!(
        DECISION_SIGNATURES,
        v["decisions"]["signatures"].as_str().unwrap()
    );
    assert_eq!(DECISION_COMMIT, v["decisions"]["commit"].as_str().unwrap());
    assert_eq!(
        DECISION_PARTIAL_SIGNATURE,
        v["decisions"]["partial"].as_str().unwrap()
    );
    assert_eq!(
        REASON_MISSING_SELECTION,
        v["reasons"]["missing_selection"].as_str().unwrap()
    );
    assert_eq!(
        REASON_PARTIAL_SIGNATURE,
        v["reasons"]["partial_signature"].as_str().unwrap()
    );
    assert_eq!(
        PLAN_DOMAIN,
        hex_decode(v["plan_domain_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        OPERATION_DOMAIN,
        hex_decode(v["operation_domain_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        AUTHORITY_FIELDS.to_vec(),
        v["authority_fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(AUTHORITY_FIELDS.len(), 9);
    assert_eq!(
        PLAN_DOMAIN,
        b"eliot-search/context-materialization-plan/v1\0"
    );
    assert_eq!(OPERATION_DOMAIN, b"eliot-search/materialize-context/v1\0");
}

// authority_map: all-false ceiling byte-exact.
#[test]
fn authority_map_all_false_pass() {
    let v = vectors();
    assert_eq!(authority_map(), v["authority_map"]);
    let Value::Object(map) = authority_map() else {
        panic!("authority map must be an object");
    };
    assert_eq!(map.len(), 9);
    assert!(map.values().all(|item| item == &Value::Bool(false)));
}

// plan_digest: empty / a1 / nested byte-exact.
#[test]
fn plan_digest_vectors_pass() {
    let v = vectors();
    assert_eq!(
        plan_digest(&json!({})),
        v["plan_digest"]["empty"].as_str().unwrap()
    );
    assert_eq!(
        plan_digest(&json!({"a": 1})),
        v["plan_digest"]["a1"].as_str().unwrap()
    );
    assert_eq!(
        plan_digest(&json!({"m": {"z": [1, 2], "a": true}, "a": "x"})),
        v["plan_digest"]["nested"].as_str().unwrap()
    );
}

// operation_id: empty / a1 byte-exact, domain-separated from plan_digest.
#[test]
fn operation_id_vectors_pass() {
    let v = vectors();
    assert_eq!(
        operation_id(&json!({})),
        v["operation_id"]["empty"].as_str().unwrap()
    );
    assert_eq!(
        operation_id(&json!({"a": 1})),
        v["operation_id"]["a1"].as_str().unwrap()
    );
    assert_ne!(
        operation_id(&json!({"a": 1})),
        plan_digest(&json!({"a": 1}))
    );
    assert_ne!(operation_id(&json!({})), operation_id(&json!({"a": 1})));
}

// require_sha: valid passes, six negatives rejected with exact reason.
#[test]
fn require_sha_vectors_pass() {
    let v = vectors();
    assert!(v["require_sha"]["valid"]["ok"].as_bool().unwrap());
    assert_eq!(
        require_sha(&json!("a".repeat(64)), "s").unwrap(),
        "a".repeat(64)
    );
    for key in ["uppercase", "short", "nibble", "empty", "int", "none"] {
        let row = &v["require_sha"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_INPUT_INVALID"
        );
    }
    assert_eq!(
        reason_of(&require_sha(&json!("A".repeat(64)), "s")),
        "MATERIALIZATION_INPUT_INVALID"
    );
    assert!(sha256_hex_valid(&"a".repeat(64)));
    assert!(!sha256_hex_valid(&"A".repeat(64)));
    assert_eq!(
        sha256_hex_valid("abc"),
        xtask::ticket_planner::sha256_hex_valid("abc")
    );
}

// require_opaque: three passes, six negatives rejected.
#[test]
fn require_opaque_vectors_pass() {
    let v = vectors();
    assert!(v["require_opaque"]["single"]["ok"].as_bool().unwrap());
    assert!(v["require_opaque"]["mixed"]["ok"].as_bool().unwrap());
    assert!(v["require_opaque"]["max128"]["ok"].as_bool().unwrap());
    assert_eq!(
        require_opaque(&json!("a".repeat(128)), "o").unwrap(),
        "a".repeat(128)
    );
    for key in ["over129", "leading_dash", "slash", "empty", "space", "int"] {
        let row = &v["require_opaque"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_INPUT_INVALID"
        );
    }
    assert!(opaque_id_valid("abc-123_X.Y"));
    assert!(!opaque_id_valid("-abc"));
    assert_eq!(
        opaque_id_valid("abc"),
        xtask::ticket_planner::opaque_id_valid("abc")
    );
}

// require_actor: four roles pass, four negatives rejected.
#[test]
fn require_actor_vectors_pass() {
    let v = vectors();
    for key in ["user", "service", "reviewer", "integration"] {
        assert!(
            v["require_actor"][key]["ok"].as_bool().unwrap(),
            "{key} must pass"
        );
    }
    assert_eq!(
        require_actor(&json!("actor:user:abc"), "a").unwrap(),
        "actor:user:abc"
    );
    for key in ["bad_role", "no_prefix", "empty_id", "bad_id"] {
        let row = &v["require_actor"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_INPUT_INVALID"
        );
    }
    assert!(actor_identity_valid("actor:reviewer:x-1_Y.Z"));
    assert!(!actor_identity_valid("actor:admin:x"));
    assert_eq!(
        actor_identity_valid("actor:user:abc"),
        xtask::ticket_planner::actor_identity_valid("actor:user:abc")
    );
}

// require_rfc3339: four passes; calendar vs shape messages pinned.
#[test]
fn require_rfc3339_vectors_pass() {
    let v = vectors();
    for key in ["valid", "leap", "y2k_leap", "max"] {
        assert!(
            v["require_rfc3339"][key]["ok"].as_bool().unwrap(),
            "{key} must pass"
        );
    }
    assert_eq!(
        require_rfc3339(&json!("2026-08-31T00:00:00Z"), "t").unwrap(),
        "2026-08-31T00:00:00Z"
    );
    assert!(rfc3339_valid("2024-02-29T12:00:00Z"));
    assert!(!rfc3339_valid("2026-02-30T00:00:00Z"));
    for key in [
        "feb30", "non_leap", "century", "month13", "hour24", "sec60", "year0",
    ] {
        let row = &v["require_rfc3339"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_INPUT_INVALID"
        );
        assert!(
            row["message"]
                .as_str()
                .unwrap()
                .ends_with("is not a valid calendar timestamp"),
            "{key} calendar message"
        );
    }
    for key in ["offset", "date_only", "no_pad", "int"] {
        let row = &v["require_rfc3339"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert!(
            row["message"]
                .as_str()
                .unwrap()
                .ends_with("is not whole-second UTC RFC3339"),
            "{key} shape message"
        );
    }
    assert!(require_rfc3339(&json!("1900-02-29T00:00:00Z"), "t").is_err());
    assert!(require_rfc3339(&json!("2000-02-29T00:00:00Z"), "t").is_ok());
}

// require_u64: zero/one/max pass with values; six negatives rejected.
#[test]
fn require_u64_vectors_pass() {
    let v = vectors();
    assert_eq!(v["require_u64"]["zero"]["value"].as_u64().unwrap(), 0);
    assert_eq!(v["require_u64"]["one"]["value"].as_u64().unwrap(), 1);
    assert_eq!(v["require_u64"]["max"]["value"].as_u64().unwrap(), u64::MAX);
    assert_eq!(require_u64(&json!(0), "n").unwrap(), 0);
    assert_eq!(require_u64(&json!(u64::MAX), "n").unwrap(), u64::MAX);
    for key in ["overflow", "negative", "bool", "float", "str", "none"] {
        let row = &v["require_u64"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_INPUT_INVALID"
        );
    }
    assert!(require_u64(&json!(-1), "n").is_err());
    assert!(require_u64(&json!(true), "n").is_err());
}

// advisory_output_target: five passes with targets, five negatives rejected.
#[test]
fn advisory_output_roots_pass() {
    let v = vectors();
    for key in [
        "root",
        "descendant",
        "nested",
        "backslash",
        "trailing_slash",
    ] {
        let row = &v["output_roots"][key];
        assert!(row["ok"].as_bool().unwrap(), "{key} must pass");
        assert_eq!(
            advisory_output_target(match key {
                "root" => "artifacts/context-materialization-plans",
                "descendant" => "artifacts/context-materialization-plans/validation",
                "nested" => "artifacts/context-materialization-plans/workflow/search-contracts",
                "backslash" => "artifacts\\context-materialization-plans\\validation",
                _ => "artifacts/context-materialization-plans/",
            })
            .unwrap(),
            row["target"].as_str().unwrap()
        );
    }
    for key in ["outside", "absolute", "traversal", "double_slash", "empty"] {
        let row = &v["output_roots"][key];
        assert!(!row["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            row["reason"].as_str().unwrap(),
            "MATERIALIZATION_OUTPUT_PATH_INVALID"
        );
    }
    assert!(advisory_output_target("artifacts/context-materialization-plans//validation").is_err());
    assert!(advisory_output_target("artifacts/context-artifact-candidates").is_err());
}

// validate_artifact_ref: ok fields byte-exact; five negatives with exact reasons.
#[test]
fn artifact_ref_vectors_pass() {
    let v = vectors();
    let bundle = b"hello-bundle";
    let normalized = validate_artifact_ref(
        &json!({
            "store_profile_ref": "qualified-store-v1",
            "artifact_id": "context-001",
            "bytes": bundle.len(),
            "sha256": xtask::ticket_planner::exact_sha256_hex(bundle),
        }),
        bundle,
    )
    .unwrap();
    assert_eq!(normalized.store_profile_ref, "qualified-store-v1");
    assert_eq!(normalized.artifact_id, "context-001");
    assert_eq!(normalized.bytes, u64::try_from(bundle.len()).unwrap());
    assert_eq!(
        normalized.sha256,
        v["artifact_ref"]["ok"]["value"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        validate_artifact_ref(&json!({}), bundle)
            .unwrap_err()
            .reason(),
        "MATERIALIZATION_ARTIFACT_REF_INVALID"
    );
    assert_eq!(
        v["artifact_ref"]["field_set_empty"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_ARTIFACT_REF_INVALID"
    );
    assert_eq!(
        v["artifact_ref"]["field_set_extra"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_ARTIFACT_REF_INVALID"
    );
    assert_eq!(
        v["artifact_ref"]["sha_mismatch"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH"
    );
    assert_eq!(
        v["artifact_ref"]["bytes_mismatch"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH"
    );
    assert_eq!(
        v["artifact_ref"]["bad_opaque"]["reason"].as_str().unwrap(),
        "MATERIALIZATION_INPUT_INVALID"
    );
}

// validate_optional_signature: absent/present pass; six negatives with exact reasons.
#[test]
fn optional_signature_vectors_pass() {
    let v = vectors();
    let absent = validate_optional_signature(
        &json!({"state": "ABSENT", "value": ""}),
        "actor:user:a",
        "sel",
    )
    .unwrap();
    assert_eq!(absent.state, "ABSENT");
    assert!(absent.value.is_none());
    assert_eq!(
        v["optional_signature"]["absent"]["state"].as_str().unwrap(),
        "ABSENT"
    );

    let present = validate_optional_signature(
        &json!({
            "state": "PRESENT",
            "value": {
                "approval_profile_ref": "qualified-approval-v1",
                "approval_artifact_ref": {
                    "store_profile_ref": "s",
                    "artifact_id": "a",
                    "bytes": 1,
                    "sha256": "c".repeat(64),
                },
                "signed_payload_sha256": "e".repeat(64),
                "actor_identity": "actor:user:a",
            },
        }),
        "actor:user:a",
        "sel",
    )
    .unwrap();
    assert_eq!(present.state, "PRESENT");
    let inner = present.value.unwrap();
    assert_eq!(inner.actor_identity, "actor:user:a");
    assert_eq!(inner.signed_payload_sha256, "e".repeat(64));
    assert_eq!(inner.approval_profile_ref, "qualified-approval-v1");

    assert_eq!(
        v["optional_signature"]["actor_mismatch"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH"
    );
    for key in [
        "absent_with_value",
        "bad_state",
        "bad_shape",
        "bad_field_set",
    ] {
        assert_eq!(
            v["optional_signature"][key]["reason"].as_str().unwrap(),
            "MATERIALIZATION_SIGNATURE_REF_INVALID",
            "{key} reason"
        );
    }
    assert_eq!(
        v["optional_signature"]["bad_actor_grammar"]["reason"]
            .as_str()
            .unwrap(),
        "MATERIALIZATION_INPUT_INVALID"
    );
    assert!(
        validate_optional_signature(
            &json!({"state": "ABSENT", "value": ""}),
            "actor:user:a",
            "sel",
        )
        .is_ok()
    );
    assert!(
        validate_optional_signature(
            &json!({"state": "ABSENT", "value": "x"}),
            "actor:user:a",
            "sel",
        )
        .is_err()
    );
}
