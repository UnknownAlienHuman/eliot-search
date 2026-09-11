//! Parity tests for the T41 ticket slice (`xtask` vs Python).
//!
//! `ticket_planner` pure helpers are pinned against `CPython` 3.12 vectors in
//! `fixtures/tooling/tickets/vectors.json` (captured via `probe_tickets.py`,
//! which drives the real `ticket_issuance_planner_v2` functions, including
//! `resolve_selector` and `expected_contract_pack_sources` through stub
//! `GitView`s). `ticket_drafts` (full port of
//! `tools/p00_ticket_drafts_validator.py`) is pinned against the live-repo
//! Python report captured in the same vectors file.

use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::ticket_drafts::{
    CONTROL_ROOTS as DRAFT_CONTROL_ROOTS, EXACT_PACK_SOURCE_CEILING as DRAFT_EXACT,
    MAX_HANDOFF_SLOTS as DRAFT_HANDOFFS, MAX_REGISTRY_FRAGMENTS as DRAFT_FRAGMENTS,
    ORDINARY_SOURCE_CEILING as DRAFT_ORDINARY, P00_REQUIRED_FILE_COUNT as DRAFT_REQUIRED,
    PACKAGES as DRAFT_PACKAGES,
};
use xtask::ticket_drafts::{exit_code, render_report_json, validate_p00_ticket_drafts};
use xtask::ticket_planner::{
    CLOSED_REASON_CODES, CONFLICT_REASONS, CONTEXT_ALLOWED, CONTEXT_CANONICALIZATION_FIELDS,
    CONTEXT_CONTENT_FIELDS, CONTEXT_TOTAL_BYTE_CEILING, CONTROL_ROOTS,
    CURRENT_PACKAGE_RECORD_ROOTS, DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING,
    DECISION_PREREQUISITE, DECISION_READY, DOMAIN_SEPARATOR, HARD_TOTAL_LINES, INVALID_REASONS,
    PLAN_ARTIFACT_ROOT, PLAN_BYTE_CEILING, PLANNER_FILE_BYTE_CEILING, PREREQUISITE_REASONS,
    RECORD_KIND, REPOSITORY_NAME, ROOT_METADATA_NAMES, SCHEMA_VERSION, SPLIT_REVIEW_TOTAL_LINES,
    STATUS, SelectorDocs, SelectorStatus, TICKET_ALLOWED, TICKET_CONTEXT_FIELDS,
    TICKET_DELIVERABLES_FIELDS, TICKET_DEPENDENCIES_FIELDS, TICKET_LIMITS_FIELDS,
    TICKET_REPOSITORY_FENCE_FIELDS, TICKET_UNRESOLVED_IDENTITY_FIELDS, actor_identity_valid,
    advisory_output_path_valid, advisory_output_selectable, canonical_json_bytes, choose_decision,
    context_source_forbidden, context_total_bytes_ok, contract_pack_sources, exact_sha256_hex,
    expected_handoff_slots, expected_required_handoffs, line_limits_ok, one_table, opaque_id_valid,
    package_name_valid, plan_digest, resolve_selector, safe_path, select_ceiling, selection_state,
    sha256_hex_valid, signed_payload_digest, tagged_git_valid, under, unknown_fields,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/tickets/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

fn str_vec(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0F)] as char);
    }
    out
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

fn opt_str(value: &Value) -> Option<&str> {
    if value.is_null() {
        None
    } else {
        Some(value.as_str().unwrap())
    }
}

// p00 live-repo report matches the captured Python report byte-for-value.
#[test]
fn p00_live_report_matches_python_pass() {
    let v = vectors();
    let current = &v["p00_current"];
    let report = validate_p00_ticket_drafts(&repo_root());
    assert_eq!(
        exit_code(&report),
        i32::try_from(current["exit"].as_i64().unwrap()).unwrap()
    );
    assert!(report.complete);
    assert_eq!(report.passed, current["status"] == "PASS");
    let as_count = |value: &Value| usize::try_from(value.as_u64().unwrap()).unwrap();
    assert_eq!(report.ticket_drafts, as_count(&current["ticket_drafts"]));
    assert_eq!(report.context_drafts, as_count(&current["context_drafts"]));
    assert_eq!(
        report.p00_required_files,
        as_count(&current["p00_required_files"])
    );
    assert_eq!(
        report.search_contracts_sources,
        as_count(&current["search_contracts_sources"])
    );
    assert_eq!(
        report.search_domain_sources,
        as_count(&current["search_domain_sources"])
    );
    assert_eq!(
        report.search_ports_sources,
        as_count(&current["search_ports_sources"])
    );
    assert_eq!(
        report.active_stage.as_deref(),
        current["active_stage"].as_str()
    );
    assert_eq!(report.active_wave, current["active_wave"].as_i64());
    assert_eq!(report.errors, str_vec(&current["errors"]));
    // Rendered JSON parses back to the same values with sorted keys.
    let rendered: Value =
        serde_json::from_str(&render_report_json(&report)).expect("report renders JSON");
    assert_eq!(rendered["status"], current["status"]);
    assert_eq!(rendered["ticket_drafts"], current["ticket_drafts"]);
    let keys: Vec<&str> = rendered
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted);
}

