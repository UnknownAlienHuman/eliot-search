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
fn full_architecture_coverage_is_rust_owned_and_split() {
    let root = repository_root();
    assert!(
        !root.join("tools/validate-architecture-coverage.py").exists(),
        "retired Python validator returned"
    );

    let facade = read(&root, "xtask/src/architecture_coverage.rs");
    assert!(facade.len() < 7_000, "facade grew to {} bytes", facade.len());
    for module in ["control", "load", "markdown", "schemas", "topology"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("std::fs"));
    assert!(!facade.contains("fn operation_names"));
    assert!(!facade.contains("fn exact_type_registry_symbols"));

    let topology = read(&root, "xtask/src/architecture_coverage/topology.rs");
    for module in ["packages", "ports", "relations"] {
        assert!(topology.contains(&format!("mod {module};")));
    }

    let wrapper = read(&root, "tools/validate-architecture-coverage.ps1");
    let lower = wrapper.to_ascii_lowercase();
    assert!(lower.contains("cargo"));
    assert!(lower.contains("--locked"));
    assert!(lower.contains("architecture-coverage"));
    assert!(lower.contains("architecture-coverage-contracts"));
    assert!(!lower.contains("python"));

    for workflow in std::fs::read_dir(root.join(".github/workflows"))
        .expect("workflow directory")
    {
        let path = workflow.expect("workflow entry").path();
        if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                !text.contains("validate-architecture-coverage.py"),
                "{} references the retired validator",
                path.display()
            );
        }
    }
}
