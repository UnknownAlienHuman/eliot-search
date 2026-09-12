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
fn coverage_graph_generation_is_rust_owned_and_review_only() {
    let root = repository_root();
    assert!(!root.join("tools/generate-coverage-graph-v2.py").exists());
    assert!(!root.join("tools/coverage_graph_v2.py").exists());

    let command = read(&root, "xtask/src/command.rs");
    assert!(command.contains("generate coverage-graph [--check] [--json]"));
    assert!(command.contains("coverage_graph::generate(options)"));

    let wrapper = read(&root, "tools/generate-coverage-graph-v2.ps1");
    let lower = wrapper.to_ascii_lowercase();
    for token in ["cargo", "--locked", "generate", "coverage-graph"] {
        assert!(lower.contains(token), "wrapper missing {token}");
    }
    assert!(!lower.contains("python"));

    let facade = read(&root, "xtask/src/coverage_graph_generation.rs");
    for module in ["derive", "io", "load", "render"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(facade.contains("generate_coverage_graph"));
    assert!(!facade.contains("choose_module"));
    assert!(!facade.contains("MODULE_ALIASES"));

    let derivation = read(&root, "xtask/src/coverage_graph_generation/derive.rs");
    for module in ["documentation", "operations", "registry", "source"] {
        assert!(derivation.contains(&format!("mod {module};")));
    }
    assert!(!derivation.contains("choose_module"));
    assert!(!derivation.contains("semantic score"));

    let operations = read(
        &root,
        "xtask/src/coverage_graph_generation/derive/operations.rs",
    );
    assert!(operations.contains("source-derived operation"));
    assert!(operations.contains("pub(crate) async fn scoped_call"));
    assert!(!operations.contains("choose_module"));

    let documentation = read(
        &root,
        "xtask/src/coverage_graph_generation/derive/documentation.rs",
    );
    assert!(documentation.contains("documentation heading"));

    let registry = read(
        &root,
        "xtask/src/coverage_graph_generation/derive/registry.rs",
    );
    assert!(registry.contains("package dependency"));
    assert!(registry.contains("logical module"));

    let helper_facade = read(&root, "xtask/src/coverage_graph.rs");
    for module in ["digest", "markdown", "text"] {
        assert!(helper_facade.contains(&format!("mod {module};")));
    }

    let manifest = read(&root, "swarm/coverage/manifest.toml");
    assert!(manifest.contains("route_assignment_policy = \"reviewed_registry_only\""));
    assert!(manifest.contains("heuristic_route_generation_allowed = false"));

    for workflow in [
        ".github/workflows/package-map-coverage-v2.yml",
        ".github/workflows/implementation-program.yml",
        ".github/workflows/refactor-boundaries.yml",
    ] {
        let text = read(&root, workflow);
        assert!(text.contains("generate coverage-graph --check --json"));
        assert!(!text.to_ascii_lowercase().contains("python"));
    }
}