// p00 constants: packages, control roots, ceilings.
#[test]
fn p00_constants_match_python_pass() {
    assert_eq!(
        DRAFT_PACKAGES,
        ["search-contracts", "search-domain", "search-ports"]
    );
    assert_eq!(DRAFT_CONTROL_ROOTS.len(), 9);
    assert_eq!(DRAFT_CONTROL_ROOTS[0], "swarm/context-manifests");
    assert_eq!(DRAFT_CONTROL_ROOTS[8], "swarm/wave-receipts");
    assert_eq!(DRAFT_ORDINARY, 16);
    assert_eq!(DRAFT_EXACT, 24);
    assert_eq!(DRAFT_FRAGMENTS, 6);
    assert_eq!(DRAFT_HANDOFFS, 1);
    assert_eq!(DRAFT_REQUIRED, 13);
}

// Planner scalar constants and closed registries.
#[test]
fn planner_consts_match_python_pass() {
    let c = &vectors()["consts"];
    assert_eq!(SCHEMA_VERSION, c["schema_version"].as_i64().unwrap());
    assert_eq!(RECORD_KIND, c["record_kind"].as_str().unwrap());
    assert_eq!(STATUS, c["status"].as_str().unwrap());
    assert_eq!(
        PLAN_ARTIFACT_ROOT,
        c["plan_artifact_root"].as_str().unwrap()
    );
    assert_eq!(REPOSITORY_NAME, c["repository_name"].as_str().unwrap());
    assert_eq!(
        [
            DECISION_READY,
            DECISION_MISSING,
            DECISION_PREREQUISITE,
            DECISION_CONFLICT,
            DECISION_INVALID
        ],
        [
            c["decisions"][0].as_str().unwrap(),
            c["decisions"][1].as_str().unwrap(),
            c["decisions"][2].as_str().unwrap(),
            c["decisions"][3].as_str().unwrap(),
            c["decisions"][4].as_str().unwrap(),
        ]
    );
    assert_eq!(CLOSED_REASON_CODES.len(), 32);
    assert_eq!(
        CLOSED_REASON_CODES.to_vec(),
        str_vec(&c["closed_reason_codes"])
    );
    assert_eq!(INVALID_REASONS.to_vec(), str_vec(&c["invalid_reasons"]));
    assert_eq!(CONFLICT_REASONS.to_vec(), str_vec(&c["conflict_reasons"]));
    assert_eq!(
        PREREQUISITE_REASONS.to_vec(),
        str_vec(&c["prerequisite_reasons"])
    );
    assert_eq!(CONTROL_ROOTS.to_vec(), str_vec(&c["control_roots"]));
    assert_eq!(
        CURRENT_PACKAGE_RECORD_ROOTS.to_vec(),
        str_vec(&c["current_package_record_roots"])
    );
    assert_eq!(
        ROOT_METADATA_NAMES.to_vec(),
        str_vec(&c["root_metadata_names"])
    );
    assert_eq!(TICKET_ALLOWED.to_vec(), str_vec(&c["ticket_allowed"]));
    assert_eq!(CONTEXT_ALLOWED.to_vec(), str_vec(&c["context_allowed"]));
    let sections = c["ticket_sections"].as_object().unwrap();
    assert_eq!(sections.len(), 6);
    let section_cases: [(&str, Vec<&str>); 8] = [
        (
            "unresolved_identity",
            TICKET_UNRESOLVED_IDENTITY_FIELDS.to_vec(),
        ),
        ("repository_fence", TICKET_REPOSITORY_FENCE_FIELDS.to_vec()),
        ("context", TICKET_CONTEXT_FIELDS.to_vec()),
        ("dependencies", TICKET_DEPENDENCIES_FIELDS.to_vec()),
        ("limits", TICKET_LIMITS_FIELDS.to_vec()),
        ("deliverables", TICKET_DELIVERABLES_FIELDS.to_vec()),
        ("canonicalization", CONTEXT_CANONICALIZATION_FIELDS.to_vec()),
        ("content", CONTEXT_CONTENT_FIELDS.to_vec()),
    ];
    for (key, got) in &section_cases {
        let source = if *key == "canonicalization" || *key == "content" {
            &c["context_sections"]
        } else {
            &c["ticket_sections"]
        };
        assert_eq!(got, &str_vec(&source[key]), "section {key}");
    }
    assert_eq!(SPLIT_REVIEW_TOTAL_LINES, 8500);
    assert_eq!(HARD_TOTAL_LINES, 10_000);
    assert_eq!(PLANNER_FILE_BYTE_CEILING, 4 * 1024 * 1024);
    assert_eq!(CONTEXT_TOTAL_BYTE_CEILING, 16 * 1024 * 1024);
    assert_eq!(PLAN_BYTE_CEILING, 262_144);
    assert!(context_total_bytes_ok(16 * 1024 * 1024));
    assert!(!context_total_bytes_ok(16 * 1024 * 1024 + 1));
    assert_eq!(
        hex_encode(DOMAIN_SEPARATOR),
        c["domain_separator_hex"].as_str().unwrap()
    );
}

