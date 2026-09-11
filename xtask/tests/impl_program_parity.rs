//! Parity tests for the T41 implementation-program slice (`xtask` vs Python).
//!
//! The live-repo report is pinned against the `CPython` 3.12 report captured
//! in `fixtures/tooling/impl-program/vectors.json` before the Python
//! entrypoint was retired. Capture and live check agree because the workflow
//! still invokes the validator on both sides: pre-edit Python saw its own
//! `validate-implementation-program.py` token, post-edit Rust sees the
//! `validate implementation-program` token, and both trees are otherwise
//! identical (both reports carry the same pre-existing `Cargo.lock`
//! presence error). Negative overlays rebuild a green tree under a temp dir
//! and mutate one load-bearing field each.

use std::path::{Path, PathBuf};

use serde_json::Value;
use xtask::impl_program::{
    EXPECTED_BASELINE_REQUIRES, EXPECTED_GATE_IDS, EXPECTED_INTEGRATION_ORDER, EXPECTED_NEXT_ORDER,
    EXPECTED_PATHS, EXPECTED_STAGE_IDS, EXPECTED_TARGETS, WORKFLOW_XTASK_TOKEN, exit_code,
    render_report_json, validate_implementation_program,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/impl-program/vectors.json");
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
    parsed
}

/// Registry files copied into every temp overlay (same relative paths).
const OVERLAY_FILES: [&str; 9] = [
    "swarm/implementation-program.toml",
    "swarm/launch-state.toml",
    "swarm/stages.toml",
    "swarm/gates.toml",
    "swarm/crates.toml",
    "qualification/product-pulse/metrics.toml",
    "swarm/coverage/manifest.toml",
    "qualification/implementation-program/cases-v1.toml",
    ".github/workflows/implementation-program.yml",
];

/// `expected_paths` entries not covered by `OVERLAY_FILES`; only existence
/// is checked, so empty placeholders suffice.
const PLACEHOLDER_PATHS: [&str; 5] = [
    "swarm/function-packets.toml",
    "swarm/module-packets.toml",
    "swarm/coverage/package-map-index.toml",
    "swarm/stage-readsets.toml",
    "config/sections.toml",
];

fn overlay_root(name: &str) -> PathBuf {
    let root = repo_root();
    let dir =
        std::env::temp_dir().join(format!("xtask-impl-program-{}-{name}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clean temp overlay");
    }
    for relative in OVERLAY_FILES {
        let target = dir.join(relative);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::copy(root.join(relative), &target).expect("copy registry file");
    }
    for relative in PLACEHOLDER_PATHS {
        let target = dir.join(relative);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::write(&target, "").expect("placeholder registry file");
    }
    dir
}

fn mutate(root: &Path, relative: &str, from: &str, to: &str) {
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path).expect("read overlay file");
    assert!(text.contains(from), "overlay pattern present: {from}");
    std::fs::write(&path, text.replacen(from, to, 1)).expect("write overlay file");
}

fn drop_overlay(root: &Path) {
    std::fs::remove_dir_all(root).ok();
}

// Live-repo report matches the captured Python report field-for-field.
#[test]
fn live_report_matches_python() {
    let current = &vectors()["program_current"];
    let report = validate_implementation_program(&repo_root());
    let as_count = |value: &Value| usize::try_from(value.as_u64().expect("count")).expect("usize");
    assert_eq!(exit_code(&report), 1);
    assert!(report.complete);
    assert!(!report.passed);
    assert_eq!(report.stages, as_count(&current["stages"]));
    assert_eq!(report.packages, as_count(&current["packages"]));
    assert_eq!(report.targets, as_count(&current["targets"]));
    assert_eq!(
        report.integration_steps,
        as_count(&current["integration_steps"])
    );
    assert_eq!(report.next_steps, as_count(&current["next_steps"]));
    assert_eq!(
        report.baseline_requirements,
        as_count(&current["baseline_release_requirements"])
    );
    assert_eq!(
        report.current_stage.as_deref(),
        current["current_stage"].as_str()
    );
    assert_eq!(report.current_wave, current["current_wave"].as_i64());
    assert_eq!(
        report.cargo_lock_present,
        current["cargo_lock_present"].as_bool().expect("bool")
    );
    assert_eq!(report.errors, str_vec(&current["errors"]));
    let parsed = assert_sorted_keys(&render_report_json(&report));
    assert_eq!(parsed["status"], current["status"]);
    assert_eq!(parsed["warnings"], Value::Array(Vec::new()));
}

// A faithful overlay tree with no mutations passes with exact counters.
#[test]
fn overlay_all_green_passes() {
    let dir = overlay_root("green");
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 0, "errors: {:?}", report.errors);
    assert!(report.complete && report.passed);
    assert_eq!(report.stages, 11);
    assert_eq!(report.packages, 45);
    assert_eq!(report.targets, 6);
    assert_eq!(report.integration_steps, 5);
    assert_eq!(report.next_steps, 7);
    assert_eq!(report.baseline_requirements, 7);
    assert_eq!(report.current_stage.as_deref(), Some("P00"));
    assert_eq!(report.current_wave, Some(0));
    assert!(!report.cargo_lock_present);
    assert!(report.errors.is_empty());
    let parsed = assert_sorted_keys(&render_report_json(&report));
    assert_eq!(parsed["status"], Value::String("PASS".to_owned()));
    drop_overlay(&dir);
}

