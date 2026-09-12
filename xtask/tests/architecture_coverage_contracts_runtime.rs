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
fn architecture_contract_closure_is_rust_owned() {
    let root = repository_root();
    assert!(
        !root
            .join("tools/validate-architecture-coverage-contracts.py")
            .exists(),
        "retired Python contract validator returned"
    );

    let wrapper = read(&root, "tools/validate-architecture-coverage.ps1");
    assert!(wrapper.contains("validate-architecture-coverage.py"));
    assert!(wrapper.contains("cargo"));
    assert!(wrapper.contains("--locked"));
    assert!(wrapper.contains("architecture-coverage-contracts"));
    assert!(!wrapper.contains("validate-architecture-coverage-contracts.py"));

    let facade = read(&root, "xtask/src/architecture_coverage_contracts.rs");
    for module in ["operations", "qualification", "tasks"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("fn operation_names"));

    let operations = read(
        &root,
        "xtask/src/architecture_coverage_contracts/operations.rs",
    );
    assert!(operations.contains("operation_names"));
    assert!(operations.contains("package-qualified operation"));

    for workflow in std::fs::read_dir(root.join(".github/workflows"))
        .expect("workflow directory")
    {
        let path = workflow.expect("workflow entry").path();
        if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                !text.contains("validate-architecture-coverage-contracts.py"),
                "{} references the retired validator",
                path.display()
            );
        }
    }
}
