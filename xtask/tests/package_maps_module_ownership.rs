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
fn package_map_validation_consumes_canonical_registries_without_python() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/package_maps.rs");
    assert!(facade.contains("mod helpers;"));
    assert!(facade.contains("mod load;"));
    assert!(facade.contains("mod validation;"));
    assert!(!facade.contains("std::process"));
    assert!(!facade.contains("python"));

    let validation = read(&root, "xtask/src/package_maps/validation.rs");
    for module in ["content", "files", "policy", "relations"] {
        assert!(validation.contains(&format!("mod {module};")));
    }

    let load = read(&root, "xtask/src/package_maps/load.rs");
    assert!(load.contains("module_coverage_registry"));
    assert!(load.contains("operation_module_registry"));
    assert!(load.contains("documentation_node_registry"));
    assert!(load.contains("dependency_edge_registry"));
    assert!(!load.contains("coverage_graph_v2.py"));

    let wrapper = read(&root, "tools/validate-package-maps-v2.ps1");
    let lower = wrapper.to_ascii_lowercase();
    assert!(lower.contains("cargo"));
    assert!(lower.contains("--locked"));
    assert!(lower.contains("validate"));
    assert!(lower.contains("package-maps"));
    assert!(!lower.contains("python"));
    assert!(!root.join("tools/validate-package-maps-v2.py").exists());
}
