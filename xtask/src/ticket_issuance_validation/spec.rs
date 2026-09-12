pub(super) const REGISTRY_PATH: &str = "swarm/ticket-issuance-planner-v2.toml";
pub(super) const SCHEMA_PATH: &str = "swarm/ticket-issuance-plan-schema-v2.toml";
pub(super) const DIGEST_PATH: &str = "swarm/ticket-issuance-plan-digest-v2.toml";
pub(super) const CASES_PATH: &str = "qualification/ticket-issuance/cases-v2.toml";

pub(super) const EXPECTED_PATHS: [(&str, &str); 15] = [
    ("contract", "docs/handoff/TICKET_ISSUANCE_PLANNER_V2.md"),
    (
        "digest_contract",
        "docs/handoff/TICKET_ISSUANCE_PLANNER_DIGEST_V2.md",
    ),
    ("index", "docs/handoff/TICKET_ISSUANCE_PLANNER_INDEX.md"),
    ("plan_schema", SCHEMA_PATH),
    ("digest_profile", DIGEST_PATH),
    ("implementation", "tools/plan-ticket-issuance.py"),
    ("powershell_wrapper", "tools/plan-ticket-issuance.ps1"),
    (
        "structural_validator",
        "xtask/src/ticket_issuance_validation.rs",
    ),
    (
        "structural_validator_wrapper",
        "tools/validate-ticket-issuance-plan.ps1",
    ),
    (
        "qualification_readme",
        "qualification/ticket-issuance/README.md",
    ),
    ("qualification_cases", CASES_PATH),
    (
        "qualification_fixture",
        "qualification/ticket-issuance/fixture_plan_ticket_issuance_v2.py",
    ),
    (
        "qualification_tests",
        "qualification/ticket-issuance/test_plan_ticket_issuance_v2.py",
    ),
    (
        "manual_workflow",
        ".github/workflows/ticket-issuance-plan.yml",
    ),
    ("artifact_root", "artifacts/ticket-issuance-plans"),
];

pub(super) const IMPLEMENTATION_MODULES: [&str; 6] = [
    "tools/ticket_issuance_planner_v2/__init__.py",
    "tools/ticket_issuance_planner_v2/core.py",
    "tools/ticket_issuance_planner_v2/drafts.py",
    "tools/ticket_issuance_planner_v2/context.py",
    "tools/ticket_issuance_planner_v2/control.py",
    "tools/ticket_issuance_planner_v2/plan.py",
];

pub(super) const REGISTRY_AUTHORITY_KEYS: [&str; 11] = [
    "output_is_control_record",
    "output_is_claimable",
    "output_is_evidence_receipt",
    "may_materialize_context",
    "may_issue_ticket",
    "may_issue_or_acknowledge_lease",
    "may_authorize_implementation",
    "may_record_submission_or_review",
    "may_publish_package_handoff",
    "may_accept_gate_or_wave",
    "may_advance_launch_state",
];

pub(super) const EXECUTION_TRUE_KEYS: [&str; 6] = [
    "deterministic",
    "repository_inputs_from_immutable_git_tree",
    "ordinary_artifact_write_optional",
    "exact_base_commit_validation_supported",
    "schema_v2_drafts_required",
    "manifest_owned_context_ceilings_required",
];

pub(super) const EXECUTION_FALSE_KEYS: [&str; 5] = [
    "network_required",
    "third_party_python_dependencies",
    "working_tree_source_of_truth",
    "repository_mutations",
    "control_root_writes",
];

pub(super) const TRUE_INVARIANTS: [&str; 8] = [
    "mutations_must_be_empty",
    "authorizes_context_materialization_must_be_false",
    "authorizes_ticket_issuance_must_be_false",
    "creates_writer_lease_must_be_false",
    "authorizes_implementation_must_be_false",
    "publishes_package_handoff_must_be_false",
    "advances_launch_state_must_be_false",
    "repository_inputs_are_immutable_git_tree_only",
];

pub(super) const FALSE_INVARIANTS: [&str; 3] = [
    "branch_head_is_authority",
    "wall_clock_time_in_output",
    "random_identity_in_output",
];

pub(super) const PLAN_AUTHORITY_FIELDS: [&str; 6] = [
    "authorizes_context_materialization",
    "authorizes_ticket_issuance",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "advances_launch_state",
];
