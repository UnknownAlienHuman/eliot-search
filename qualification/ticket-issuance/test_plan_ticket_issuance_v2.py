from __future__ import annotations

import json
import unittest

from fixture_plan_ticket_issuance_v2 import (
    FixtureRepository,
    arguments,
    planner,
    reasons,
)


class PlannerV2Tests(unittest.TestCase):
    maxDiff = None

    def setUp(self) -> None:
        self.fixture = FixtureRepository()

    def tearDown(self) -> None:
        self.fixture.close()

    def build(self, package: str = "search-contracts", **overrides: object) -> dict[str, object]:
        plan, _target = planner.build_plan(arguments(self.fixture.root, package, **overrides))
        return plan

    def complete(self, package: str = "search-contracts", **overrides: object) -> dict[str, object]:
        values = {
            "base_commit": self.fixture.tagged_head,
            "writer": "actor:service:writer-01",
            "reviewer": "actor:reviewer:reviewer-01",
        }
        values.update(overrides)
        return self.build(package, **values)

    def assert_non_authoritative(self, plan: dict[str, object]) -> None:
        self.assertEqual(plan["mutations"], [])
        for field in (
            "authorizes_context_materialization",
            "authorizes_ticket_issuance",
            "creates_writer_lease",
            "authorizes_implementation",
            "publishes_package_handoff",
            "advances_launch_state",
        ):
            self.assertIs(plan[field], False)
        payload = dict(plan)
        digest = payload.pop("plan_sha256")
        self.assertEqual(digest, planner.plan_digest(payload))

    def test_01_missing_selection(self) -> None:
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_MISSING)
        self.assertEqual(reasons(plan), set())
        self.assert_non_authoritative(plan)

    def test_02_deterministic(self) -> None:
        first = self.build()
        second = self.build()
        self.assertEqual(planner.canonical_json_bytes(first), planner.canonical_json_bytes(second))

    def test_03_complete_contracts_selection(self) -> None:
        plan = self.complete()
        self.assertEqual(plan["decision"], planner.DECISION_READY)
        self.assertEqual(reasons(plan), set())

    def test_04_partial_selection(self) -> None:
        plan = self.build(base_commit=self.fixture.tagged_head)
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("PARTIAL_ISSUANCE_SELECTION", reasons(plan))

    def test_05_invalid_actor(self) -> None:
        plan = self.complete(writer="display name")
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("ACTOR_IDENTITY_INVALID", reasons(plan))

    def test_06_same_actor(self) -> None:
        plan = self.complete(
            writer="actor:service:same",
            reviewer="actor:service:same",
        )
        self.assertEqual(plan["decision"], planner.DECISION_CONFLICT)
        self.assertIn("WRITER_REVIEWER_CONFLICT", reasons(plan))

    def test_07_abbreviated_base(self) -> None:
        plan = self.build(
            base_commit="deadbeef",
            writer="actor:service:writer",
            reviewer="actor:reviewer:reviewer",
        )
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("BASE_COMMIT_INVALID", reasons(plan))

    def test_08_working_tree_is_ignored(self) -> None:
        self.fixture.replace(
            "swarm/ticket-drafts/p00/search-contracts.toml",
            "claimable = false",
            "claimable = true",
        )
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_MISSING)
        self.assertNotIn("DRAFT_BECAME_CLAIMABLE", reasons(plan))

    def test_09_claimable_draft(self) -> None:
        self.fixture.replace(
            "swarm/ticket-drafts/p00/search-contracts.toml",
            "claimable = false",
            "claimable = true",
        )
        self.fixture.commit()
        plan = self.build()
        self.assertEqual(plan["decision"], planner.DECISION_INVALID)
        self.assertIn("DRAFT_BECAME_CLAIMABLE", reasons(plan))

    def test_10_premature_identity(self) -> None:
        self.fixture.replace(
            "swarm/ticket-drafts/p00/search-contracts.toml",
            'ticket_id = "UNASSIGNED"',
            'ticket_id = "ticket-1"',
        )
        self.fixture.commit()
        plan = self.build()
        self.assertIn("DRAFT_IDENTITY_PREMATURELY_RESOLVED", reasons(plan))

    def test_11_legacy_schema_field_rejected(self) -> None:
        self.fixture.replace(
            "swarm/ticket-drafts/p00/search-contracts.toml",
            'writer = "UNASSIGNED"',
            'lease_id = "UNASSIGNED"\nwriter = "UNASSIGNED"',
        )
        self.fixture.commit()
        plan = self.build()
        self.assertIn("DRAFT_UNKNOWN_FIELD", reasons(plan))

    def test_12_exact_contract_pack_drift(self) -> None:
        self.fixture.replace(
            "swarm/context-drafts/p00/search-contracts.toml",
            '  "docs/contracts/p00/TYPE_REGISTRY.md"',
            '  "docs/contracts/p00/CANONICAL_TYPES.md"',
        )
        self.fixture.commit()
        plan = self.build()
        self.assertIn("DRAFT_MANIFEST_MISMATCH", reasons(plan))

    def test_13_ordinary_budget_exceeded(self) -> None:
        path = "swarm/context-drafts/p00/search-domain.toml"
        self.fixture.replace(path, "source_file_count = 2", "source_file_count = 17")
        old = '''source_files = [
  "AGENTS.md",
  "crates/search-domain/AGENTS.md"
]'''
        extras = [f"docs/domain-extra-{i}.md" for i in range(15)]
        for rel in extras:
            self.fixture.write(rel, "# extra\n")
        values = ["AGENTS.md", "crates/search-domain/AGENTS.md", *extras]
        new = "source_files = [\n  " + ",\n  ".join(json.dumps(x) for x in values) + "\n]"
        self.fixture.replace(path, old, new)
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_BUDGET_EXCEEDED", reasons(plan))

    def test_14_source_missing(self) -> None:
        (self.fixture.root / "crates/search-domain/AGENTS.md").unlink()
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_SOURCE_MISSING", reasons(plan))

    def test_15_source_symlink_in_git(self) -> None:
        self.fixture.commit_index_symlink("crates/search-domain/AGENTS.md", "../target")
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_SOURCE_NOT_REGULAR", reasons(plan))

    def test_16_source_not_utf8(self) -> None:
        self.fixture.write("crates/search-domain/AGENTS.md", b"\xff\xfe")
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_SOURCE_NOT_UTF8", reasons(plan))

    def test_17_forbidden_source(self) -> None:
        path = "swarm/context-drafts/p00/search-domain.toml"
        self.fixture.write("docs/architecture/secret.md", "# no\n")
        self.fixture.replace(
            path,
            '  "crates/search-domain/AGENTS.md"',
            '  "docs/architecture/secret.md"',
        )
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_SOURCE_FORBIDDEN", reasons(plan))

    def test_18_invalid_selector(self) -> None:
        path = "swarm/context-drafts/p00/search-domain.toml"
        self.fixture.replace(
            path,
            "swarm/crates.toml::package[name=search-domain]",
            "swarm/crates.toml::package[package=search-domain]",
        )
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CONTEXT_SELECTOR_INVALID", reasons(plan))

    def test_19_nonunique_selector(self) -> None:
        crates = self.fixture.read("swarm/crates.toml")
        block = '''[[package]]
name = "search-domain"
path = "crates/search-domain"
family = "foundation"
wave = 0
soft_src_line_target = 7000
assignment = "swarm/assignments/search-domain.md"

'''
        self.fixture.write("swarm/crates.toml", crates + "\n" + block)
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertTrue(
            {"PACKAGE_UNKNOWN", "CONTEXT_SELECTOR_NOT_UNIQUE"} & reasons(plan)
        )

    def test_20_missing_conditional_handoff(self) -> None:
        plan = self.build("search-domain")
        self.assertEqual(plan["decision"], planner.DECISION_PREREQUISITE)
        self.assertIn("HANDOFF_SLOT_UNSATISFIED", reasons(plan))

    def test_21_valid_conditional_handoff(self) -> None:
        handoff, head = self.fixture.add_accepted_contracts_handoff()
        plan = self.build(
            "search-domain",
            base_commit=head,
            writer="actor:service:domain-writer",
            reviewer="actor:reviewer:domain-reviewer",
            accepted_handoff=[handoff],
        )
        self.assertEqual(plan["decision"], planner.DECISION_READY)
        self.assertEqual(reasons(plan), set())

    def test_22_invalid_handoff_signature(self) -> None:
        handoff, head = self.fixture.add_accepted_contracts_handoff(False)
        plan = self.build(
            "search-domain",
            base_commit=head,
            writer="actor:service:domain-writer",
            reviewer="actor:reviewer:domain-reviewer",
            accepted_handoff=[handoff],
        )
        self.assertIn("HANDOFF_RECORD_INVALID", reasons(plan))

    def test_23_unexpected_handoff_for_contracts(self) -> None:
        handoff, head = self.fixture.add_accepted_contracts_handoff()
        plan = self.build(
            base_commit=head,
            writer="actor:service:writer",
            reviewer="actor:reviewer:reviewer",
            accepted_handoff=[handoff],
        )
        self.assertIn("CURRENT_PACKAGE_CONTROL_RECORD_EXISTS", reasons(plan))

    def test_24_current_package_record(self) -> None:
        self.fixture.write("swarm/tickets/search-domain/ticket-1.toml", 'record_kind = "assignment_ticket_v1"\n')
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CURRENT_PACKAGE_CONTROL_RECORD_EXISTS", reasons(plan))

    def test_25_nested_readme_is_not_exempt(self) -> None:
        self.fixture.write("swarm/tickets/search-domain/README.md", "# not metadata\n")
        self.fixture.commit()
        plan = self.build("search-domain")
        self.assertIn("CURRENT_PACKAGE_CONTROL_RECORD_EXISTS", reasons(plan))

    def test_26_w0_receipt_conflict(self) -> None:
        self.fixture.write("swarm/wave-receipts/W0.toml", 'record_kind = "wave_receipt_v1"\n')
        self.fixture.commit()
        plan = self.build()
        self.assertIn("W0_ALREADY_ACCEPTED", reasons(plan))

    def test_27_automatic_workflow(self) -> None:
        self.fixture.write(
            ".github/workflows/automatic.yml",
            '''name: Automatic
on:
  push:
permissions:
  contents: read
jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          persist-credentials: false
''',
        )
        self.fixture.commit()
        plan = self.build()
        self.assertIn("WORKFLOW_POLICY_VIOLATION", reasons(plan))

    def test_28_output_outside_artifact_root(self) -> None:
        plan, target = planner.build_plan(
            arguments(self.fixture.root, output="swarm/tickets/plan.json")
        )
        self.assertIsNone(target)
        self.assertIn("OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT", reasons(plan))

    def test_29_output_inside_artifact_root(self) -> None:
        plan, target = planner.build_plan(
            arguments(
                self.fixture.root,
                output="artifacts/ticket-issuance-plans/contracts.json",
            )
        )
        self.assertIsNotNone(target)
        planner.write_plan(plan, target)
        parsed = json.loads(target.read_text(encoding="utf-8"))
        self.assertEqual(parsed["plan_sha256"], plan["plan_sha256"])

    def test_30_authority_and_digest(self) -> None:
        plan = self.complete()
        self.assert_non_authoritative(plan)


if __name__ == "__main__":
    result = unittest.main(verbosity=2, exit=False)
    raise SystemExit(0 if result.result.wasSuccessful() else 1)