// Zero-fixture root fails with the minimal early-failure object.
#[test]
fn validator_fails_without_root() {
    let missing = repo_root().join("target/__xtask_definitely_missing_root__");
    let report = validate_implementation_program(&missing);
    assert_eq!(exit_code(&report), 1);
    assert!(!report.complete);
    assert!(!report.passed);
    assert!(!report.errors.is_empty());
    let parsed: Value = serde_json::from_str(&render_report_json(&report)).expect("renders JSON");
    let keys: Vec<&str> = parsed
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["errors", "status"]);
    assert_eq!(parsed["status"], Value::String("FAIL".to_owned()));
}

// Claimed authority is rejected.
#[test]
fn rejects_non_authoritative_status() {
    let dir = overlay_root("status");
    mutate(
        &dir,
        "swarm/implementation-program.toml",
        "status = \"PLANNED_NOT_AUTHORIZED\"",
        "status = \"AUTHORIZED\"",
    );
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "implementation program status is not non-authoritative"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A non-SHA source commit is rejected.
#[test]
fn rejects_bad_source_commit() {
    let dir = overlay_root("commit");
    mutate(
        &dir,
        "swarm/implementation-program.toml",
        "source_main_commit = \"e5ab8d0be8522bc3e0883795aa905c3194153e32\"",
        "source_main_commit = \"not-a-sha\"",
    );
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "source main commit is not an exact SHA"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A duplicated stage identity fails the load phase like the Python `index_rows`.
#[test]
fn rejects_duplicate_stage_identity() {
    let dir = overlay_root("duplicate");
    let path = dir.join("swarm/implementation-program.toml");
    let mut text = std::fs::read_to_string(&path).expect("read program");
    text.push_str("\n[[stage]]\nid = \"W0\"\n");
    std::fs::write(&path, text).expect("write program");
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(!report.complete);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("duplicate stage identity: W0")),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A package outside the registry is rejected.
#[test]
fn rejects_unknown_package() {
    let dir = overlay_root("package");
    mutate(
        &dir,
        "swarm/implementation-program.toml",
        "packages = [\"search-contracts\", \"search-domain\", \"search-ports\"]",
        "packages = [\"search-contracts\", \"search-domain\", \"search-ports\", \"bogus-package\"]",
    );
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "W0: unknown package bogus-package"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// A workflow that no longer declares manual dispatch is rejected.
#[test]
fn rejects_missing_workflow_token() {
    let dir = overlay_root("token");
    mutate(
        &dir,
        ".github/workflows/implementation-program.yml",
        "workflow_dispatch:",
        "workflow_disabled:",
    );
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "implementation workflow missing token: workflow_dispatch:"),
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
        ".github/workflows/implementation-program.yml",
        "workflow_dispatch:",
        "workflow_dispatch:\n  push:",
    );
    let report = validate_implementation_program(&dir);
    assert_eq!(exit_code(&report), 1);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error == "automatic workflow trigger present: push:"),
        "errors: {:?}",
        report.errors
    );
    drop_overlay(&dir);
}

// Expected tables mirror the Python literals.
#[test]
fn constants_match_python() {
    assert_eq!(
        EXPECTED_STAGE_IDS,
        [
            "W0", "W1", "W2", "W3", "W4", "W5", "W6", "W7", "W8", "W9", "W10"
        ]
    );
    assert_eq!(
        EXPECTED_GATE_IDS,
        ["G0", "G1", "G2", "G3", "G4", "G5", "G6"]
    );
    assert_eq!(EXPECTED_PATHS.len(), 9);
    assert_eq!(
        EXPECTED_PATHS[0],
        ("launch_authority", "swarm/launch-state.toml")
    );
    assert_eq!(
        EXPECTED_PATHS[8],
        ("configuration_registry", "config/sections.toml")
    );
    assert_eq!(
        EXPECTED_TARGETS,
        [
            ("buildable_workspace", "W0"),
            ("bootable_service_shell", "W1"),
            ("direct_source_product", "W2"),
            ("useful_baseline_search", "W4"),
            ("release_candidate", "W9"),
            ("optional_depth", "W10"),
        ]
    );
    assert_eq!(
        EXPECTED_INTEGRATION_ORDER,
        [
            "pin_windows_toolchain",
            "lock_dependency_graph",
            "freeze_build_profiles",
            "establish_test_harness",
            "freeze_artifact_and_data_layout",
        ]
    );
    assert_eq!(
        EXPECTED_NEXT_ORDER,
        [
            "integration_bootstrap_pr",
            "search_contracts_implementation",
            "search_contracts_review_and_handoff",
            "search_domain_implementation",
            "search_ports_implementation",
            "w0_g0_evidence_and_acceptance",
            "advance_launch_to_w1",
        ]
    );
    assert_eq!(
        EXPECTED_BASELINE_REQUIRES,
        ["G0", "G1", "G2", "G3", "W7_LIFECYCLE", "G4", "G5"]
    );
    assert_eq!(WORKFLOW_XTASK_TOKEN, "validate implementation-program");
}
