//! Checked-in context-materialization validation closure.

pub(super) const REGISTRY: &str =
    "swarm/context-materialization-planner-v1.toml";
pub(super) const SCHEMA: &str =
    "swarm/context-materialization-plan-schema-v1.toml";
pub(super) const DIGEST: &str =
    "swarm/context-materialization-plan-digest-v1.toml";
pub(super) const INSTANCE: &str =
    "swarm/context-manifest-instance-v1.toml";
pub(super) const RENDERER: &str =
    "swarm/context-manifest-renderer-v1.toml";
pub(super) const CASES: &str =
    "qualification/context-materialization/cases-v1.toml";
pub(super) const WORKFLOW: &str =
    ".github/workflows/context-materialization-plan.yml";

pub(super) const REQUIRED: [&str; 19] = [
    REGISTRY,
    SCHEMA,
    DIGEST,
    INSTANCE,
    RENDERER,
    "swarm/accepted-evidence-digest-v1.toml",
    "xtask/src/context_materialization_builder.rs",
    "xtask/src/context_materialization_builder/assemble.rs",
    "xtask/src/context_materialization_builder/input.rs",
    "xtask/src/context_materialization_builder/manifest.rs",
    "xtask/src/context_materialization_builder/model.rs",
    "xtask/src/context_materialization_builder/write.rs",
    "xtask/src/command/context_materialization_build.rs",
    "tools/plan-context-materialization.ps1",
    CASES,
    "xtask/tests/context_materialization_builder.rs",
    "xtask/tests/context_materialization_runtime_boundary.rs",
    "docs/handoff/CONTEXT_MATERIALIZATION_PLAN_V1.md",
    WORKFLOW,
];

pub(super) const RETIRED_PYTHON: [&str; 9] = [
    "tools/plan-context-materialization.py",
    "tools/context_materialization_planner_v1/__init__.py",
    "tools/context_materialization_planner_v1/core.py",
    "tools/context_materialization_planner_v1/manifest.py",
    "tools/context_materialization_planner_v1/plan.py",
    "qualification/context-materialization/test_context_materialization_plan_v1.py",
    "tools/context_artifact_builder_v1/__init__.py",
    "tools/context_artifact_builder_v1/core.py",
    "tools/context_artifact_builder_v1/bundle.py",
];
