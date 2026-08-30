from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PLANNER_PATH = REPOSITORY_ROOT / "tools/plan-ticket-issuance.py"
SPEC = importlib.util.spec_from_file_location("ticket_issuance_planner", PLANNER_PATH)
assert SPEC and SPEC.loader
planner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(planner)


class FixtureRepository:
    def __init__(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix="eliot-ticket-plan-")
        self.root = Path(self._temporary.name)
        self._write_fixture()
        self._git("init")
        self._git("config", "user.email", "planner-test@example.invalid")
        self._git("config", "user.name", "Planner Test")
        self._git("add", ".")
        self._git("commit", "-m", "fixture")

    def close(self) -> None:
        self._temporary.cleanup()

    def _git(self, *args: str) -> str:
        completed = subprocess.run(
            ["git", "-C", str(self.root), *args],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )
        return completed.stdout.strip()

    @property
    def tagged_head(self) -> str:
        object_format = self._git("rev-parse", "--show-object-format")
        return f"{object_format}:{self._git('rev-parse', 'HEAD')}"

    def write(self, relative: str, text: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")

    def _ticket(self, package: str, launch_class: str, context_path: str, dependencies: str) -> str:
        return f'''schema_version = 1
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
launch_precondition = "CURRENTLY_PRESENT"
issuance_status = "BLOCKED_ON_IDENTITY_DIGEST_AND_CONTEXT_FREEZE"

[unresolved_identity]
ticket_id = "UNASSIGNED"
lease_id = "UNASSIGNED"
writer = "UNASSIGNED"
reviewer = "UNASSIGNED"
issued_at = ""
base_commit = "UNSELECTED"
branch_or_worktree = "UNSELECTED"
ticket_canonical_digest = "UNAVAILABLE"
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
context_draft = "{context_path}"
context_manifest_ref = "UNAVAILABLE"
context_artifact_ref = "UNAVAILABLE"
context_artifact_sha256 = "UNAVAILABLE"
writer_visible_artifact_count = 1
architecture_access = "exception-only"

[dependencies]
{dependencies}

[limits]
soft_src_lines = 8000
split_review_total_lines = 8500
hard_total_lines = 10000
one_active_writer = true

[deliverables]
required_outputs = ["package_implementation_inside_write_scope"]
required_evidence = ["contract_test"]
issuance_requirements = ["materialize_context"]
'''

    def _context(self, package: str, ticket_kind: str) -> str:
        conditional = package != "search-contracts"
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
            selectors.append(f"swarm/launch-state.toml::conditional_activation.{package}")
        selector_text = ",\n  ".join(json.dumps(value) for value in selectors)
        slots = '["search-contracts::accepted_package_and_api_handoff"]' if conditional else "[]"
        return f'''schema_version = 1
record_kind = "writer_context_draft"
status = "UNMATERIALIZED_DRAFT"
claimable = false
authorizes_implementation = false
package = "{package}"
stage = "W0"
phase = "P00"
wave = 0
base_commit = "UNSELECTED"
materialized_context_ref = "UNAVAILABLE"
materialized_context_sha256 = "UNAVAILABLE"
materialization_mode = "canonical_concatenated_bundle"
writer_visible_artifact_count = 1
source_file_count = 2
registry_fragment_count = {len(selectors)}
accepted_handoff_slot_count = {1 if conditional else 0}

[canonicalization]
encoding = "UTF-8"
line_endings = "LF"
preserve_declared_order = true
record_source_sha256 = true
record_fragment_sha256 = true

[content]
source_files = [
  "AGENTS.md",
  "crates/{package}/AGENTS.md",
]
registry_fragments = [
  {selector_text}
]
accepted_handoff_slots = {slots}
forbidden_paths = ["docs/architecture/**"]
required_unavailable_checks = ["real_toolchain"]
'''

    def _write_fixture(self) -> None:
        self.write("AGENTS.md", "# fixture\n")
        for package in ("search-contracts", "search-domain"):
            self.write(f"crates/{package}/AGENTS.md", f"# {package}\n")

        self.write(
            "swarm/crates.toml",
            '''schema_version = 7
[[package]]
name = "search-contracts"
path = "crates/search-contracts"
wave = 0
assignment = "swarm/assignments/search-contracts.md"

[[package]]
name = "search-domain"
path = "crates/search-domain"
wave = 0
assignment = "swarm/assignments/search-domain.md"
''',
        )
        self.write(
            "swarm/function-packets.toml",
            '''schema_version = 1
[[foundation]]
package = "search-contracts"
wave = 0
assignment = "swarm/assignments/search-contracts.md"
primary_contract = "docs/contracts/p00/README.md"
write_scope = "crates/search-contracts/**"

[[foundation]]
package = "search-domain"
wave = 0
assignment = "swarm/assignments/search-domain.md"
primary_contract = "docs/contracts/p00/SUPPORT_SCHEMAS.md"
write_scope = "crates/search-domain/**"
''',
        )
        self.write(
            "swarm/stages.toml",
            '''schema_version = 1
[[stage]]
id = "W0"
wave = 0
packages = ["search-contracts", "search-domain"]
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
conditional_packages = ["search-domain"]

[conditional_activation.search-domain]
requires = ["accepted search-contracts handoff"]
''',
        )
        self.write(
            "swarm/orchestration.toml",
            '''schema_version = 5
control_plane_schema_registry = "swarm/control-plane-schema.toml"
control_plane_type_registry = "swarm/schemas/types-v1.toml"
workflow_policy = "manual_only"
''',
        )
        self.write("swarm/control-plane-schema.toml", "schema_version = 3\n")
        self.write("swarm/schemas/types-v1.toml", "schema_version = 2\n")
        self.write(
            "swarm/ticket-issuance-plan-schema.toml",
            'schema_version = 1\nrecord_kind = "ticket_issuance_plan_v1"\n',
        )
        self.write(
            "swarm/ticket-issuance-plan-digest-v1.toml",
            '''schema_version = 1
self_referential_digest_allowed = false
canonical_payload = "complete_canonical_plan_object_with_plan_sha256_field_omitted"
''',
        )

        self.write(
            "swarm/ticket-drafts/manifest.toml",
            '''schema_version = 1
status = "DRAFT_ONLY_NOT_ISSUED"
draft_count = 2
[[draft]]
package = "search-contracts"
path = "swarm/ticket-drafts/p00/search-contracts.toml"
[[draft]]
package = "search-domain"
path = "swarm/ticket-drafts/p00/search-domain.toml"
''',
        )
        self.write(
            "swarm/context-drafts/manifest.toml",
            '''schema_version = 1
status = "NON_CLAIMABLE_CONTEXT_DRAFTS"
draft_count = 2
[[draft]]
package = "search-contracts"
path = "swarm/context-drafts/p00/search-contracts.toml"
[[draft]]
package = "search-domain"
path = "swarm/context-drafts/p00/search-domain.toml"
''',
        )
        self.write(
            "swarm/ticket-drafts/p00/search-contracts.toml",
            self._ticket(
                "search-contracts",
                "AUTHORIZED",
                "swarm/context-drafts/p00/search-contracts.toml",
                'required_handoff_packages = []\naccepted_handoff_refs = []\nstatus = "NOT_REQUIRED"',
            ),
        )
        self.write(
            "swarm/ticket-drafts/p00/search-domain.toml",
            self._ticket(
                "search-domain",
                "CONDITIONAL",
                "swarm/context-drafts/p00/search-domain.toml",
                'required_handoff_packages = ["search-contracts"]\naccepted_handoff_refs = []\nstatus = "UNAVAILABLE"',
            ),
        )
        self.write(
            "swarm/context-drafts/p00/search-contracts.toml",
            self._context("search-contracts", "AUTHORIZED"),
        )
        self.write(
            "swarm/context-drafts/p00/search-domain.toml",
            self._context("search-domain", "CONDITIONAL"),
        )

        for protected in planner.PROTECTED_ROOTS:
            self.write(f"{protected}/README.md", "# reserved\n")

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


class PlannerConformanceTests(unittest.TestCase):
    maxDiff = None

    def setUp(self) -> None:
        self.fixture = FixtureRepository()

    def tearDown(self) -> None:
        self.fixture.close()

    def build(self, package: str = "search-contracts", **overrides: object) -> dict[str, object]:
        plan, _ = planner.build_plan(arguments(self.fixture.root, package, **overrides))
        return plan

    def assert_non_authoritative(self, plan: dict[str, object]) -> None:
        self.assertEqual(plan["mutations"], [])
        self.assertIs(plan["authorizes_ticket_issuance"], False)
        self.assertIs(plan["creates_writer_lease"], False)
        self.assertIs(plan["authorizes_implementation"], False)
        self.assertIs(plan["publishes_package_handoff"], False)
        self.assertIs(plan["advances_launch_state"], False)
        digest = plan.pop("plan_sha256")
        self.assertEqual(digest, planner.plan_digest(plan))
        plan["plan_sha256"] = digest

    def test_missing_selection_is_advisory(self) -> None:
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_MISSING)
        self.assertEqual(reasons(plan), set())
        self.assert_non_authoritative(plan)

    def test_repeated_plans_are_byte_identical(self) -> None:
        first = self.build()
        second = self.build()
        self.assertEqual(planner.canonical_json_bytes(first), planner.canonical_json_bytes(second))

    def test_complete_valid_selection_is_preview_ready(self) -> None:
        plan = self.build(
            base_commit=self.fixture.tagged_head,
            writer="actor:service:writer-01",
            reviewer="actor:reviewer:reviewer-01",
        )
        self.assertEqual(plan["decision"], planner.DECISION_READY)
        self.assertEqual(reasons(plan), set())
        self.assert_non_authoritative(plan)

    def test_partial_selection_fails_closed(self) -> None:
        plan = self.build(base_commit=self.fixture.tagged_head)
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("PARTIAL_ISSUANCE_SELECTION", reasons(plan))

    def test_writer_reviewer_collision(self) -> None:
        plan = self.build(
            base_commit=self.fixture.tagged_head,
            writer="actor:service:same",
            reviewer="actor:service:same",
        )
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("WRITER_REVIEWER_CONFLICT", reasons(plan))

    def test_invalid_actor_identity(self) -> None:
        plan = self.build(
            base_commit=self.fixture.tagged_head,
            writer="writer display name",
            reviewer="actor:reviewer:reviewer-01",
        )
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("ACTOR_IDENTITY_INVALID", reasons(plan))

    def test_abbreviated_base_commit(self) -> None:
        plan = self.build(
            base_commit="deadbeef",
            writer="actor:service:writer-01",
            reviewer="actor:reviewer:reviewer-01",
        )
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("BASE_COMMIT_INVALID", reasons(plan))

    def test_missing_context_source(self) -> None:
        (self.fixture.root / "crates/search-contracts/AGENTS.md").unlink()
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("CONTEXT_SOURCE_MISSING", reasons(plan))

    def test_symlink_context_source(self) -> None:
        original = Path.is_symlink

        def fake_is_symlink(path: Path) -> bool:
            if path == self.fixture.root / "crates/search-contracts/AGENTS.md":
                return True
            return original(path)

        with mock.patch.object(Path, "is_symlink", fake_is_symlink):
            plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("CONTEXT_SOURCE_SYMLINK", reasons(plan))

    def test_context_traversal(self) -> None:
        path = self.fixture.root / "swarm/context-drafts/p00/search-contracts.toml"
        text = path.read_text(encoding="utf-8").replace(
            '"crates/search-contracts/AGENTS.md"', '"../outside"'
        )
        path.write_text(text, encoding="utf-8", newline="\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("CONTEXT_SOURCE_FORBIDDEN", reasons(plan))

    def test_invalid_selector(self) -> None:
        path = self.fixture.root / "swarm/context-drafts/p00/search-contracts.toml"
        text = path.read_text(encoding="utf-8").replace(
            "swarm/stages.toml::stage[id=W0]", "swarm/stages.toml::regex[.*]"
        )
        path.write_text(text, encoding="utf-8", newline="\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("CONTEXT_SELECTOR_INVALID", reasons(plan))

    def test_claimable_draft(self) -> None:
        path = self.fixture.root / "swarm/ticket-drafts/p00/search-contracts.toml"
        text = path.read_text(encoding="utf-8").replace("claimable = false", "claimable = true", 1)
        path.write_text(text, encoding="utf-8", newline="\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("DRAFT_BECAME_CLAIMABLE", reasons(plan))

    def test_premature_ticket_identity(self) -> None:
        path = self.fixture.root / "swarm/ticket-drafts/p00/search-contracts.toml"
        text = path.read_text(encoding="utf-8").replace(
            'ticket_id = "UNASSIGNED"', 'ticket_id = "premature"'
        )
        path.write_text(text, encoding="utf-8", newline="\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("DRAFT_IDENTITY_PREMATURELY_RESOLVED", reasons(plan))

    def test_conditional_package_without_handoff(self) -> None:
        plan = self.build(package="search-domain")
        self.assertEqual(plan["decision"], planner.DECISION_PREREQUISITE)
        self.assertIn("HANDOFF_SLOT_UNSATISFIED", reasons(plan))

    def test_unexpected_handoff(self) -> None:
        fake = (
            "search-domain=swarm/handoffs/search-domain/fake.toml,"
            + "0" * 64
            + ","
            + self.fixture.tagged_head
            + ","
            + "1" * 64
            + ","
            + "2" * 64
        )
        plan = self.build(accepted_handoff=[fake])
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("HANDOFF_SET_UNEXPECTED", reasons(plan))

    def test_issued_record_breaks_zero_state(self) -> None:
        self.fixture.write("swarm/tickets/search-contracts/issued.toml", "record_kind = \"x\"\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("PROTECTED_ROOT_NOT_ZERO_STATE", reasons(plan))

    def test_automatic_workflow_trigger(self) -> None:
        path = self.fixture.root / ".github/workflows/manual.yml"
        text = path.read_text(encoding="utf-8").replace(
            "on:\n  workflow_dispatch:\n", "on:\n  push:\n  workflow_dispatch:\n"
        )
        path.write_text(text, encoding="utf-8", newline="\n")
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("WORKFLOW_POLICY_VIOLATION", reasons(plan))

    def test_protected_output_path(self) -> None:
        plan = self.build(output="swarm/tickets/plan.json")
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("OUTPUT_PATH_PROTECTED", reasons(plan))


class RepositoryIntegrationTest(unittest.TestCase):
    def test_current_repository_search_contracts_is_non_authoritative(self) -> None:
        plan, target = planner.build_plan(arguments(REPOSITORY_ROOT))
        self.assertIsNone(target)
        self.assertEqual(plan["decision"], planner.DECISION_MISSING)
        self.assertEqual(plan["mutations"], [])
        for field in (
            "authorizes_ticket_issuance",
            "creates_writer_lease",
            "authorizes_implementation",
            "publishes_package_handoff",
            "advances_launch_state",
        ):
            self.assertIs(plan[field], False)


if __name__ == "__main__":
    unittest.main(verbosity=2)
