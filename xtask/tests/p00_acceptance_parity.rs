//! Parity tests for the T41 p00-foundation-acceptance slice (`xtask` vs Python).
//!
//! The live-repo report is pinned against the `CPython` 3.12 report captured
//! in `fixtures/tooling/p00-acceptance/vectors.json` before the Python
//! entrypoint was retired. Capture and live check agree because the slice
//! only rewires the call sites: the `.ps1` wrapper keeps its name (so the
//! `tool-navigation` check still resolves), the workflow keeps its
//! manual-only/read-only/credential-free shape (only the `py_compile` step
//! became `cargo build`/`cargo test --no-run`), and no registry, draft,
//! matrix or control root changed. Both reports carry the same pre-existing
//! `orchestration-version` error (live `swarm/orchestration.toml` is schema
//! v6 while the acceptance boundary requires v5). Negative overlays rebuild
//! a green tree under a temp dir (overlay copies plus `schema_version = 5`)
//! and mutate one load-bearing field each.

use std::path::{Path, PathBuf};

use serde_json::Value;
use xtask::p00_acceptance::{
    EXPECTED_CHAIN, EXPECTED_CHECKPOINTS, EXPECTED_G0, EXPECTED_MATRIX_TOKENS, EXPECTED_PACKAGES,
    EXPECTED_W1_PACKAGES, FORBIDDEN_WORKFLOW_TRIGGERS, PROTECTED_ROOTS, VALIDATOR_ID, exit_code,
    render_report_json, validate_p00_foundation_acceptance,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/p00-acceptance/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

fn str_vec(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string").to_owned())
        .collect()
}

fn assert_sorted_keys(rendered: &str) -> Value {
    let parsed: Value = serde_json::from_str(rendered).expect("report renders JSON");
    let keys: Vec<&str> = parsed
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "JSON keys are sorted");
    for check in parsed["checks"].as_array().expect("checks array") {
        let check_keys: Vec<&str> = check
            .as_object()
            .expect("check object")
            .keys()
            .map(String::as_str)
            .collect();
        let mut sorted = check_keys.clone();
        sorted.sort_unstable();
        assert_eq!(check_keys, sorted, "check keys are sorted");
    }
    parsed
}

/// Registry files copied into every temp overlay (same relative paths).
const OVERLAY_FILES: [&str; 18] = [
    "swarm/p00-foundation-acceptance.toml",
    "swarm/gates.toml",
    "swarm/stages.toml",
    "swarm/launch-state.toml",
    "swarm/crates.toml",
    "swarm/function-packets.toml",
    "swarm/ticket-drafts/manifest.toml",
    "swarm/context-drafts/manifest.toml",
    "swarm/orchestration.toml",
    "swarm/ticket-drafts/p00/search-contracts.toml",
    "swarm/ticket-drafts/p00/search-domain.toml",
    "swarm/ticket-drafts/p00/search-ports.toml",
    "swarm/context-drafts/p00/search-contracts.toml",
    "swarm/context-drafts/p00/search-domain.toml",
    "swarm/context-drafts/p00/search-ports.toml",
    "docs/handoff/P00_FOUNDATION_ACCEPTANCE_MATRIX.md",
    "docs/handoff/README.md",
    "tools/README.md",
];

/// Minimal manual-only/read-only/credential-free workflow for overlays.
const OVERLAY_WORKFLOW: &str = "name: overlay\non:\n  workflow_dispatch:\npermissions:\n  contents: read\njobs:\n  qualify:\n    runs-on: windows-latest\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          persist-credentials: false\n";

/// Workflow files in the live tree (each contributes one closure check).
fn live_workflow_count() -> usize {
    let dir = repo_root().join(".github/workflows");
    std::fs::read_dir(&dir)
        .expect("workflow dir exists")
        .flatten()
        .filter(|entry| {
            entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| matches!(ext, "yml" | "yaml"))
        })
        .count()
}

fn overlay_root(name: &str) -> PathBuf {
    let root = repo_root();
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("xtask-p00-acceptance-{pid}-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clean temp overlay");
    }
    for relative in OVERLAY_FILES {
        let target = dir.join(relative);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::copy(root.join(relative), &target).expect("copy registry file");
    }
    let workflow = dir.join(".github/workflows/overlay.yml");
    std::fs::create_dir_all(workflow.parent().expect("parent")).expect("mkdir");
    std::fs::write(&workflow, OVERLAY_WORKFLOW).expect("write overlay workflow");
    for relative in PROTECTED_ROOTS {
        std::fs::create_dir_all(dir.join(relative)).expect("mkdir protected root");
    }
    // The live tree pins schema v6 against a v5 boundary; overlays start green.
    // (Single-line anchor: registry checkouts use CRLF.)
    mutate(
        &dir,
        "swarm/orchestration.toml",
        "schema_version = 6",
        "schema_version = 5",
    );
    dir
}

