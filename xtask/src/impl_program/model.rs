/// Manual-only workflow guarding the program closure.
pub const WORKFLOW: &str = ".github/workflows/implementation-program.yml";
/// Token proving the workflow invokes this Rust entrypoint.
pub const WORKFLOW_XTASK_TOKEN: &str = "validate implementation-program";
/// Central stage order W0-W10.
pub const EXPECTED_STAGE_IDS: [&str; 11] = [
    "W0", "W1", "W2", "W3", "W4", "W5", "W6", "W7", "W8", "W9", "W10",
];
/// Gate registry G0-G6.
pub const EXPECTED_GATE_IDS: [&str; 7] = ["G0", "G1", "G2", "G3", "G4", "G5", "G6"];
/// `(program key, registry path)` pairs in check order.
pub const EXPECTED_PATHS: [(&str, &str); 9] = [
    ("launch_authority", "swarm/launch-state.toml"),
    ("package_registry", "swarm/crates.toml"),
    ("function_registry", "swarm/function-packets.toml"),
    ("module_registry", "swarm/module-packets.toml"),
    ("package_map_index", "swarm/coverage/package-map-index.toml"),
    ("stage_registry", "swarm/stages.toml"),
    ("stage_readset_registry", "swarm/stage-readsets.toml"),
    ("gate_registry", "swarm/gates.toml"),
    ("configuration_registry", "config/sections.toml"),
];
/// (`target id`, `required stage`) pairs.
pub const EXPECTED_TARGETS: [(&str, &str); 6] = [
    ("buildable_workspace", "W0"),
    ("bootable_service_shell", "W1"),
    ("direct_source_product", "W2"),
    ("useful_baseline_search", "W4"),
    ("release_candidate", "W9"),
    ("optional_depth", "W10"),
];
/// Integration bootstrap order.
pub const EXPECTED_INTEGRATION_ORDER: [&str; 5] = [
    "pin_windows_toolchain",
    "lock_dependency_graph",
    "freeze_build_profiles",
    "establish_test_harness",
    "freeze_artifact_and_data_layout",
];
/// First implementation sequence order.
pub const EXPECTED_NEXT_ORDER: [&str; 7] = [
    "integration_bootstrap_pr",
    "search_contracts_implementation",
    "search_contracts_review_and_handoff",
    "search_domain_implementation",
    "search_ports_implementation",
    "w0_g0_evidence_and_acceptance",
    "advance_launch_to_w1",
];
/// Baseline release gate/receipt sequence.
pub const EXPECTED_BASELINE_REQUIRES: [&str; 7] =
    ["G0", "G1", "G2", "G3", "W7_LIFECYCLE", "G4", "G5"];

/// Validator outcome: `complete == false` renders the minimal early-failure
/// object (unreadable registries), exactly like the Python `except` path.
pub struct ProgramReport {
    /// Whether all registries loaded (minimal `{"errors","status"}` when false).
    pub complete: bool,
    /// PASS (no errors) vs FAIL.
    pub passed: bool,
    /// Program stage row count.
    pub stages: usize,
    /// Package registry row count.
    pub packages: usize,
    /// Program target row count.
    pub targets: usize,
    /// Integration step row count.
    pub integration_steps: usize,
    /// Next-step row count.
    pub next_steps: usize,
    /// Baseline release requirement count.
    pub baseline_requirements: usize,
    /// Program `active_stage` (`None` renders `null`).
    pub current_stage: Option<String>,
    /// Program `active_wave` (`None` renders `null`).
    pub current_wave: Option<i64>,
    /// Whether `Cargo.lock` exists on disk.
    pub cargo_lock_present: bool,
    /// Accumulated error messages in check order.
    pub errors: Vec<String>,
}

pub(super) struct Docs {
    pub(super) program: toml::Value,
    pub(super) launch: toml::Value,
    pub(super) stages_doc: toml::Value,
    pub(super) gates_doc: toml::Value,
    pub(super) packages_doc: toml::Value,
    pub(super) metrics: toml::Value,
    pub(super) coverage: toml::Value,
    pub(super) cases: toml::Value,
}

pub(super) struct Indexes<'a> {
    pub(super) program_stages: Vec<(String, &'a toml::Value)>,
    pub(super) stages: Vec<(String, &'a toml::Value)>,
    pub(super) gates: Vec<(String, &'a toml::Value)>,
    pub(super) packages: Vec<(String, &'a toml::Value)>,
    pub(super) targets: Vec<(String, &'a toml::Value)>,
    pub(super) integration_steps: Vec<(String, &'a toml::Value)>,
    pub(super) next_steps: Vec<(String, &'a toml::Value)>,
}

pub(super) struct CurrentView {
    pub(super) stage: Option<String>,
    pub(super) wave: Option<i64>,
    pub(super) lock_present: bool,
}
