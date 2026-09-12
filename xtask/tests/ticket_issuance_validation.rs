//! Structural coverage for the Rust ticket-issuance validator.

use std::path::{Path, PathBuf};

use xtask::ticket_issuance_validation::{
    render_report_json, validate_ticket_issuance_plan,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member")
        .to_owned()
}

#[test]
fn current_repository_closure_is_structurally_valid() {
    let report = validate_ticket_issuance_plan(&repository_root());
    assert!(report.passed(), "{:?}", report.errors);
    assert!(report.checks.len() >= 70, "bounded closure must stay broad");
}

#[test]
fn report_preserves_zero_authority_nonclaims() {
    let report = validate_ticket_issuance_plan(&repository_root());
    let json = render_report_json(&report);
    for token in [
        "\"non_authoritative\":true",
        "\"package_acceptance_claimed\":false",
        "\"g0_acceptance_claimed\":false",
        "\"w0_acceptance_claimed\":false",
        "\"w1_authority_claimed\":false",
    ] {
        assert!(json.contains(token), "missing {token}: {json}");
    }
}