fn mutate(root: &Path, relative: &str, from: &str, to: &str) {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).expect("read overlay file");
    assert!(text.contains(from), "overlay pattern present: {from}");
    std::fs::write(&path, text.replacen(from, to, 1)).expect("write overlay file");
}

fn mutate_all(root: &Path, relative: &str, from: &str, to: &str) {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).expect("read overlay file");
    assert!(text.contains(from), "overlay pattern present: {from}");
    std::fs::write(&path, text.replace(from, to)).expect("write overlay file");
}

fn drop_overlay(root: &Path) {
    std::fs::remove_dir_all(root).ok();
}

// Live-repo report matches the captured Python report field-for-field.
#[test]
fn live_report_matches_python() {
    let current = &vectors()["p00_acceptance_current"];
    let report = validate_p00_foundation_acceptance(&repo_root());
    assert_eq!(exit_code(&report), 1);
    assert!(!report.passed);
    assert_eq!(
        report.checks.len(),
        current["checks"].as_array().expect("array").len()
    );
    assert_eq!(report.checks.len(), 276);
    let expected_checks: Vec<(String, String, String)> = current["checks"]
        .as_array()
        .expect("array")
        .iter()
        .map(|check| {
            (
                check["id"].as_str().expect("string").to_owned(),
                check["status"].as_str().expect("string").to_owned(),
                check["detail"].as_str().expect("string").to_owned(),
            )
        })
        .collect();
    let actual_checks: Vec<(String, String, String)> = report
        .checks
        .iter()
        .map(|check| {
            (
                check.id.clone(),
                if check.passed {
                    "PASS".to_owned()
                } else {
                    "FAIL".to_owned()
                },
                check.detail.clone(),
            )
        })
        .collect();
    assert_eq!(actual_checks, expected_checks);
    assert_eq!(report.errors, str_vec(&current["errors"]));
    assert_eq!(
        report.errors,
        vec!["orchestration-version: orchestration schema v5".to_owned()]
    );
    let parsed = assert_sorted_keys(&render_report_json(&report));
    assert_eq!(parsed["status"], current["status"]);
    assert_eq!(parsed["status"], Value::String("FAIL".to_owned()));
    assert_eq!(parsed["validator"], Value::String(VALIDATOR_ID.to_owned()));
    assert_eq!(parsed["schema_version"], Value::from(1));
    for key in [
        "package_acceptance_claimed",
        "g0_acceptance_claimed",
        "w0_acceptance_claimed",
        "w1_authority_claimed",
    ] {
        assert_eq!(parsed[key], Value::Bool(false), "{key} claims nothing");
    }
    assert_eq!(parsed["non_authoritative"], Value::Bool(true));
}

// A faithful overlay tree with the v5 boundary passes with exact counters.
#[test]
fn overlay_all_green_passes() {
    let dir = overlay_root("green");
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 0, "errors: {:?}", report.errors);
    assert!(report.passed);
    // One overlay workflow file instead of the live tree's workflow set, so
    // the check count is lower by exactly that file-count delta.
    assert_eq!(report.checks.len(), 276 - (live_workflow_count() - 1));
    assert!(report.errors.is_empty());
    let parsed = assert_sorted_keys(&render_report_json(&report));
    assert_eq!(parsed["status"], Value::String("PASS".to_owned()));
    drop_overlay(&dir);
}

// A missing root fails closed with accumulated file errors.
#[test]
fn validator_fails_without_root() {
    let missing = repo_root().join("target/__xtask_definitely_missing_root__");
    let report = validate_p00_foundation_acceptance(&missing);
    assert_eq!(exit_code(&report), 1);
    assert!(!report.passed);
    assert!(!report.checks.is_empty());
    assert!(!report.errors.is_empty());
    assert!(
        report.errors.iter().all(|error| error.contains(':')),
        "errors: {:?}",
        report.errors
    );
    let parsed = assert_sorted_keys(&render_report_json(&report));
    assert_eq!(parsed["status"], Value::String("FAIL".to_owned()));
}

