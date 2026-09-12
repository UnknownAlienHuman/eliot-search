pub(super) const REGISTRY_PATH: &str = "swarm/context-artifact-builder-v1.toml";
pub(super) const SCHEMA_PATH: &str = "swarm/context-artifact-candidate-schema-v1.toml";
pub(super) const DIGEST_PATH: &str = "swarm/context-artifact-candidate-digest-v1.toml";
pub(super) const CASES_PATH: &str = "qualification/context-artifact/cases-v1.toml";

pub(super) const EXPECTED_PATHS: [(&str, &str); 13] = [
    ("contract", "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_V1.md"),
    (
        "digest_contract",
        "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md",
    ),
    ("index", "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_INDEX.md"),
    ("candidate_schema", SCHEMA_PATH),
    ("digest_profile", DIGEST_PATH),
    (
        "implementation",
        "tools/build-context-artifact-candidate.py",
    ),
    (
        "powershell_wrapper",
        "tools/build-context-artifact-candidate.ps1",
    ),
    (
        "structural_validator",
        "xtask/src/context_artifact_validation.rs",
    ),
    (
        "structural_validator_wrapper",
        "tools/validate-context-artifact-candidate.ps1",
    ),
    (
        "qualification_readme",
        "qualification/context-artifact/README.md",
    ),
    ("qualification_cases", CASES_PATH),
    (
        "qualification_tests",
        "qualification/context-artifact/test_context_artifact_candidate_v1.py",
    ),
    (
        "manual_workflow",
        ".github/workflows/context-artifact-candidate.yml",
    ),
];

pub(super) const IMPLEMENTATION_MODULES: [&str; 5] = [
    "tools/context_artifact_builder_v1/__init__.py",
    "tools/context_artifact_builder_v1/core.py",
    "tools/context_artifact_builder_v1/bundle.py",
    "tools/context_artifact_builder_v1/extract.py",
    "tools/context_artifact_builder_v1/build.py",
];

pub(super) const EXECUTION_TRUE_KEYS: [&str; 7] = [
    "deterministic",
    "repository_inputs_from_immutable_git_tree",
    "length_framed_bundle",
    "bundle_roundtrip_verification",
    "ordinary_artifact_writes_only",
    "idempotent_equal_output",
    "conflicting_existing_output_fails",
];

pub(super) const EXECUTION_FALSE_KEYS: [&str; 4] = [
    "network_required",
    "third_party_python_dependencies",
    "working_tree_source_of_truth",
    "control_root_writes",
];

pub(super) const TRUE_INVARIANTS: [&str; 9] = [
    "working_tree_used_as_input_must_be_false",
    "reason_codes_must_be_empty_on_success",
    "control_record_mutations_must_be_empty",
    "bundle_roundtrip_verified_must_be_true",
    "authoritative_artifact_store_readback_verified_must_be_false",
    "manifest_projection_schema_instance_must_be_false",
    "manifest_projection_unresolved_fields_must_be_exact",
    "candidate_id_is_domain_separated_bundle_digest",
    "candidate_sha256_is_non_circular_metadata_digest",
];

pub(super) const FALSE_INVARIANTS: [&str; 3] = [
    "local_candidate_file_is_immutable_artifact_ref",
    "wall_clock_time_in_candidate",
    "random_identity_in_candidate",
];
