//! Parity tests for the T41 context-artifact slice (`xtask` vs Python).
//!
//! Covers only the ported IO-free helpers from
//! `tools/context_artifact_builder_v1/core.py` and `bundle.py`. Vectors in
//! `fixtures/tooling/context-artifact/vectors.json` were captured from
//! `CPython` 3.12 driving the real Python functions. Git-tree reads,
//! preflight/extraction, candidate assembly, idempotent writes and both
//! `build`/`validate` entrypoints remain Python-owned.

use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::context_artifact::{
    ADDITIONAL_FAILURE_CODES, ARTIFACT_FORMAT, ARTIFACT_ROOT, AUTHORITY_FIELDS, BUNDLE_END,
    BUNDLE_MAGIC, BundleBlock, CANDIDATE_ID_DOMAIN, CANDIDATE_METADATA_DOMAIN, MAX_BUNDLE_BYTES,
    RECORD_KIND, SCHEMA_VERSION, STATUS, UNRESOLVED_MANIFEST_FIELDS, advisory_output_target,
    assert_candidate_digest, authority_map, candidate_id, candidate_metadata_digest,
    expected_header, normalize_utf8_lf, parse_bundle, render_bundle, require_json_value,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/context-artifact/vectors.json");
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

// Closed constants: identity, format, ceilings, domains, registries.
#[test]
fn consts_match_python_pass() {
    let v = vectors();
    assert_eq!(SCHEMA_VERSION, v["schema_version"].as_i64().unwrap());
    assert_eq!(RECORD_KIND, v["record_kind"].as_str().unwrap());
    assert_eq!(STATUS, v["status"].as_str().unwrap());
    assert_eq!(ARTIFACT_FORMAT, v["artifact_format"].as_str().unwrap());
    assert_eq!(ARTIFACT_ROOT, v["artifact_root"].as_str().unwrap());
    assert_eq!(
        MAX_BUNDLE_BYTES,
        usize::try_from(v["max_bundle_bytes"].as_u64().unwrap()).unwrap()
    );
    assert_eq!(MAX_BUNDLE_BYTES, 20 * 1024 * 1024);
    assert_eq!(
        BUNDLE_MAGIC,
        hex_decode(v["bundle_magic_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        BUNDLE_END,
        hex_decode(v["bundle_end_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        CANDIDATE_ID_DOMAIN,
        hex_decode(v["candidate_id_domain_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        CANDIDATE_METADATA_DOMAIN,
        hex_decode(v["candidate_metadata_domain_hex"].as_str().unwrap()).as_slice()
    );
    assert_eq!(
        ADDITIONAL_FAILURE_CODES.to_vec(),
        v["additional_failure_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect::<Vec<_>>()
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
    assert_eq!(
        UNRESOLVED_MANIFEST_FIELDS.to_vec(),
        v["unresolved_manifest_fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(AUTHORITY_FIELDS.len(), 9);
    assert_eq!(UNRESOLVED_MANIFEST_FIELDS.len(), 14);
}

// normalize_utf8_lf: LF/CRLF/lone-CR pass through byte-exact.
#[test]
fn normalize_ok_vectors_pass() {
    let v = &vectors()["normalize"];
    for key in ["lf", "crlf", "lonecr", "mixed", "empty"] {
        let row = &v[key];
        assert!(row["ok"].as_bool().unwrap(), "{key} must pass");
    }
    assert_eq!(
        normalize_utf8_lf(b"# root\r\nsecond\r\n").unwrap(),
        b"# root\nsecond\n"
    );
    assert_eq!(normalize_utf8_lf(b"a\rb\r").unwrap(), b"a\nb\n");
    assert_eq!(normalize_utf8_lf(b"a\nb\n").unwrap(), b"a\nb\n");
    assert_eq!(normalize_utf8_lf(b"x\r\ny\rz\n").unwrap(), b"x\ny\nz\n");
    assert_eq!(normalize_utf8_lf(b"").unwrap(), b"");
}

// normalize_utf8_lf: non-UTF8 and NUL rejected with exact reason codes.
#[test]
fn normalize_failures_rejected() {
    let v = &vectors()["normalize"];
    assert_eq!(
        v["nonutf8"]["reason"].as_str().unwrap(),
        "CONTEXT_SOURCE_NOT_UTF8"
    );
    assert_eq!(
        v["nul"]["reason"].as_str().unwrap(),
        "CONTEXT_SOURCE_CONTAINS_NUL"
    );
    assert_eq!(
        normalize_utf8_lf(b"\xff\xfe").unwrap_err().reason(),
        "CONTEXT_SOURCE_NOT_UTF8"
    );
    assert_eq!(
        normalize_utf8_lf(b"root\x00value\n").unwrap_err().reason(),
        "CONTEXT_SOURCE_CONTAINS_NUL"
    );
}

// require_json_value: strings/ints/bools/containers pass; null/floats fail.
#[test]
fn require_json_matrix_pass() {
    let v = &vectors()["require_json"];
    for key in [
        "str",
        "int",
        "bool",
        "neg",
        "big",
        "list",
        "dict",
        "empty_list",
        "empty_dict",
    ] {
        assert!(v[key]["ok"].as_bool().unwrap(), "{key} must pass");
    }
    for key in ["null", "float", "nested_null", "nested_float"] {
        assert!(!v[key]["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            v[key]["reason"].as_str().unwrap(),
            "REGISTRY_FRAGMENT_NONCANONICAL"
        );
    }
    assert!(require_json_value(&json!("x")).is_ok());
    assert!(require_json_value(&json!(3)).is_ok());
    assert!(require_json_value(&json!({"a": [1, {"k": "v"}]})).is_ok());
    assert_eq!(
        require_json_value(&Value::Null).unwrap_err().reason(),
        "REGISTRY_FRAGMENT_NONCANONICAL"
    );
    assert_eq!(
        require_json_value(&json!(1.5)).unwrap_err().reason(),
        "REGISTRY_FRAGMENT_NONCANONICAL"
    );
    assert_eq!(
        require_json_value(&json!({"a": [1, Value::Null]}))
            .unwrap_err()
            .reason(),
        "REGISTRY_FRAGMENT_NONCANONICAL"
    );
}

// candidate_id: domain-separated SHA-256 pinned on two inputs.
#[test]
fn candidate_id_vectors_pass() {
    let v = &vectors()["candidate_id"];
    assert_eq!(candidate_id(b""), v["empty_bundle"].as_str().unwrap());
    assert_eq!(candidate_id(b"abc"), v["abc"].as_str().unwrap());
    assert_ne!(candidate_id(b"abc"), candidate_id(b"abd"));
}

// candidate digest: pinned digest, ok roundtrip, tamper/missing rejected.
#[test]
fn candidate_digest_vectors_pass() {
    let v = vectors();
    let payload = json!({"a": [true, "x"], "b": 1});
    // Note: key order is canonicalized, so construction order is irrelevant.
    assert_eq!(
        candidate_metadata_digest(&payload),
        v["metadata_digest"].as_str().unwrap()
    );
    assert!(v["assert_digest_ok"].as_bool().unwrap());
    assert!(!v["assert_digest_tampered"].as_bool().unwrap());
    assert!(!v["assert_digest_missing"].as_bool().unwrap());
    let mut good = payload.clone();
    good["candidate_sha256"] = Value::String(candidate_metadata_digest(&payload));
    assert!(assert_candidate_digest(&good));
    let mut bad = good.clone();
    bad["b"] = json!(2);
    assert!(!assert_candidate_digest(&bad));
    assert!(!assert_candidate_digest(&payload));
}

// authority_map: exact all-false ceiling.
#[test]
fn authority_map_vectors_pass() {
    let v = vectors();
    assert_eq!(authority_map(), v["authority_map"]);
    let Value::Object(map) = authority_map() else {
        panic!("authority map is an object");
    };
    assert_eq!(map.len(), 9);
    assert!(map.values().all(|x| x == &Value::Bool(false)));
}

// advisory output-root grammar: root/descendant/backslash pass; escapes fail.
#[test]
fn output_root_matrix_pass() {
    let v = &vectors()["output_roots"];
    for key in ["root", "descendant", "nested", "backslash"] {
        let row = &v[key];
        assert!(row["ok"].as_bool().unwrap(), "{key} must pass");
        assert_eq!(
            advisory_output_target(match key {
                "root" => ARTIFACT_ROOT,
                "descendant" => "artifacts/context-artifact-candidates/validation",
                "nested" => "artifacts/context-artifact-candidates/workflow/search-contracts",
                _ => "artifacts\\context-artifact-candidates\\validation",
            })
            .unwrap(),
            row["target"].as_str().unwrap()
        );
    }
    for key in ["outside", "dot", "traversal", "absolute"] {
        assert!(!v[key]["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            v[key]["reason"].as_str().unwrap(),
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT"
        );
    }
    assert_eq!(
        advisory_output_target("artifacts/elsewhere")
            .unwrap_err()
            .reason(),
        "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT"
    );
    assert_eq!(
        advisory_output_target("artifacts/context-artifact-candidates/.")
            .unwrap_err()
            .reason(),
        "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT"
    );
}

// expected_header: three kinds pinned; unknown kind rejected.
#[test]
fn headers_matrix_pass() {
    let v = &vectors()["headers"];
    assert_eq!(
        expected_header("source", &json!({"repository_path": "AGENTS.md"})).unwrap(),
        v["source"]["header"].as_str().unwrap()
    );
    assert_eq!(
        expected_header(
            "registry_fragment",
            &json!({"registry_path": "swarm/crates.toml", "selector": "package[name=search-contracts]"})
        )
        .unwrap(),
        v["fragment"]["header"].as_str().unwrap()
    );
    assert_eq!(
        expected_header("accepted_handoff", &json!({"package": "search-contracts"})).unwrap(),
        v["handoff"]["header"].as_str().unwrap()
    );
    assert_eq!(
        v["unknown"]["reason"].as_str().unwrap(),
        "BUNDLE_FORMAT_INVALID"
    );
    assert_eq!(
        expected_header("bogus", &json!({})).unwrap_err().reason(),
        "BUNDLE_FORMAT_INVALID"
    );
}

// render_bundle: byte-exact with CPython (length framing tolerates header-like bytes).
#[test]
fn bundle_render_bytes_match_python_pass() {
    let v = vectors();
    let want = hex_decode(v["bundle_hex"].as_str().unwrap());
    let preamble: Value = serde_json::from_value(v["bundle_roundtrip_preamble"].clone()).unwrap();
    let contents: Vec<Vec<u8>> = v["bundle_roundtrip_contents_hex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| hex_decode(x.as_str().unwrap()))
        .collect();
    let headers: Vec<String> = v["bundle_roundtrip_headers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    let kinds: Vec<String> = v["bundle_roundtrip_kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    let meta0: Value = serde_json::from_value(v["bundle_roundtrip_meta0"].clone()).unwrap();
    // Rebuild the three pinned blocks from roundtripped metadata/contents.
    let frag_meta: Value = {
        let parsed = parse_bundle(&want).unwrap().1;
        parsed[1].metadata.clone()
    };
    let handoff_meta: Value = {
        let parsed = parse_bundle(&want).unwrap().1;
        parsed[2].metadata.clone()
    };
    let blocks = vec![
        BundleBlock {
            kind: kinds[0].clone(),
            header: headers[0].clone(),
            metadata: meta0,
            content: contents[0].clone(),
        },
        BundleBlock {
            kind: kinds[1].clone(),
            header: headers[1].clone(),
            metadata: frag_meta,
            content: contents[1].clone(),
        },
        BundleBlock {
            kind: kinds[2].clone(),
            header: headers[2].clone(),
            metadata: handoff_meta,
            content: contents[2].clone(),
        },
    ];
    // Header-like source bytes survive length framing.
    assert_eq!(
        contents[0],
        b"--- end-context-artifact ---\n--- repository-path: fake ---\n"
    );
    assert_eq!(render_bundle(&preamble, &blocks).unwrap(), want);
    assert_eq!(
        want.len(),
        usize::try_from(v["bundle_len"].as_u64().unwrap()).unwrap()
    );
}

// parse_bundle: roundtrip fields plus every failure pinned to BUNDLE_FORMAT_INVALID.
#[test]
fn bundle_parse_roundtrip_and_failures_pass() {
    let v = vectors();
    let bundle = hex_decode(v["bundle_hex"].as_str().unwrap());
    let (preamble, blocks) = parse_bundle(&bundle).unwrap();
    assert_eq!(preamble, v["bundle_roundtrip_preamble"]);
    let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
    assert_eq!(kinds, ["source", "registry_fragment", "accepted_handoff"]);
    assert_eq!(
        blocks[0].content,
        hex_decode(v["bundle_roundtrip_contents_hex"][0].as_str().unwrap())
    );
    let failures = &v["parse_failures"];
    for key in [
        "bad_magic",
        "truncated",
        "bad_header",
        "digest_mismatch",
        "trailing",
        "count_mismatch",
    ] {
        assert!(!failures[key]["ok"].as_bool().unwrap(), "{key} must fail");
        assert_eq!(
            failures[key]["reason"].as_str().unwrap(),
            "BUNDLE_FORMAT_INVALID"
        );
    }
    assert!(parse_bundle(b"BAD\n{}").is_err());
    assert!(parse_bundle(&bundle[..20]).is_err());
    assert!(parse_bundle(&[bundle.clone(), b"X".to_vec()].concat()).is_err());
    // Wrong header is rejected, not re-framed.
    let mut bad = bundle;
    let needle = b"--- repository-path: AGENTS.md ---".to_vec();
    let pos = bad.windows(needle.len()).position(|w| w == needle).unwrap();
    bad.splice(
        pos..pos + needle.len(),
        b"--- repository-path: WRONG ---".to_vec(),
    );
    assert_eq!(
        parse_bundle(&bad).unwrap_err().reason(),
        "BUNDLE_FORMAT_INVALID"
    );
}