// safe_path grammar byte-exact over 24 cases.
#[test]
fn safe_path_matrix_pass() {
    let v = vectors();
    let cases = v["safe_path"].as_object().unwrap();
    assert_eq!(cases.len(), 27);
    for (input, want) in cases {
        assert_eq!(
            safe_path(input),
            want.as_bool().unwrap(),
            "safe_path({input:?})"
        );
    }
    assert!(safe_path("swarm/crates.toml"));
    assert!(!safe_path("a/../b"));
}

// under prefix predicate.
#[test]
fn under_matrix_pass() {
    let v = vectors();
    for row in v["under"].as_array().unwrap() {
        assert_eq!(
            under(row[0].as_str().unwrap(), row[1].as_str().unwrap()),
            row[2].as_bool().unwrap()
        );
    }
}

// exact_sha256 hex vectors.
#[test]
fn sha256_vectors_pass() {
    let v = vectors();
    for (input, want) in v["sha256"].as_object().unwrap() {
        assert_eq!(exact_sha256_hex(input.as_bytes()), want.as_str().unwrap());
    }
    assert_eq!(
        exact_sha256_hex(b"abc"),
        xtask::coverage_graph::digest_text("abc")
    );
}

// canonical JSON byte-exact (incl. lowercase \u00xx, raw DEL/unicode).
#[test]
fn canonical_vectors_pass() {
    let v = vectors();
    for (pretty, want) in v["canonical"].as_object().unwrap() {
        let payload: Value = serde_json::from_str(pretty).unwrap();
        assert_eq!(
            hex_encode(&canonical_json_bytes(&payload)),
            want.as_str().unwrap()
        );
    }
}

// plan digest: canonical bytes plus domain-separated SHA-256.
#[test]
fn plan_digest_vector_pass() {
    let v = &vectors()["plan_digest"];
    let payload: Value = serde_json::from_value(v["payload"].clone()).unwrap();
    assert_eq!(
        hex_encode(&canonical_json_bytes(&payload)),
        v["canonical_hex"].as_str().unwrap()
    );
    assert_eq!(plan_digest(&payload), v["digest"].as_str().unwrap());
}

// choose_decision precedence matrix.
#[test]
fn decisions_matrix_pass() {
    let v = vectors();
    for row in v["decisions_matrix"].as_array().unwrap() {
        let reasons: Vec<&str> = row["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(
            choose_decision(row["state"].as_str().unwrap(), &reasons),
            row["decision"].as_str().unwrap(),
            "state={} reasons={:?}",
            row["state"],
            reasons
        );
    }
}

// selection_state classification plus reasons.
#[test]
fn selection_matrix_pass() {
    let v = vectors();
    for row in v["selection"].as_array().unwrap() {
        let (state, reasons) = selection_state(
            opt_str(&row["base"]),
            opt_str(&row["writer"]),
            opt_str(&row["reviewer"]),
        );
        assert_eq!(state, row["state"].as_str().unwrap());
        assert_eq!(reasons, str_vec(&row["reasons"]));
    }
}

