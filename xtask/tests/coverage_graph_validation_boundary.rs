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
fn coverage_graph_validation_is_rust_owned_and_bounded() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/coverage_graph_validation.rs");
    for module in ["content", "load", "policy", "relations"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(facade.contains("validate_coverage_graph"));
    assert!(facade.contains("validate_package_maps"));
    assert!(!facade.contains("std::process"));
    assert!(!facade.contains("Command::new"));

    let command = read(&root, "xtask/src/command.rs");
    assert!(command.contains("validate coverage-graph [--json]"));
    assert!(command.contains("structural::validate_coverage_graph()"));
    assert!(command.contains("generate coverage-graph [--check] [--json]"));

    let wrapper = read(&root, "tools/validate-coverage-graph-v2.ps1");
    assert!(wrapper.contains("cargo"));
    assert!(wrapper.contains("--locked"));
    assert!(wrapper.contains("coverage-graph"));
    assert!(!wrapper.to_ascii_lowercase().contains("python"));

    assert!(!root.join("tools/validate-coverage-graph-v2.py").exists());
    assert!(!root.join("tools/generate-coverage-graph-v2.py").exists());
    assert!(!root.join("tools/coverage_graph_v2.py").exists());

    let workflow = read(
        &root,
        ".github/workflows/package-map-coverage-v2.yml",
    );
    assert!(workflow.contains("generate coverage-graph --check --json"));
    assert!(workflow.contains("validate coverage-graph --json"));
    assert!(!workflow.to_ascii_lowercase().contains("python"));
}
