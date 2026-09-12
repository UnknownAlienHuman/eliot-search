use std::fs;
use std::path::Path;

#[test]
fn context_materialization_entrypoint_remains_rust_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member");
    let wrapper_path = root.join("tools/plan-context-materialization.ps1");
    let wrapper = fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|error| panic!("{}: {error}", wrapper_path.display()));
    let lower = wrapper.to_ascii_lowercase();
    for required in [
        "cargo",
        "--locked",
        "xtask",
        "build",
        "context-materialization-plan",
    ] {
        assert!(lower.contains(required), "wrapper lost Rust token: {required}");
    }
    for forbidden in ["python", "py -", "node", "npx"] {
        assert!(!lower.contains(forbidden), "wrapper restored runtime: {forbidden}");
    }
    for retired in [
        "tools/plan-context-materialization.py",
        "tools/context_materialization_planner_v1/__init__.py",
        "tools/context_materialization_planner_v1/core.py",
        "tools/context_materialization_planner_v1/manifest.py",
        "tools/context_materialization_planner_v1/plan.py",
        "qualification/context-materialization/test_context_materialization_plan_v1.py",
        "tools/context_artifact_builder_v1/__init__.py",
        "tools/context_artifact_builder_v1/core.py",
        "tools/context_artifact_builder_v1/bundle.py",
    ] {
        assert!(!root.join(retired).exists(), "retired Python returned: {retired}");
    }

    let workflow = fs::read_to_string(
        root.join(".github/workflows/context-materialization-plan.yml"),
    )
    .expect("materialization workflow is readable");
    assert!(!workflow.to_ascii_lowercase().contains("python"));
    assert!(workflow.contains(
        "cargo test --locked -p xtask --test context_materialization_builder"
    ));
}
