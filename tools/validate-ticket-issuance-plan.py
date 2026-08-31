#!/usr/bin/env python3
"""Validate the schema-v2 P00 ticket-issuance advisory planner.

This validator is read-only. A PASS proves registry/document/corpus closure and
the current repository's expected non-authoritative planner result.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable

EXPECTED_DECISIONS = (
    "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW",
    "BLOCKED_MISSING_SELECTION",
    "BLOCKED_PREREQUISITE",
    "BLOCKED_CONFLICT",
    "INVALID_REPOSITORY_STATE",
)
AUTHORITY_FIELDS = (
    "authorizes_context_materialization",
    "authorizes_ticket_issuance",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "advances_launch_state",
)


class Validation:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.checks: list[dict[str, str]] = []

    def require(self, condition: bool, check_id: str, detail: str) -> None:
        status = "PASS" if condition else "FAIL"
        self.checks.append({"id": check_id, "status": status, "detail": detail})
        if not condition:
            self.errors.append(f"{check_id}: {detail}")


def load_toml(root: Path, relative: str, validation: Validation) -> dict[str, Any]:
    path = root / relative
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (FileNotFoundError, tomllib.TOMLDecodeError) as exc:
        validation.require(False, f"file:{relative}", f"unreadable TOML: {exc}")
        return {}
    validation.require(isinstance(value, dict), f"file:{relative}", "TOML root is a table")
    return value if isinstance(value, dict) else {}


def string_array(value: Any) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        return ("__INVALID_STRING_ARRAY__",)
    return tuple(value)


def load_planner(root: Path):
    path = root / "tools/plan-ticket-issuance.py"
    spec = importlib.util.spec_from_file_location("ticket_issuance_planner_v2_validation", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load planner: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def validate(root: Path) -> dict[str, Any]:
    v = Validation()
    try:
        planner = load_planner(root)
    except Exception as exc:
        return {
            "schema_version": 2,
            "validator": "ticket_issuance_planner_v2",
            "status": "FAIL",
            "non_authoritative": True,
            "checks": [],
            "errors": [f"planner-load: {exc}"],
        }

    registry = load_toml(root, "swarm/ticket-issuance-planner-v2.toml", v)
    schema = load_toml(root, "swarm/ticket-issuance-plan-schema-v2.toml", v)
    digest = load_toml(root, "swarm/ticket-issuance-plan-digest-v2.toml", v)
    cases = load_toml(root, "qualification/ticket-issuance/cases-v2.toml", v)

    v.require(
        registry.get("schema_version") == 2
        and registry.get("component") == "ticket_issuance_planner_v2"
        and registry.get("status") == "ADVISORY_DRY_RUN_ONLY",
        "registry-identity",
        "planner registry is schema-v2 advisory-only",
    )
    expected_paths = {
        "contract": "docs/handoff/TICKET_ISSUANCE_PLANNER_V2.md",
        "digest_contract": "docs/handoff/TICKET_ISSUANCE_PLANNER_DIGEST_V2.md",
        "index": "docs/handoff/TICKET_ISSUANCE_PLANNER_INDEX.md",
        "plan_schema": "swarm/ticket-issuance-plan-schema-v2.toml",
        "digest_profile": "swarm/ticket-issuance-plan-digest-v2.toml",
        "implementation": "tools/plan-ticket-issuance.py",
        "powershell_wrapper": "tools/plan-ticket-issuance.ps1",
        "structural_validator": "tools/validate-ticket-issuance-plan.py",
        "structural_validator_wrapper": "tools/validate-ticket-issuance-plan.ps1",
        "qualification_readme": "qualification/ticket-issuance/README.md",
        "qualification_cases": "qualification/ticket-issuance/cases-v2.toml",
        "qualification_fixture": "qualification/ticket-issuance/fixture_plan_ticket_issuance_v2.py",
        "qualification_tests": "qualification/ticket-issuance/test_plan_ticket_issuance_v2.py",
        "manual_workflow": ".github/workflows/ticket-issuance-plan.yml",
        "artifact_root": "artifacts/ticket-issuance-plans",
    }
    for key, expected in expected_paths.items():
        value = registry.get(key)
        v.require(value == expected, f"registry-path:{key}", f"{key} = {expected}")
        if isinstance(value, str) and key != "artifact_root":
            v.require((root / value).is_file(), f"registry-file:{key}", f"registered file exists: {value}")

    expected_modules = (
        "tools/ticket_issuance_planner_v2/__init__.py",
        "tools/ticket_issuance_planner_v2/core.py",
        "tools/ticket_issuance_planner_v2/drafts.py",
        "tools/ticket_issuance_planner_v2/context.py",
        "tools/ticket_issuance_planner_v2/control.py",
        "tools/ticket_issuance_planner_v2/plan.py",
    )
    modules = string_array(registry.get("implementation_modules"))
    v.require(modules == expected_modules, "registry-modules", "implementation module set and order are exact")
    for relative in modules:
        v.require((root / relative).is_file(), f"registry-module:{relative}", f"registered module exists: {relative}")

    authority = registry.get("authority")
    v.require(isinstance(authority, dict) and bool(authority), "registry-authority-table", "authority table exists")
    if isinstance(authority, dict):
        expected_authority_keys = {
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
        }
        v.require(set(authority) == expected_authority_keys, "registry-authority-keys", "authority key set is closed")
        v.require(all(value is False for value in authority.values()), "registry-authority-false", "all authority flags are false")

    execution = registry.get("execution")
    v.require(isinstance(execution, dict), "registry-execution", "execution table exists")
    if isinstance(execution, dict):
        for key in (
            "deterministic",
            "repository_inputs_from_immutable_git_tree",
            "ordinary_artifact_write_optional",
            "exact_base_commit_validation_supported",
            "schema_v2_drafts_required",
            "manifest_owned_context_ceilings_required",
        ):
            v.require(execution.get(key) is True, f"execution:{key}", f"{key} is true")
        for key in (
            "network_required",
            "third_party_python_dependencies",
            "working_tree_source_of_truth",
            "repository_mutations",
            "control_root_writes",
        ):
            v.require(execution.get(key) is False, f"execution:{key}", f"{key} is false")

    v.require(
        schema.get("schema_version") == 2
        and schema.get("record_kind") == planner.RECORD_KIND
        and schema.get("status") == planner.STATUS,
        "schema-identity",
        "plan schema identity matches implementation",
    )
    v.require(
        string_array(schema.get("closed_decisions")) == EXPECTED_DECISIONS,
        "schema-decisions",
        "exact five-decision registry",
    )
    v.require(
        string_array(schema.get("closed_reason_codes")) == tuple(planner.CLOSED_REASON_CODES),
        "schema-reasons",
        "machine reason registry equals implementation",
    )
    v.require(
        schema.get("output_artifact_root") == planner.PLAN_ARTIFACT_ROOT
        and schema.get("all_other_output_paths_allowed") is False
        and schema.get("working_tree_input_allowed") is False,
        "schema-output-boundary",
        "artifact root and immutable-input boundary are exact",
    )
    invariants = schema.get("invariants")
    v.require(isinstance(invariants, dict) and bool(invariants), "schema-invariants", "invariant table exists")
    if isinstance(invariants, dict):
        true_invariants = (
            "mutations_must_be_empty",
            "authorizes_context_materialization_must_be_false",
            "authorizes_ticket_issuance_must_be_false",
            "creates_writer_lease_must_be_false",
            "authorizes_implementation_must_be_false",
            "publishes_package_handoff_must_be_false",
            "advances_launch_state_must_be_false",
            "repository_inputs_are_immutable_git_tree_only",
        )
        false_invariants = (
            "branch_head_is_authority",
            "wall_clock_time_in_output",
            "random_identity_in_output",
        )
        v.require(
            set(invariants) == set(true_invariants) | set(false_invariants),
            "schema-invariant-keys",
            "schema invariant key set is closed",
        )
        for key in true_invariants:
            v.require(invariants.get(key) is True, f"schema-invariant:{key}", f"{key} is true")
        for key in false_invariants:
            v.require(invariants.get(key) is False, f"schema-invariant:{key}", f"{key} is false")
    for key in ("output_is_control_record", "output_is_evidence_receipt", "output_is_claimable"):
        v.require(schema.get(key) is False, f"schema:{key}", f"{key} is false")

    v.require(
        digest.get("schema_version") == 2
        and digest.get("profile") == "ticket_issuance_plan_digest_v2"
        and digest.get("domain_separator_ascii") == "eliot-search/ticket-issuance-plan/v2\\0"
        and digest.get("canonical_payload") == "complete_canonical_plan_object_with_plan_sha256_field_omitted",
        "digest-identity",
        "digest profile and payload are exact",
    )
    for key in ("self_referential_digest_allowed", "placeholder_replacement_allowed", "parsed_reserialization_allowed"):
        v.require(digest.get(key) is False, f"digest:{key}", f"{key} is false")

    case_rows = cases.get("case")
    case_ids = tuple(row.get("id") for row in case_rows) if isinstance(case_rows, list) else ()
    v.require(cases.get("schema_version") == 2 and cases.get("case_count") == 30, "cases-count", "30 schema-v2 cases")
    v.require(
        case_ids == tuple(f"PLAN2-{index:03d}" for index in range(1, 31)),
        "cases-ids",
        "case IDs are exact and ordered",
    )

    text_requirements = {
        "docs/handoff/TICKET_ISSUANCE_PLANNER_V2.md": (
            "working tree is never a source of truth",
            "ticket_signed_payload_sha256",
            "materialized_context_manifest_ref",
            "ORDINARY                 <= 16",
            "P00_EXACT_CONTRACT_PACK  <= 24",
            "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW",
            "authorizes_context_materialization = false",
        ),
        "docs/handoff/TICKET_ISSUANCE_PLANNER_DIGEST_V2.md": (
            "with only `plan_sha256` omitted",
            "fixed-point or self-referential hashing",
        ),
        "qualification/ticket-issuance/README.md": (
            "30 cases",
            "BLOCKED_MISSING_SELECTION",
            "not:",
        ),
    }
    for relative, tokens in text_requirements.items():
        path = root / relative
        text = path.read_text(encoding="utf-8") if path.is_file() else ""
        for token in tokens:
            v.require(token in text, f"text:{relative}:{token}", f"{relative} contains {token}")

    workflow_path = root / ".github/workflows/ticket-issuance-plan.yml"
    workflow = workflow_path.read_text(encoding="utf-8") if workflow_path.is_file() else ""
    workflow_ok = (
        re.search(r"^\s{2}workflow_dispatch:\s*$", workflow, re.MULTILINE) is not None
        and re.search(
            r"^\s{0,6}(?:push|pull_request|schedule|workflow_run|repository_dispatch|workflow_call):\s*$",
            workflow,
            re.MULTILINE,
        )
        is None
        and re.search(r"^\s{2}contents:\s*read\s*$", workflow, re.MULTILINE) is not None
        and "persist-credentials: false" in workflow
    )
    v.require(workflow_ok, "workflow-policy", "planner workflow is manual/read-only/credential-free")

    artifact_root = root / planner.PLAN_ARTIFACT_ROOT
    v.require((artifact_root / "README.md").is_file(), "artifact-readme", "artifact root README exists")
    v.require((artifact_root / ".gitignore").is_file(), "artifact-ignore", "generated JSON is ignored")

    try:
        args = argparse.Namespace(
            root=str(root),
            package="search-contracts",
            base_commit=None,
            writer=None,
            reviewer=None,
            accepted_handoff=[],
            output="-",
            require_ready=False,
        )
        plan, target = planner.build_plan(args)
    except Exception as exc:
        v.require(False, "actual-plan", f"current repository planner failed: {exc}")
        plan = {}
        target = None
    else:
        v.require(target is None, "actual-output", "current validation uses stdout")
        v.require(plan.get("decision") == planner.DECISION_MISSING, "actual-decision", "current decision is BLOCKED_MISSING_SELECTION")
        v.require(plan.get("reason_codes") == [], "actual-reasons", "current repository has no structural planner reason")
        v.require(plan.get("mutations") == [], "actual-mutations", "current plan contains no mutations")
        for field in AUTHORITY_FIELDS:
            v.require(plan.get(field) is False, f"actual:{field}", f"{field} is false")
        payload = dict(plan)
        embedded = payload.pop("plan_sha256", None)
        v.require(embedded == planner.plan_digest(payload), "actual-digest", "embedded plan digest is non-circular and exact")

    return {
        "schema_version": 2,
        "validator": "ticket_issuance_planner_v2",
        "status": "PASS" if not v.errors else "FAIL",
        "non_authoritative": True,
        "package_acceptance_claimed": False,
        "g0_acceptance_claimed": False,
        "w0_acceptance_claimed": False,
        "w1_authority_claimed": False,
        "checks": v.checks,
        "errors": v.errors,
    }


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--json", action="store_true", help="emit compact JSON")
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    report = validate(Path(args.root).resolve())
    if args.json:
        print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    elif report["status"] == "PASS":
        print(f"PASS: {len(report['checks'])} planner-v2 structural checks")
    else:
        print(f"FAIL: {len(report['errors'])} error(s)", file=sys.stderr)
        for error in report["errors"]:
            print(f"- {error}", file=sys.stderr)
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
