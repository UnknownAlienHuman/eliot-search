use std::fs;
use std::path::Path;

#[test]
fn context_artifact_builder_entrypoint_remains_rust_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member");
    let wrapper_path = root.join("tools/build-context-artifact-candidate.ps1");
    let wrapper = fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|error| panic!("{}: {error}", wrapper_path.display()));
    let lower = wrapper.to_ascii_lowercase();
    for required in [
        "cargo",
        "--locked",
        "xtask",
        "build",
        "context-artifact-candidate",
    ] {
        assert!(
            lower.contains(required),
            "wrapper lost required Rust token: {required}"
        );
    }
    for forbidden in ["python", "py -", "node", "npx"] {
        assert!(
            !lower.contains(forbidden),
            "wrapper restored forbidden runtime: {forbidden}"
        );
    }
    for retired in [
        "tools/build-context-artifact-candidate.py",
        "tools/context_artifact_builder_v1/build.py",
        "tools/context_artifact_builder_v1/extract.py",
        "qualification/context-artifact/test_context_artifact_candidate_v1.py",
    ] {
        assert!(!root.join(retired).exists(), "retired builder returned: {retired}");
    }

    let workflow_path = root.join(".github/workflows/context-artifact-candidate.yml");
    let workflow = fs::read_to_string(&workflow_path)
        .unwrap_or_else(|error| panic!("{}: {error}", workflow_path.display()));
    assert!(!workflow.to_ascii_lowercase().contains("python"));
    assert!(workflow.contains("cargo test --locked -p xtask --test context_artifact_builder"));

    let registry = fs::read_to_string(root.join("swarm/context-artifact-builder-v1.toml"))
        .expect("builder registry is readable");
    assert!(registry.contains("implementation = \"xtask/src/context_artifact_builder.rs\""));
    assert!(!registry.contains(".py\""));
}