// Actor/package/commit/digest/opaque grammars.
#[test]
fn grammar_matrix_pass() {
    let v = &vectors()["grammar"];
    for (input, want) in v["package"].as_object().unwrap() {
        assert_eq!(
            package_name_valid(input),
            want.as_bool().unwrap(),
            "package {input:?}"
        );
    }
    for (input, want) in v["tagged"].as_object().unwrap() {
        assert_eq!(
            tagged_git_valid(input),
            want.as_bool().unwrap(),
            "tagged {input:?}"
        );
    }
    for (input, want) in v["sha256"].as_object().unwrap() {
        assert_eq!(
            sha256_hex_valid(input),
            want.as_bool().unwrap(),
            "sha256 {input:?}"
        );
    }
    for (input, want) in v["opaque"].as_object().unwrap() {
        assert_eq!(
            opaque_id_valid(input),
            want.as_bool().unwrap(),
            "opaque {input:?}"
        );
    }
    for (input, want) in v["actor"].as_object().unwrap() {
        assert_eq!(
            actor_identity_valid(input),
            want.as_bool().unwrap(),
            "actor {input:?}"
        );
    }
    // Negative shapes: uppercase hex rejected, trailing-dash package rejected.
    assert!(!sha256_hex_valid(&"A".repeat(64)));
    assert!(!package_name_valid("search-"));
    assert!(!actor_identity_valid("actor:user:alice:extra"));
}

// signed_payload_digest: single marker only.
#[test]
fn signed_payload_matrix_pass() {
    let v = vectors();
    for row in v["signed_payload"].as_array().unwrap() {
        let raw = hex_decode(row["raw_hex"].as_str().unwrap());
        assert_eq!(
            signed_payload_digest(&raw).as_deref(),
            row["digest"].as_str(),
            "raw={:?}",
            row["raw_hex"]
        );
    }
}

// Advisory output path rules (symlink checks stay Python-owned).
#[test]
fn output_path_matrix_pass() {
    let v = vectors();
    for row in v["output_paths"].as_array().unwrap() {
        let input = row["input"].as_str().unwrap();
        if input == "-" {
            assert!(advisory_output_selectable(input));
            assert!(row["reasons"].as_array().unwrap().is_empty());
            continue;
        }
        assert!(!advisory_output_selectable(input));
        assert_eq!(
            advisory_output_path_valid(input),
            row["ok"].as_bool().unwrap()
        );
        if row["ok"].as_bool().unwrap() {
            assert_eq!(input.replace('\\', "/"), row["target"].as_str().unwrap());
            assert!(row["reasons"].as_array().unwrap().is_empty());
        } else {
            assert_eq!(
                str_vec(&row["reasons"]),
                ["OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT"]
            );
        }
    }
}

// Context-source fence predicate.
#[test]
fn forbidden_matrix_pass() {
    let v = vectors();
    for (input, want) in v["forbidden"].as_object().unwrap() {
        assert_eq!(
            context_source_forbidden(input),
            want.as_bool().unwrap(),
            "forbidden({input:?})"
        );
    }
    assert!(context_source_forbidden("swarm/tickets/a.toml"));
    assert!(!context_source_forbidden("swarm/crates.toml"));
}

// Manifest-owned ceiling selection.
#[test]
fn ceilings_select_matrix_pass() {
    let v = vectors();
    for row in v["ceilings_select"].as_array().unwrap() {
        let exceptions: Vec<&str> = row["exceptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        let (ceiling, class_ok) = select_ceiling(
            row["class"].as_str().unwrap(),
            row["package"].as_str().unwrap(),
            &exceptions,
            row["ordinary"].as_i64().unwrap(),
            row["exact"].as_i64().unwrap(),
        );
        assert_eq!(ceiling, row["ceiling"].as_i64().unwrap());
        assert_eq!(class_ok, row["class_ok"].as_bool().unwrap());
    }
}

