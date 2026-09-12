//! Structural coverage for the Rust context-artifact validator.

use std::path::{Path, PathBuf};

use xtask::context_artifact_validation::{
    render_report_json, validate_context_artifact_candidate,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member")
        .to_owned()
}

#[test]
fn current_repository_closure_is_structurally_valid() {
    let report = validate_context_artifact_candidate(&repository_root());
    assert!(report.passed(), "{:?}", report.errors);
    assert!(report.checks.len() >= 60, "bounded closure must stay broad");
}

#[test]
fn report_preserves_zero_authority_nonclaims() {
    let report = validate_context_artifact_candidate(&repository_root());
    let json = render_report_json(&report);
    for token in [
        "\"current_candidate_id\":\"UNAVAILABLE\"",
        "\"authoritative_context_materialized\":false",
        "\"context_manifest_created\":false",
        "\"ticket_issued\":false",
        "\"writer_lease_created\":false",
        "\"implementation_authorized\":false",
        "\"package_acceptance_claimed\":false",
        "\"g0_acceptance_claimed\":false",
        "\"w0_acceptance_claimed\":false",
        "\"w1_authority_claimed\":false",
    ] {
        assert!(json.contains(token), "missing {token}: {json}");
    }
}
