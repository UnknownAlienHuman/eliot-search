use xtask::ticket_planner::CONTROL_ROOTS;

use super::FixtureRepository;

impl FixtureRepository {
    pub(super) fn write_fixture(&self) {
        self.write_text("AGENTS.md", "# root\n");
        for package in ["search-contracts", "search-domain", "search-ports"] {
            self.write_text(
                &format!("crates/{package}/AGENTS.md"),
                &format!("# {package}\n"),
            );
            self.write_text(
                &format!("swarm/assignments/{package}.md"),
                &format!("# {package} assignment\n"),
            );
        }
        self.write_text("docs/handoff/AUTHORITY_MAP.md", "# authority\n");
        self.write_text("docs/handoff/P00_BOOTSTRAP.md", "# bootstrap\n");
        self.write_text("swarm/ASSIGNMENT_PROTOCOL.md", "# assignment protocol\n");
        self.write_text(
            "docs/contracts/p00/manifest.toml",
            concat!(
                "schema_version = 1\n",
                "required_files = [\"README.md\", \"CANONICAL_TYPES.md\", \"TYPE_REGISTRY.md\"]\n",
            ),
        );
        for name in ["README.md", "CANONICAL_TYPES.md", "TYPE_REGISTRY.md"] {
            self.write_text(
                &format!("docs/contracts/p00/{name}"),
                &format!("# {name}\n"),
            );
        }

        self.write_text(
            "swarm/crates.toml",
            r#"schema_version = 7
[[package]]
name = "search-contracts"
path = "crates/search-contracts"
family = "foundation"
wave = 0
soft_src_line_target = 7500
assignment = "swarm/assignments/search-contracts.md"

[[package]]
name = "search-domain"
path = "crates/search-domain"
family = "foundation"
wave = 0
soft_src_line_target = 7000
assignment = "swarm/assignments/search-domain.md"

[[package]]
name = "search-ports"
path = "crates/search-ports"
family = "foundation"
wave = 0
soft_src_line_target = 5500
assignment = "swarm/assignments/search-ports.md"
"#,
        );
        self.write_text(
            "swarm/function-packets.toml",
            r#"schema_version = 1
[[foundation]]
package = "search-contracts"
wave = 0
assignment = "swarm/assignments/search-contracts.md"
write_scope = "crates/search-contracts/**"

[[foundation]]
package = "search-domain"
wave = 0
assignment = "swarm/assignments/search-domain.md"
write_scope = "crates/search-domain/**"

[[foundation]]
package = "search-ports"
wave = 0
assignment = "swarm/assignments/search-ports.md"
write_scope = "crates/search-ports/**"
"#,
        );
        self.write_text(
            "swarm/stages.toml",
            r#"schema_version = 1
[[stage]]
id = "W0"
wave = 0
status = "ACTIVE_PACKAGE_ONLY"
packages = ["search-contracts", "search-domain", "search-ports"]

[[stage]]
id = "W1"
wave = 1
status = "BLOCKED"
requires_accepted_gates = ["G0"]
requires_accepted_receipts = ["W0"]
packages = []
"#,
        );
        self.write_text(
            "swarm/launch-state.toml",
            r#"schema_version = 6
active_stage = "P00"
active_wave = 0
orchestration_registry_schema_version = 5
orchestration_registry_path = "swarm/orchestration.toml"
authorized_packages = ["search-contracts"]
conditional_packages = ["search-domain", "search-ports"]

[conditional_activation.search-domain]
requires = ["accepted contracts handoff"]

[conditional_activation.search-ports]
requires = ["accepted contracts handoff"]
"#,
        );
        self.write_text(
            "swarm/orchestration.toml",
            concat!(
                "schema_version = 5\n",
                "workflow_policy = \"manual_only\"\n",
                "consumer_uses_branch_head = false\n",
                "consumer_requires_exact_commit_and_api_digest = true\n",
            ),
        );
        self.write_text("swarm/control-plane-schema.toml", "schema_version = 3\n");
        self.write_text("swarm/schemas/types-v1.toml", "schema_version = 2\n");
        self.write_text(
            "swarm/ticket-issuance-plan-schema-v2.toml",
            concat!(
                "schema_version = 2\n",
                "record_kind = \"ticket_issuance_plan_v2\"\n",
            ),
        );
        self.write_text(
            "swarm/ticket-issuance-plan-digest-v2.toml",
            concat!(
                "schema_version = 2\n",
                "self_referential_digest_allowed = false\n",
            ),
        );
        self.write_text(
            "swarm/ticket-issuance-planner-v2.toml",
            concat!(
                "schema_version = 2\n",
                "component = \"ticket_issuance_planner_v2\"\n",
            ),
        );
        self.write_text(
            "swarm/p00-foundation-acceptance.toml",
            concat!(
                "schema_version = 1\n",
                "status = \"DESIGNED_NOT_EXECUTED\"\n",
            ),
        );
        self.write_text(
            "swarm/ticket-drafts/manifest.toml",
            r#"schema_version = 2
ticket_draft_schema_version = 2
draft_count = 3
[[draft]]
package = "search-contracts"
path = "swarm/ticket-drafts/p00/search-contracts.toml"
[[draft]]
package = "search-domain"
path = "swarm/ticket-drafts/p00/search-domain.toml"
[[draft]]
package = "search-ports"
path = "swarm/ticket-drafts/p00/search-ports.toml"
"#,
        );
        self.write_text(
            "swarm/context-drafts/manifest.toml",
            r#"schema_version = 2
context_draft_schema_version = 2
draft_count = 3
ordinary_static_source_file_ceiling = 16
p00_exact_contract_pack_source_file_ceiling = 24
p00_exact_contract_pack_exception_packages = ["search-contracts"]
max_registry_fragments_per_context = 6
max_accepted_handoff_slots_per_context = 1

[[draft]]
package = "search-contracts"
path = "swarm/context-drafts/p00/search-contracts.toml"
source_ceiling_class = "P00_EXACT_CONTRACT_PACK"

[[draft]]
package = "search-domain"
path = "swarm/context-drafts/p00/search-domain.toml"
source_ceiling_class = "ORDINARY"

[[draft]]
package = "search-ports"
path = "swarm/context-drafts/p00/search-ports.toml"
source_ceiling_class = "ORDINARY"
"#,
        );
        for (package, launch_class) in [
            ("search-contracts", "AUTHORIZED"),
            ("search-domain", "CONDITIONAL"),
            ("search-ports", "CONDITIONAL"),
        ] {
            self.write_text(
                &format!("swarm/ticket-drafts/p00/{package}.toml"),
                &Self::ticket(package, launch_class),
            );
            self.write_text(
                &format!("swarm/context-drafts/p00/{package}.toml"),
                &Self::context(package),
            );
        }
        for root in CONTROL_ROOTS {
            self.write_text(&format!("{root}/README.md"), "# reserved\n");
        }
        self.write_text(
            ".github/workflows/manual.yml",
            r#"name: Manual
on:
  workflow_dispatch:
permissions:
  contents: read
jobs:
  validate:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          persist-credentials: false
"#,
        );
    }
}
