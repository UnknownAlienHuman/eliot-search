from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PLANNER_PATH = REPOSITORY_ROOT / "tools/plan-ticket-issuance.py"
SPEC = importlib.util.spec_from_file_location("ticket_issuance_planner_v2_cli", PLANNER_PATH)
assert SPEC and SPEC.loader
planner = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = planner
SPEC.loader.exec_module(planner)


class FixtureRepository:
    def __init__(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="eliot-plan-v2-")
        self.root = Path(self._tmp.name)
        self._git("init")
        self._git("config", "user.email", "planner@example.invalid")
        self._git("config", "user.name", "Planner Tests")
        self._write_fixture()
        self.commit("fixture")

    def close(self) -> None:
        self._tmp.cleanup()

    def _git(self, *args: str, input_bytes: bytes | None = None) -> str:
        cp = subprocess.run(
            ["git", "-C", str(self.root), *args],
            input=input_bytes,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return cp.stdout.decode("utf-8", "replace").strip()

    def write(self, relative: str, text: str | bytes) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(text, bytes):
            path.write_bytes(text)
        else:
            path.write_text(text, encoding="utf-8", newline="\n")

    def read(self, relative: str) -> str:
        return (self.root / relative).read_text(encoding="utf-8")

    def replace(self, relative: str, old: str, new: str) -> None:
        text = self.read(relative)
        if text.count(old) != 1:
            raise AssertionError(f"{relative}: expected one {old!r}")
        self.write(relative, text.replace(old, new))

    def commit(self, message: str = "mutation") -> str:
        self._git("add", "-A")
        self._git("commit", "-m", message)
        return self.tagged_head

    @property
    def tagged_head(self) -> str:
        algorithm = self._git("rev-parse", "--show-object-format")
        return f"{algorithm}:{self._git('rev-parse', 'HEAD')}"

    def commit_index_symlink(self, relative: str, target: str) -> str:
        blob = self._git("hash-object", "-w", "--stdin", input_bytes=target.encode("utf-8"))
        self._git("update-index", "--add", "--cacheinfo", f"120000,{blob},{relative}")
        self._git("commit", "-m", "symlink source")
        return self.tagged_head

    def _ticket(self, package: str, launch_class: str) -> str:
        conditional = package != "search-contracts"
        dependency_fields = (
            'required_handoff_packages = ["search-contracts"]\n'
            'accepted_handoff_refs = []\n'
            'required_contract_commit = "UNSELECTED"\n'
            'required_contract_api_schema_digest = "UNAVAILABLE"\n'
            'status = "UNAVAILABLE"'
            if conditional
            else 'required_handoff_packages = []\naccepted_handoff_refs = []\nstatus = "NOT_REQUIRED"'
        )
        soft = {"search-contracts": 8000, "search-domain": 7000, "search-ports": 5500}[package]
        precondition = (
            "CURRENTLY_PRESENT"
            if not conditional
            else "ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED"
        )
        return f'''schema_version = 2
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
'''

    def _context(self, package: str) -> str:
        conditional = package != "search-contracts"
        if package == "search-contracts":
            required = [
                "README.md",
                "CANONICAL_TYPES.md",
                "TYPE_REGISTRY.md",
            ]
            sources = [
                "AGENTS.md",
                "crates/search-contracts/AGENTS.md",
                "docs/handoff/AUTHORITY_MAP.md",
                "swarm/ASSIGNMENT_PROTOCOL.md",
                "swarm/assignments/search-contracts.md",
                "docs/handoff/P00_BOOTSTRAP.md",
                "docs/contracts/p00/README.md",
                "docs/contracts/p00/manifest.toml",
                *[f"docs/contracts/p00/{name}" for name in required[1:]],
            ]
        else:
            sources = ["AGENTS.md", f"crates/{package}/AGENTS.md"]
        selectors = [
            f"swarm/crates.toml::package[name={package}]",
            f"swarm/function-packets.toml::foundation[package={package}]",
            "swarm/stages.toml::stage[id=W0]",
            (
                f"swarm/launch-state.toml::conditional_packages[{package}]"
                if conditional
                else "swarm/launch-state.toml::authorized_packages[search-contracts]"
            ),
        ]
        if conditional:
            selectors.append(
                f"swarm/launch-state.toml::conditional_activation.{package}"
            )
        slots = (
            '["search-contracts::accepted_package_and_api_handoff"]'
            if conditional
            else "[]"
        )
        source_lines = ",\n  ".join(json.dumps(value) for value in sources)
        selector_lines = ",\n  ".join(json.dumps(value) for value in selectors)
        return f'''schema_version = 2
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
source_file_count = {len(sources)}
registry_fragment_count = {len(selectors)}
accepted_handoff_slot_count = {1 if conditional else 0}

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
'''

    def _write_fixture(self) -> None:
        self.write("AGENTS.md", "# root\n")
        for package in ("search-contracts", "search-domain", "search-ports"):
            self.write(f"crates/{package}/AGENTS.md", f"# {package}\n")
            self.write(f"swarm/assignments/{package}.md", f"# {package} assignment\n")

        self.write("docs/handoff/AUTHORITY_MAP.md", "# authority\n")
        self.write("docs/handoff/P00_BOOTSTRAP.md", "# bootstrap\n")
        self.write("swarm/ASSIGNMENT_PROTOCOL.md", "# assignment protocol\n")
        self.write(
            "docs/contracts/p00/manifest.toml",
            'schema_version = 1\nrequired_files = ["README.md", "CANONICAL_TYPES.md", "TYPE_REGISTRY.md"]\n',
        )
        for name in ("README.md", "CANONICAL_TYPES.md", "TYPE_REGISTRY.md"):
            self.write(f"docs/contracts/p00/{name}", f"# {name}\n")

        self.write(
            "swarm/crates.toml",
            '''schema_version = 7
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
''',
        )
        self.write(
            "swarm/function-packets.toml",
            '''schema_version = 1
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
''',
        )
        self.write(
            "swarm/stages.toml",
            '''schema_version = 1
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
''',
        )
        self.write(
            "swarm/launch-state.toml",
            '''schema_version = 6
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
''',
        )
        self.write(
            "swarm/orchestration.toml",
            '''schema_version = 5
workflow_policy = "manual_only"
consumer_uses_branch_head = false
consumer_requires_exact_commit_and_api_digest = true
''',
        )
        self.write("swarm/control-plane-schema.toml", "schema_version = 3\n")
        self.write("swarm/schemas/types-v1.toml", "schema_version = 2\n")
        self.write(
            "swarm/ticket-issuance-plan-schema-v2.toml",
            'schema_version = 2\nrecord_kind = "ticket_issuance_plan_v2"\n',
        )
        self.write(
            "swarm/ticket-issuance-plan-digest-v2.toml",
            'schema_version = 2\nself_referential_digest_allowed = false\n',
        )
        self.write(
            "swarm/ticket-issuance-planner-v2.toml",
            'schema_version = 2\ncomponent = "ticket_issuance_planner_v2"\n',
        )
        self.write(
            "swarm/p00-foundation-acceptance.toml",
            'schema_version = 1\nstatus = "DESIGNED_NOT_EXECUTED"\n',
        )
        self.write(
            "swarm/ticket-drafts/manifest.toml",
            '''schema_version = 2
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
''',
        )
        self.write(
            "swarm/context-drafts/manifest.toml",
            '''schema_version = 2
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
''',
        )
        for package, launch_class in (
            ("search-contracts", "AUTHORIZED"),
            ("search-domain", "CONDITIONAL"),
            ("search-ports", "CONDITIONAL"),
        ):
            self.write(
                f"swarm/ticket-drafts/p00/{package}.toml",
                self._ticket(package, launch_class),
            )
            self.write(
                f"swarm/context-drafts/p00/{package}.toml",
                self._context(package),
            )

        for root in planner.CONTROL_ROOTS:
            self.write(f"{root}/README.md", "# reserved\n")

        self.write(
            ".github/workflows/manual.yml",
            '''name: Manual
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
''',
        )

    def add_accepted_contracts_handoff(self, valid_signature: bool = True) -> tuple[str, str]:
        accepted_commit = self.tagged_head
        api = "1" * 64
        errors = "2" * 64
        prefix = f'''schema_version = 1
record_kind = "package_handoff_v1"
status = "ACCEPTED"

[identity]
handoff_id = "contracts-001"
operation_id = "{'3' * 64}"
package = "search-contracts"
stage = "W0"
accepted_at = "2026-08-31T00:00:00Z"

[accepted_code]
base_commit = "{accepted_commit}"
final_commit = "{accepted_commit}"
changed_files_digest = "{'4' * 64}"

[public_surface]
api_manifest_ref = "artifact:contracts-api"
api_schema_digest = "{api}"
configuration_digest = "ABSENT"
fixture_digest_set = []
error_reason_digest = "{errors}"

'''
        digest = hashlib.sha256(prefix.encode("utf-8")).hexdigest()
        if not valid_signature:
            digest = "0" * 64
        text = prefix + f'''[signature]
record_sha256 = "{digest}"
integration_signature_ref = "signature:contracts"
'''
        path = "swarm/handoffs/search-contracts/contracts-001.toml"
        self.write(path, text)
        return path, self.commit("accepted contracts handoff")


def arguments(root: Path, package: str = "search-contracts", **overrides: object) -> argparse.Namespace:
    values: dict[str, object] = {
        "root": str(root),
        "package": package,
        "base_commit": None,
        "writer": None,
        "reviewer": None,
        "accepted_handoff": [],
        "output": "-",
        "require_ready": False,
    }
    values.update(overrides)
    return argparse.Namespace(**values)


def reasons(plan: dict[str, object]) -> set[str]:
    return set(plan["reason_codes"])  # type: ignore[arg-type]

