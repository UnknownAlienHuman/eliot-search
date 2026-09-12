use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn ticket_planner_helpers_stay_split_by_responsibility() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/ticket_planner.rs");
    assert!(facade.len() < 5_000, "facade grew to {} bytes", facade.len());
    for module in [
        "canonical",
        "context",
        "decision",
        "grammar",
        "path",
        "selectors",
        "spec",
    ] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("Sha256::new"));
    assert!(!facade.contains("fn resolve_selector"));
    assert!(!facade.contains("pub const CLOSED_REASON_CODES"));

    let canonical = read(&root, "xtask/src/ticket_planner/canonical.rs");
    assert!(canonical.contains("canonical_json_bytes"));
    assert!(canonical.contains("plan_digest"));
    assert!(!canonical.contains("PLAN_ARTIFACT_ROOT"));

    let selectors = read(&root, "xtask/src/ticket_planner/selectors.rs");
    assert!(selectors.contains("resolve_selector"));
    assert!(!selectors.contains("std::fs"));
    assert!(!selectors.contains("std::process"));

    let path = read(&root, "xtask/src/ticket_planner/path.rs");
    assert!(path.contains("context_source_forbidden"));
    assert!(path.contains("advisory_output_path_valid"));
    assert!(!path.contains("Sha256"));

    let spec = read(&root, "xtask/src/ticket_planner/spec.rs");
    assert!(spec.contains("CLOSED_REASON_CODES"));
    assert!(spec.contains("PLAN_BYTE_CEILING"));
    assert!(!spec.contains("serde_json"));
}