// Claimed registry authority is rejected.
#[test]
fn rejects_executed_registry_status() {
    let dir = overlay_root("status");
    mutate(
        &dir,
        "swarm/p00-foundation-acceptance.toml",
        "status = \"DESIGNED_NOT_EXECUTED\"",
        "status = \"EXECUTED\"",
    );
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "registry-status: registry remains designed, not executed"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A renamed checkpoint breaks the exact P00-A through P00-D sequence.
#[test]
fn rejects_renamed_checkpoint() {
    let dir = overlay_root("checkpoint");
    mutate(
        &dir,
        "swarm/p00-foundation-acceptance.toml",
        "id = \"P00-D\"",
        "id = \"P00-X\"",
    );
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "checkpoint-ids: exact checkpoints"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A claimable ticket draft is rejected.
#[test]
fn rejects_claimable_ticket() {
    let dir = overlay_root("ticket");
    mutate(
        &dir,
        "swarm/ticket-drafts/p00/search-contracts.toml",
        "claimable = false",
        "claimable = true",
    );
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "ticket:search-contracts:claimable: not claimable"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// An issued record under a protected root breaks zero-state.
#[test]
fn rejects_issued_record() {
    let dir = overlay_root("zero");
    std::fs::write(dir.join("swarm/tickets/stray.toml"), "stray = true\n")
        .expect("write stray record");
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "zero:swarm/tickets: unexpected: swarm/tickets/stray.toml"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A matrix that loses a navigation token is rejected.
#[test]
fn rejects_missing_matrix_token() {
    let dir = overlay_root("matrix");
    mutate_all(
        &dir,
        "docs/handoff/P00_FOUNDATION_ACCEPTANCE_MATRIX.md",
        "UNAVAILABLE",
        "TAKEN",
    );
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "matrix-token:UNAVAILABLE: matrix contains UNAVAILABLE"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// An automatic trigger poisons the manual-only closure.
#[test]
fn rejects_automatic_trigger() {
    let dir = overlay_root("trigger");
    mutate(
        &dir,
        ".github/workflows/overlay.yml",
        "  workflow_dispatch:",
        "  workflow_dispatch:\n  push:",
    );
    let report = validate_p00_foundation_acceptance(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report.errors.iter().any(|error| error
            == "workflow:.github/workflows/overlay.yml: manual/read-only/credential-free"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// Expected tables mirror the Python literals.
#[test]
fn constants_match_python() {
    assert_eq!(
        EXPECTED_PACKAGES,
        ["search-contracts", "search-domain", "search-ports"]
    );
    assert_eq!(
        EXPECTED_W1_PACKAGES,
        [
            "search-config",
            "search-runtime-owner",
            "search-os-secrets",
            "search-control-redb",
            "search-provider-protocol",
            "eliot-searchd",
            "eliot-search",
        ]
    );
    assert_eq!(
        EXPECTED_CHAIN,
        [
            "context_manifest_v1",
            "assignment_ticket_v1",
            "writer_lease_v1",
            "lease_event_v1:ACKNOWLEDGED",
            "package_submission_v1",
            "independent_review_v1:ACCEPT_SUBMISSION_FOR_INTEGRATION",
            "package_handoff_v1",
        ]
    );
    assert_eq!(
        EXPECTED_G0,
        [
            "architecture_hash_challenge",
            "workspace_registry_assignment_parity",
            "dependency_graph_acyclic",
            "dependency_direction_policy",
            "recipe_set_exact",
            "epoch_and_sentinel_contract",
            "canonical_public_schema_fixtures",
            "reason_code_registry",
            "contract_domain_tests",
            "dependency_source_and_license_policy",
        ]
    );
    assert_eq!(EXPECTED_CHECKPOINTS, ["P00-A", "P00-B", "P00-C", "P00-D"]);
    assert_eq!(
        PROTECTED_ROOTS,
        [
            "swarm/context-manifests",
            "swarm/tickets",
            "swarm/leases",
            "swarm/submissions",
            "swarm/reviews",
            "swarm/handoffs",
            "swarm/supersessions",
            "swarm/wave-receipts",
        ]
    );
    assert_eq!(FORBIDDEN_WORKFLOW_TRIGGERS.len(), 32);
    assert_eq!(FORBIDDEN_WORKFLOW_TRIGGERS[0], "push");
    assert_eq!(FORBIDDEN_WORKFLOW_TRIGGERS[31], "watch");
    assert_eq!(EXPECTED_MATRIX_TOKENS.len(), 12);
    assert_eq!(VALIDATOR_ID, "p00_foundation_acceptance_v1");
}