// unknown_fields sorted deduplicated difference.
#[test]
fn unknown_fields_matrix_pass() {
    let v = vectors();
    for row in v["unknown_fields"].as_array().unwrap() {
        let keys: Vec<&str> = row["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        let allowed: Vec<&str> = row["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(unknown_fields(&keys, &allowed), str_vec(&row["unknown"]));
    }
    assert_eq!(unknown_fields(&["b", "a", "b"], &["a"]), ["b"]);
}

// Handoff topology per package.
#[test]
fn handoff_topology_pass() {
    let v = &vectors()["handoff_topology"];
    for package in ["search-contracts", "search-domain", "search-ports"] {
        assert_eq!(
            expected_handoff_slots(package).to_vec(),
            str_vec(&v[package]["slots"])
        );
        assert_eq!(
            expected_required_handoffs(package).to_vec(),
            str_vec(&v[package]["required"])
        );
    }
}

// Line-limit coherence.
#[test]
fn line_limits_matrix_pass() {
    let v = vectors();
    for row in v["line_limits"].as_array().unwrap() {
        let inputs = row["in"].as_array().unwrap();
        assert_eq!(
            line_limits_ok(
                inputs[0].as_i64().unwrap(),
                inputs[1].as_i64().unwrap(),
                inputs[2].as_i64().unwrap(),
                inputs[3].as_i64().unwrap(),
            ),
            row["ok"].as_bool().unwrap()
        );
    }
}

// Exact-pack source construction (drives the real Python function via stub view).
#[test]
fn contract_pack_matrix_pass() {
    let v = vectors();
    for row in v["contract_pack"].as_array().unwrap() {
        let required: Vec<&str> = row["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        match contract_pack_sources(row["package"].as_str().unwrap(), &required) {
            Ok(sources) => assert_eq!(sources, str_vec(&row["sources"])),
            Err(reason) => assert_eq!(reason, row["reason"].as_str().unwrap()),
        }
    }
}

// one_table exact-once semantics.
#[test]
fn one_table_matrix_pass() {
    let v = vectors();
    for row in v["one_table"].as_array().unwrap() {
        let found = one_table(
            &row["rows"],
            row["key"].as_str().unwrap(),
            row["expected"].as_str().unwrap(),
        );
        assert_eq!(found.is_some(), row["found"].as_bool().unwrap());
        if let Some(hit) = found {
            assert_eq!(hit, &row["row"]);
        }
    }
    assert!(one_table(&json!([{"name": "a"}]), "name", "a").is_some());
    assert!(one_table(&json!([{"name": "a"}, {"name": "a"}]), "name", "a").is_none());
}

// resolve_selector over canned registry documents (stub GitView equivalent).
#[test]
fn selectors_matrix_pass() {
    let docs_ok = json!({
        "crates": {"package": [{"name": "search-contracts"}, {"name": "search-domain"}]},
        "functions": {"foundation": [{"package": "search-contracts"}]},
        "stages": {"stage": [{"id": "W0", "packages": ["search-contracts", "search-domain"]}]},
        "launch": {"authorized_packages": ["search-contracts"],
                   "conditional_packages": ["search-domain"],
                   "conditional_activation": {"search-domain": {"needs": ["x"]}}},
    });
    let docs_dup = json!({
        "crates": {"package": [{"name": "search-contracts"}, {"name": "search-contracts"}]},
        "functions": {"foundation": [{"package": "search-contracts"}]},
        "stages": {"stage": [{"id": "W0", "packages": ["search-contracts", "search-domain"]}]},
        "launch": {"authorized_packages": ["search-contracts"],
                   "conditional_packages": ["search-domain"],
                   "conditional_activation": {"search-domain": {"needs": ["x"]}}},
    });
    let docs_missing = json!({});
    let v = vectors();
    for row in v["selectors"].as_array().unwrap() {
        let docs = match row["view"].as_str().unwrap() {
            "ok" => &docs_ok,
            "dup" => &docs_dup,
            _ => &docs_missing,
        };
        let view = SelectorDocs {
            crates: docs.get("crates"),
            functions: docs.get("functions"),
            stages: docs.get("stages"),
            launch: docs.get("launch"),
        };
        let (status, detail) = resolve_selector(
            &view,
            row["selector"].as_str().unwrap(),
            row["package"].as_str().unwrap(),
        );
        assert_eq!(
            status.as_str(),
            row["status"].as_str().unwrap(),
            "{}",
            row["selector"]
        );
        assert_eq!(detail, row["detail"].as_str().unwrap());
    }
    // Status tokens are closed.
    assert_eq!(SelectorStatus::Ok.as_str(), "OK");
    assert_eq!(SelectorStatus::Unsupported.as_str(), "UNSUPPORTED");
    assert_eq!(SelectorStatus::NotUnique.as_str(), "NOT_UNIQUE");
}
