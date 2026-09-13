use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use search_materializer::api::{
    AcceptedProfiles, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET,
    MaterializationBudget, MaterializationRequest, ValidatedMaterializationRequest,
    validate_materialization_request,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn request_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/request.rs"))
        .expect("request facade exists");

    for module in [
        "mod budget;",
        "mod cancellation;",
        "mod model;",
        "mod validate;",
        "mod tests;",
    ] {
        assert!(entry.contains(module), "missing request owner: {module}");
    }
    assert!(entry.lines().count() <= 32);

    for implementation_marker in [
        "pub struct MaterializationBudget",
        "pub struct CancellationToken",
        "pub struct MaterializationRequest",
        "pub fn validate_materialization_request",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to request facade: {implementation_marker}"
        );
    }
}

#[test]
fn request_responsibilities_have_distinct_private_owners() {
    let root = crate_root().join("src/request");
    let expectations = [
        ("budget.rs", "pub struct MaterializationBudget"),
        ("cancellation.rs", "pub struct CancellationToken"),
        ("model.rs", "pub struct MaterializationRequest"),
        ("validate.rs", "pub fn validate_materialization_request"),
        ("tests.rs", "fn unsaved_bytes_require_snapshot_receipt"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

#[test]
fn public_request_surface_remains_importable() {
    let _ = core::mem::size_of::<AcceptedProfiles>();
    let _ = core::mem::size_of::<CancellationToken<'static>>();
    let _ = core::mem::size_of::<MaterializationBudget>();
    let _ = core::mem::size_of::<MaterializationRequest>();
    let _ = core::mem::size_of::<ValidatedMaterializationRequest>();
    let _ = validate_materialization_request;
    assert!(DEFAULT_MATERIALIZATION_BUDGET.validate().is_ok());
}

#[test]
fn cancellation_token_still_observes_the_external_flag() {
    let flag = AtomicBool::new(false);
    let token = CancellationToken::new(&flag);
    assert!(!token.is_cancelled());
    flag.store(true, Ordering::SeqCst);
    assert!(token.is_cancelled());
    assert!(!CancellationToken::never().is_cancelled());
}
