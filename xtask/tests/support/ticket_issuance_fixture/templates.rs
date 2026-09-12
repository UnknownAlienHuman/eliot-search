use super::FixtureRepository;

impl FixtureRepository {
    pub(super) fn ticket(package: &str, launch_class: &str) -> String {
        let conditional = package != "search-contracts";
        let dependency_fields = if conditional {
            concat!(
                "required_handoff_packages = [\"search-contracts\"]\n",
                "accepted_handoff_refs = []\n",
                "required_contract_commit = \"UNSELECTED\"\n",
                "required_contract_api_schema_digest = \"UNAVAILABLE\"\n",
                "status = \"UNAVAILABLE\"",
            )
        } else {
            concat!(
                "required_handoff_packages = []\n",
                "accepted_handoff_refs = []\n",
                "status = \"NOT_REQUIRED\"",
            )
        };
        let soft = match package {
            "search-contracts" => 8000,
            "search-domain" => 7000,
            "search-ports" => 5500,
            _ => panic!("unsupported fixture package"),
        };
        let precondition = if conditional {
            "ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED"
        } else {
            "CURRENTLY_PRESENT"
        };
        format!(
            r#"schema_version = 2
record_kind = "assignment_ticket_draft"
status = "DRAFT_ONLY_NOT_ISSUED"
claimable = false
authorizes_implementation = false
creates_lease = false
may_be_writer_acknowledged = false
package = "{package}"
stage = "W0"
phase = "P00"
wave = 0
launch_class = "{launch_class}"
launch_precondition = "{precondition}"
issuance_status = "BLOCKED_ON_IDENTITY_DIGEST_AND_CONTEXT_FREEZE"

[unresolved_identity]
ticket_id = "UNASSIGNED"
writer = "UNASSIGNED"
reviewer = "UNASSIGNED"
issued_at = ""
base_commit = "UNSELECTED"
branch_or_worktree = "UNSELECTED"
ticket_signed_payload_sha256 = "UNAVAILABLE"
ticket_exact_record_file_sha256 = "UNAVAILABLE"
integration_signature_ref = ""

[repository_fence]
repository = "UnknownAlienHuman/eliot-search"
write_scope = "crates/{package}/**"
feature_profile = "P00_FOUNDATION"
package_registry_path = "swarm/crates.toml"
function_registry_path = "swarm/function-packets.toml"
stage_registry_path = "swarm/stages.toml"
launch_state_path = "swarm/launch-state.toml"
registry_digests = "UNRESOLVED_AT_ISSUANCE"

[context]
context_draft = "swarm/context-drafts/p00/{package}.toml"
context_manifest_ref = "UNAVAILABLE"
context_artifact_ref = "UNAVAILABLE"
context_artifact_sha256 = "UNAVAILABLE"
writer_visible_artifact_count = 1
architecture_access = "exception-only"

[dependencies]
{dependency_fields}

[limits]
soft_src_lines = {soft}
split_review_total_lines = 8500
hard_total_lines = 10000
one_active_writer = true

[deliverables]
required_outputs = ["package_implementation_inside_write_scope"]
required_evidence = ["contract_test"]
issuance_requirements = ["materialize_context"]
"#,
        )
    }

    pub(super) fn context(package: &str) -> String {
        let conditional = package != "search-contracts";
        let sources: Vec<String> = if package == "search-contracts" {
            [
                "AGENTS.md",
                "crates/search-contracts/AGENTS.md",
                "docs/handoff/AUTHORITY_MAP.md",
                "swarm/ASSIGNMENT_PROTOCOL.md",
                "swarm/assignments/search-contracts.md",
                "docs/handoff/P00_BOOTSTRAP.md",
                "docs/contracts/p00/README.md",
                "docs/contracts/p00/manifest.toml",
                "docs/contracts/p00/CANONICAL_TYPES.md",
                "docs/contracts/p00/TYPE_REGISTRY.md",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        } else {
            vec![
                "AGENTS.md".to_owned(),
                format!("crates/{package}/AGENTS.md"),
            ]
        };
        let mut selectors = vec![
            format!("swarm/crates.toml::package[name={package}]"),
            format!(
                "swarm/function-packets.toml::foundation[package={package}]"
            ),
            "swarm/stages.toml::stage[id=W0]".to_owned(),
            if conditional {
                format!(
                    "swarm/launch-state.toml::conditional_packages[{package}]"
                )
            } else {
                "swarm/launch-state.toml::authorized_packages[search-contracts]"
                    .to_owned()
            },
        ];
        if conditional {
            selectors.push(format!(
                "swarm/launch-state.toml::conditional_activation.{package}"
            ));
        }
        let source_lines = render_array_lines(&sources);
        let selector_lines = render_array_lines(&selectors);
        let slots = if conditional {
            "[\"search-contracts::accepted_package_and_api_handoff\"]"
        } else {
            "[]"
        };
        format!(
            r#"schema_version = 2
record_kind = "writer_context_draft"
status = "UNMATERIALIZED_DRAFT"
claimable = false
authorizes_implementation = false
package = "{package}"
stage = "W0"
phase = "P00"
wave = 0
base_commit = "UNSELECTED"
materialized_context_manifest_ref = "UNAVAILABLE"
materialized_context_record_sha256 = "UNAVAILABLE"
materialized_context_artifact_ref = "UNAVAILABLE"
materialized_context_artifact_sha256 = "UNAVAILABLE"
materialization_mode = "canonical_concatenated_bundle"
writer_visible_artifact_count = 1
source_file_count = {source_count}
registry_fragment_count = {selector_count}
accepted_handoff_slot_count = {slot_count}

[canonicalization]
encoding = "UTF-8"
line_endings = "LF"
path_header_format = "--- repository-path: <path> ---"
registry_header_format = "--- registry-selector: <path>::<selector> ---"
preserve_declared_order = true
record_source_sha256 = true
record_fragment_sha256 = true

[content]
source_files = [
{source_lines}
]
registry_fragments = [
{selector_lines}
]
accepted_handoff_slots = {slots}
forbidden_paths = ["docs/architecture/**", "bins/**"]
required_unavailable_checks = ["real_toolchain"]
"#,
            source_count = sources.len(),
            selector_count = selectors.len(),
            slot_count = if conditional { 1 } else { 0 },
        )
    }
}

pub(super) fn render_array_lines(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("  {}", serde_json::to_string(value).expect("JSON string")))
        .collect::<Vec<_>>()
        .join(",\n")
}
