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
fn context_materialization_helpers_stay_split_by_responsibility() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/context_materialization.rs");
    assert!(facade.len() < 3_000, "facade grew to {} bytes", facade.len());
    for module in ["digest", "error", "output", "references", "scalars", "spec"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("Sha256::new"));
    assert!(!facade.contains("fn rfc3339_valid"));
    assert!(!facade.contains("struct ArtifactRef"));

    let digest = read(&root, "xtask/src/context_materialization/digest.rs");
    assert!(digest.contains("PLAN_DOMAIN"));
    assert!(digest.contains("OPERATION_DOMAIN"));
    assert!(!digest.contains("rfc3339"));

    let scalar_facade = read(&root, "xtask/src/context_materialization/scalars.rs");
    assert!(scalar_facade.contains("mod format;"));
    assert!(scalar_facade.contains("mod require;"));

    let references = read(&root, "xtask/src/context_materialization/references.rs");
    assert!(references.contains("mod model;"));
    assert!(references.contains("mod validation;"));

    let reference_validation = read(
        &root,
        "xtask/src/context_materialization/references/validation.rs",
    );
    assert!(reference_validation.contains("validate_artifact_ref"));
    assert!(reference_validation.contains("validate_optional_signature"));
    assert!(!reference_validation.contains("std::fs"));
    assert!(!reference_validation.contains("std::process"));
}
