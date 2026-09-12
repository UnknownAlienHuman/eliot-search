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
fn context_materialization_validation_stays_split() {
    let root = repository_root();
    let facade = read(root.as_path(), "xtask/src/context_materialization_validation.rs");
    assert!(facade.len() < 5_000, "facade grew to {} bytes", facade.len());
    for module in ["repository", "rules", "spec"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("fn validate_workflow"));
    assert!(!facade.contains("fn validate_contracts"));
    assert!(!facade.contains("RETIRED_PYTHON"));

    let rules = read(
        root.as_path(),
        "xtask/src/context_materialization_validation/rules.rs",
    );
    assert!(rules.contains("validate_registry"));
    assert!(rules.contains("validate_contracts"));
    assert!(rules.contains("validate_cases"));
    assert!(!rules.contains("std::fs"));

    let repository = read(
        root.as_path(),
        "xtask/src/context_materialization_validation/repository.rs",
    );
    assert!(repository.contains("validate_workflow"));
    assert!(repository.contains("validate_implementation_sentinels"));
    assert!(repository.contains("validate_retired_python"));

    let spec = read(
        root.as_path(),
        "xtask/src/context_materialization_validation/spec.rs",
    );
    assert!(spec.contains("REQUIRED"));
    assert!(spec.contains("RETIRED_PYTHON"));
}
