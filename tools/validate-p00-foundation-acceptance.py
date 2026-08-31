#!/usr/bin/env python3
"""Validate the non-authoritative P00 foundation acceptance registry.

This tool is read-only. A successful run proves structural agreement among the
P00 acceptance matrix, package/gate/stage registries, non-claimable drafts and
zero-state control roots. It does not accept a package, G0, W0 or W1 authority.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable, Mapping

EXPECTED_PACKAGES = ("search-contracts", "search-domain", "search-ports")
EXPECTED_W1_PACKAGES = (
    "search-config",
    "search-runtime-owner",
    "search-os-secrets",
    "search-control-redb",
    "search-provider-protocol",
    "eliot-searchd",
    "eliot-search",
)
EXPECTED_CHAIN = (
    "context_manifest_v1",
    "assignment_ticket_v1",
    "writer_lease_v1",
    "lease_event_v1:ACKNOWLEDGED",
    "package_submission_v1",
    "independent_review_v1:ACCEPT_SUBMISSION_FOR_INTEGRATION",
    "package_handoff_v1",
)
EXPECTED_G0 = (
    "architecture_hash_challenge",
    "workspace_registry_assignment_parity",
    "dependency_graph_acyclic",
    "dependency_direction_policy",
    "recipe_set_exact",
    "epoch_and_sentinel_contract",
    "canonical_public_schema_fixtures",
    "reason_code_registry",
    "contract_domain_tests",
    "dependency_source_and_license_policy",
)
PROTECTED_ROOTS = (
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
)
FORBIDDEN_WORKFLOW_TRIGGER = re.compile(
    r"^\s{0,6}(?:push|pull_request|pull_request_target|merge_group|schedule|workflow_run|"
    r"repository_dispatch|workflow_call|release|issues|issue_comment|discussion|"
    r"discussion_comment|create|delete|branch_protection_rule|check_run|check_suite|"
    r"deployment|deployment_status|fork|gollum|label|milestone|page_build|project|"
    r"project_card|project_column|public|registry_package|status|watch):\s*$",
    re.MULTILINE,
)


class Validation:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.checks: list[dict[str, str]] = []

    def require(self, condition: bool, check_id: str, detail: str) -> None:
        if condition:
            self.checks.append({"id": check_id, "status": "PASS", "detail": detail})
        else:
            self.checks.append({"id": check_id, "status": "FAIL", "detail": detail})
            self.errors.append(f"{check_id}: {detail}")


def load_toml(root: Path, relative: str, validation: Validation) -> dict[str, Any]:
    path = root / relative
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except FileNotFoundError:
        validation.require(False, f"file:{relative}", "required file is missing")
        return {}
    except tomllib.TOMLDecodeError as exc:
        validation.require(False, f"toml:{relative}", f"invalid TOML: {exc}")
        return {}
    validation.require(isinstance(value, dict), f"toml:{relative}", "TOML root is a table")
    return value if isinstance(value, dict) else {}


def find_table(rows: Any, key: str, expected: str) -> Mapping[str, Any] | None:
    if not isinstance(rows, list):
        return None
    matches = [row for row in rows if isinstance(row, dict) and row.get(key) == expected]
    return matches[0] if len(matches) == 1 else None


def strings(value: Any) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        return ()
    return tuple(value)


def validate_registry(root: Path, validation: Validation) -> dict[str, Any]:
    path = "swarm/p00-foundation-acceptance.toml"
    registry = load_toml(root, path, validation)
    validation.require(registry.get("schema_version") == 1, "registry-schema", "schema_version is 1")
    validation.require(
        registry.get("registry_kind") == "p00_foundation_acceptance_v1",
        "registry-kind",
        "registry kind is closed",
    )
    validation.require(
        registry.get("status") == "DESIGNED_NOT_EXECUTED",
        "registry-status",
        "registry remains designed, not executed",
    )
    validation.require(
        registry.get("owner") == "integration-owner",
        "registry-owner",
        "integration owner owns acceptance",
    )
    validation.require(
        (registry.get("stage"), registry.get("phase"), registry.get("wave"), registry.get("gate"))
        == ("W0", "P00", 0, "G0"),
        "registry-stage",
        "registry is bound to W0/P00/G0",
    )
    validation.require(registry.get("completion_receipt") == "W0", "registry-receipt", "W0 receipt")
    validation.require(registry.get("package_count") == 3, "registry-package-count", "three packages")
    validation.require(registry.get("checkpoint_count") == 4, "registry-checkpoint-count", "four checkpoints")
    validation.require(registry.get("g0_evidence_count") == 10, "registry-evidence-count", "ten G0 items")

    authority = registry.get("authority")
    authority_ok = isinstance(authority, dict) and authority and all(value is False for value in authority.values())
    validation.require(bool(authority_ok), "registry-no-authority", "all authority flags are false")

    packages = registry.get("package")
    package_names = tuple(row.get("name") for row in packages) if isinstance(packages, list) else ()
    validation.require(package_names == EXPECTED_PACKAGES, "registry-packages", "exact package order")
    for order, name in enumerate(EXPECTED_PACKAGES):
        row = find_table(packages, "name", name)
        validation.require(row is not None, f"package:{name}", "one package row exists")
        if row is None:
            continue
        expected_class = "AUTHORIZED" if name == "search-contracts" else "CONDITIONAL"
        expected_predecessors = () if name == "search-contracts" else ("search-contracts",)
        expected_parallel = () if name == "search-contracts" else (
            "search-ports" if name == "search-domain" else "search-domain",
        )
        validation.require(row.get("order") == order, f"package:{name}:order", "topological order")
        validation.require(row.get("launch_class") == expected_class, f"package:{name}:class", expected_class)
        validation.require(
            row.get("ticket_draft") == f"swarm/ticket-drafts/p00/{name}.toml",
            f"package:{name}:ticket",
            "exact ticket draft",
        )
        validation.require(
            row.get("context_draft") == f"swarm/context-drafts/p00/{name}.toml",
            f"package:{name}:context",
            "exact context draft",
        )
        validation.require(
            row.get("write_scope") == f"crates/{name}/**",
            f"package:{name}:scope",
            "package-only write scope",
        )
        validation.require(
            strings(row.get("required_handoff_packages")) == expected_predecessors,
            f"package:{name}:handoffs",
            "exact predecessor handoffs",
        )
        validation.require(
            strings(row.get("required_control_records")) == EXPECTED_CHAIN,
            f"package:{name}:chain",
            "complete control-record ladder",
        )
        validation.require(
            strings(row.get("may_run_parallel_with")) == expected_parallel,
            f"package:{name}:parallel",
            "closed parallelism declaration",
        )
        validation.require(
            row.get("acceptance_output") == "package_handoff_v1",
            f"package:{name}:output",
            "package handoff only",
        )

    checkpoints = registry.get("checkpoint")
    ids = tuple(row.get("id") for row in checkpoints) if isinstance(checkpoints, list) else ()
    validation.require(ids == ("P00-A", "P00-B", "P00-C", "P00-D"), "checkpoint-ids", "exact checkpoints")
    if isinstance(checkpoints, list):
        validation.require(
            tuple(row.get("order") for row in checkpoints) == (0, 1, 2, 3),
            "checkpoint-order",
            "checkpoint order is closed",
        )
        validation.require(
            all(isinstance(row.get("requires"), list) and row.get("requires") for row in checkpoints),
            "checkpoint-requires",
            "every checkpoint has prerequisites",
        )
        validation.require(
            all(isinstance(row.get("produces"), list) and row.get("produces") for row in checkpoints),
            "checkpoint-produces",
            "every checkpoint has outputs",
        )

    evidence = registry.get("evidence")
    evidence_ids = tuple(row.get("id") for row in evidence) if isinstance(evidence, list) else ()
    validation.require(evidence_ids == EXPECTED_G0, "registry-g0-evidence", "exact ordered G0 evidence set")
    if isinstance(evidence, list):
        for row in evidence:
            evidence_id = str(row.get("id", "unknown"))
            validation.require(row.get("required_state") == "PASS", f"evidence:{evidence_id}:required", "PASS required")
            validation.require(
                row.get("current_state") == "UNAVAILABLE",
                f"evidence:{evidence_id}:current",
                "current state remains unavailable",
            )
            validation.require(
                row.get("raw_output_required") is True,
                f"evidence:{evidence_id}:raw",
                "raw output required",
            )
            validation.require(
                row.get("independent_review_required") is True,
                f"evidence:{evidence_id}:review",
                "independent review required",
            )

    w0 = registry.get("w0_acceptance")
    validation.require(isinstance(w0, dict), "w0-table", "W0 acceptance table exists")
    if isinstance(w0, dict):
        validation.require(strings(w0.get("required_packages")) == EXPECTED_PACKAGES, "w0-packages", "three exact packages")
        validation.require(w0.get("required_gate") == "G0", "w0-gate", "G0 required")
        validation.require(w0.get("required_wave_receipt") == "W0", "w0-receipt", "W0 required")
        for key in (
            "raw_output_required",
            "independent_review_required",
            "requires_no_active_writer_leases",
            "requires_zero_unreviewed_submissions",
            "requires_append_only_package_handoffs",
            "package_handoff_does_not_accept_gate_or_wave",
            "receipt_and_launch_update_same_reviewed_change",
        ):
            validation.require(w0.get(key) is True, f"w0:{key}", f"{key} is true")

    w1 = registry.get("w1_unlock")
    validation.require(isinstance(w1, dict), "w1-table", "W1 unlock table exists")
    if isinstance(w1, dict):
        validation.require(w1.get("stage") == "W1", "w1-stage", "W1")
        validation.require(w1.get("current_state") == "BLOCKED", "w1-current", "W1 remains blocked")
        validation.require(w1.get("requires_accepted_gate") == "G0", "w1-gate", "G0 prerequisite")
        validation.require(w1.get("requires_accepted_receipt") == "W0", "w1-receipt", "W0 prerequisite")
        validation.require(w1.get("requires_launch_state_active_wave") == 1, "w1-wave", "wave 1 after advance")
        for key in (
            "configuration_or_stage_presence_authorizes",
            "package_presence_authorizes",
            "manual_workflow_authorizes",
        ):
            validation.require(w1.get(key) is False, f"w1:{key}", f"{key} is false")

    current = registry.get("current_repository_state")
    validation.require(isinstance(current, dict), "current-state", "current-state table exists")
    if isinstance(current, dict):
        for key in (
            "materialized_contexts",
            "issued_tickets",
            "active_writer_leases",
            "submissions",
            "accepted_reviews",
            "accepted_package_handoffs",
        ):
            validation.require(current.get(key) == 0, f"current:{key}", f"{key} remains zero")
        validation.require(current.get("accepted_g0_receipt") is False, "current:g0", "G0 absent")
        validation.require(current.get("accepted_w0_receipt") is False, "current:w0", "W0 absent")
        validation.require(
            (current.get("active_stage"), current.get("active_wave")) == ("P00", 0),
            "current:launch",
            "P00/W0 remains active",
        )
    return registry


def validate_cross_registries(root: Path, registry: Mapping[str, Any], validation: Validation) -> None:
    gates = load_toml(root, "swarm/gates.toml", validation)
    stages = load_toml(root, "swarm/stages.toml", validation)
    launch = load_toml(root, "swarm/launch-state.toml", validation)
    crates = load_toml(root, "swarm/crates.toml", validation)
    functions = load_toml(root, "swarm/function-packets.toml", validation)
    ticket_manifest = load_toml(root, "swarm/ticket-drafts/manifest.toml", validation)
    context_manifest = load_toml(root, "swarm/context-drafts/manifest.toml", validation)
    orchestration = load_toml(root, "swarm/orchestration.toml", validation)

    g0 = find_table(gates.get("gate"), "id", "G0")
    validation.require(g0 is not None, "gate-g0", "one G0 row exists")
    if g0 is not None:
        validation.require(strings(g0.get("required_evidence")) == EXPECTED_G0, "gate-g0-set", "G0 set matches registry")
        validation.require((g0.get("stage"), g0.get("wave")) == ("P00", 0), "gate-g0-stage", "G0 is P00/W0")

    w0 = find_table(stages.get("stage"), "id", "W0")
    w1 = find_table(stages.get("stage"), "id", "W1")
    validation.require(w0 is not None and w1 is not None, "stage-w0-w1", "W0 and W1 rows exist")
    if w0 is not None:
        validation.require(strings(w0.get("packages")) == EXPECTED_PACKAGES, "stage-w0-packages", "exact W0 packages")
        validation.require(w0.get("contributes_to_gate") == "G0", "stage-w0-gate", "W0 closes G0")
        validation.require(w0.get("completion_receipt") == "W0", "stage-w0-receipt", "W0 receipt")
        validation.require(w0.get("status") == "ACTIVE_PACKAGE_ONLY", "stage-w0-status", "package-only active")
    if w1 is not None:
        validation.require(w1.get("status") == "BLOCKED", "stage-w1-status", "W1 blocked")
        validation.require(strings(w1.get("requires_accepted_gates")) == ("G0",), "stage-w1-gate", "G0 prerequisite")
        validation.require(strings(w1.get("requires_accepted_receipts")) == ("W0",), "stage-w1-receipt", "W0 prerequisite")
        validation.require(strings(w1.get("packages")) == EXPECTED_W1_PACKAGES, "stage-w1-packages", "exact W1 packages")

    validation.require(
        (launch.get("active_stage"), launch.get("active_wave")) == ("P00", 0),
        "launch-current",
        "launch remains P00/W0",
    )
    validation.require(strings(launch.get("authorized_packages")) == ("search-contracts",), "launch-authorized", "contracts only")
    validation.require(
        strings(launch.get("conditional_packages")) == ("search-domain", "search-ports"),
        "launch-conditional",
        "domain and ports only",
    )
    draft_control = launch.get("draft_control")
    validation.require(isinstance(draft_control, dict), "launch-draft-control", "draft-control table exists")
    if isinstance(draft_control, dict):
        expected_counts = {
            "ticket_drafts": 3,
            "context_drafts": 3,
            "materialized_contexts": 0,
            "issued_tickets": 0,
            "active_leases": 0,
            "submissions": 0,
            "accepted_reviews": 0,
            "accepted_package_handoffs": 0,
        }
        for key, expected in expected_counts.items():
            validation.require(draft_control.get(key) == expected, f"launch:{key}", f"{key} = {expected}")
        validation.require(draft_control.get("draft_presence_authorizes") is False, "launch:draft-authority", "drafts do not authorize")
        validation.require(
            draft_control.get("draft_presence_satisfies_conditional_activation") is False,
            "launch:draft-conditional",
            "drafts do not satisfy conditional activation",
        )

    advancement = launch.get("advancement")
    validation.require(isinstance(advancement, dict), "launch-advancement", "advancement table exists")
    if isinstance(advancement, dict):
        validation.require(advancement.get("next_wave") == 1, "launch-next-wave", "next wave is 1")
        validation.require(advancement.get("requires_accepted_gate") == "G0", "launch-advance-gate", "G0 required")
        validation.require(advancement.get("requires_accepted_wave_receipt") == "W0", "launch-advance-receipt", "W0 required")
        validation.require(strings(advancement.get("requires_accepted_packages")) == EXPECTED_PACKAGES, "launch-advance-packages", "three handoffs")
        validation.require(advancement.get("requires_no_active_writer_leases") is True, "launch-no-leases", "no active leases")
        validation.require(advancement.get("requires_zero_unreviewed_submissions") is True, "launch-no-submissions", "no unreviewed submissions")
        validation.require(advancement.get("requires_append_only_package_handoffs") is True, "launch-append-only", "append-only handoffs")

    validation.require(ticket_manifest.get("draft_count") == 3, "ticket-manifest-count", "three ticket drafts")
    validation.require(context_manifest.get("draft_count") == 3, "context-manifest-count", "three context drafts")
    validation.require(orchestration.get("schema_version") == 5, "orchestration-version", "orchestration schema v5")
    validation.require(orchestration.get("consumer_uses_branch_head") is False, "orchestration-no-head", "consumers bind immutable commits")
    validation.require(orchestration.get("consumer_requires_exact_commit_and_api_digest") is True, "orchestration-exact-handoff", "exact commit/API required")

    for name in EXPECTED_PACKAGES:
        package_row = find_table(crates.get("package"), "name", name)
        function_row = find_table(functions.get("foundation"), "package", name)
        validation.require(package_row is not None, f"crates:{name}", "package registry row exists")
        validation.require(function_row is not None, f"functions:{name}", "foundation function row exists")
        if package_row is not None and function_row is not None:
            validation.require(package_row.get("wave") == 0, f"crates:{name}:wave", "wave 0")
            validation.require(function_row.get("wave") == 0, f"functions:{name}:wave", "wave 0")
            validation.require(function_row.get("write_scope") == f"crates/{name}/**", f"functions:{name}:scope", "package-only scope")

        ticket = load_toml(root, f"swarm/ticket-drafts/p00/{name}.toml", validation)
        context = load_toml(root, f"swarm/context-drafts/p00/{name}.toml", validation)
        expected_class = "AUTHORIZED" if name == "search-contracts" else "CONDITIONAL"
        validation.require(ticket.get("schema_version") == 2, f"ticket:{name}:schema", "ticket schema v2")
        validation.require(ticket.get("status") == "DRAFT_ONLY_NOT_ISSUED", f"ticket:{name}:status", "non-issued")
        validation.require(ticket.get("claimable") is False, f"ticket:{name}:claimable", "not claimable")
        validation.require(ticket.get("authorizes_implementation") is False, f"ticket:{name}:authority", "no implementation authority")
        validation.require(ticket.get("creates_lease") is False, f"ticket:{name}:lease", "does not create lease")
        validation.require(ticket.get("launch_class") == expected_class, f"ticket:{name}:class", expected_class)
        validation.require(context.get("schema_version") == 2, f"context:{name}:schema", "context schema v2")
        validation.require(context.get("status") == "UNMATERIALIZED_DRAFT", f"context:{name}:status", "unmaterialized")
        validation.require(context.get("claimable") is False, f"context:{name}:claimable", "not claimable")
        expected_slots = () if name == "search-contracts" else ("search-contracts::accepted_package_and_api_handoff",)
        content = context.get("content")
        validation.require(isinstance(content, dict), f"context:{name}:content", "content table exists")
        if isinstance(content, dict):
            validation.require(strings(content.get("accepted_handoff_slots")) == expected_slots, f"context:{name}:slots", "exact handoff slots")


def validate_zero_state(root: Path, validation: Validation) -> None:
    for relative in PROTECTED_ROOTS:
        path = root / relative
        validation.require(path.is_dir(), f"zero:{relative}:dir", "protected root exists")
        if not path.is_dir():
            continue
        unexpected = sorted(
            child.relative_to(root).as_posix()
            for child in path.rglob("*")
            if child.is_file() and child.name not in {"README.md", ".gitkeep"}
        )
        validation.require(not unexpected, f"zero:{relative}", "no issued record" if not unexpected else f"unexpected: {unexpected[0]}")


def validate_docs_and_workflows(root: Path, validation: Validation) -> None:
    matrix_path = root / "docs/handoff/P00_FOUNDATION_ACCEPTANCE_MATRIX.md"
    readme_path = root / "docs/handoff/README.md"
    tools_readme_path = root / "tools/README.md"
    matrix = matrix_path.read_text(encoding="utf-8") if matrix_path.is_file() else ""
    handoff_readme = readme_path.read_text(encoding="utf-8") if readme_path.is_file() else ""
    tools_readme = tools_readme_path.read_text(encoding="utf-8") if tools_readme_path.is_file() else ""
    for token in (
        "package handoff published",
        "accepted G0 receipt",
        "accepted W0 receipt",
        "P00-A",
        "P00-B",
        "P00-C",
        "P00-D",
        "search-domain",
        "search-ports",
        "W1 unlock matrix",
        "UNAVAILABLE",
        "does not unlock W1",
    ):
        validation.require(token in matrix, f"matrix-token:{token}", f"matrix contains {token}")
    validation.require("P00_FOUNDATION_ACCEPTANCE_MATRIX.md" in handoff_readme, "handoff-navigation", "handoff README links matrix")
    validation.require("validate-p00-foundation-acceptance" in tools_readme, "tool-navigation", "tools README links validator")

    workflow_dir = root / ".github/workflows"
    files = sorted([*workflow_dir.glob("*.yml"), *workflow_dir.glob("*.yaml")])
    validation.require(bool(files), "workflow-presence", "workflow files exist")
    for path in files:
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(root).as_posix()
        manual = re.search(r"^\s{2}workflow_dispatch:\s*$", text, re.MULTILINE) is not None
        no_auto = FORBIDDEN_WORKFLOW_TRIGGER.search(text) is None
        read_only = re.search(r"^\s{2}contents:\s*read\s*$", text, re.MULTILINE) is not None
        no_credentials = "persist-credentials: false" in text
        validation.require(manual and no_auto and read_only and no_credentials, f"workflow:{relative}", "manual/read-only/credential-free")


def build_report(root: Path) -> dict[str, Any]:
    validation = Validation()
    registry = validate_registry(root, validation)
    validate_cross_registries(root, registry, validation)
    validate_zero_state(root, validation)
    validate_docs_and_workflows(root, validation)
    return {
        "schema_version": 1,
        "validator": "p00_foundation_acceptance_v1",
        "status": "PASS" if not validation.errors else "FAIL",
        "non_authoritative": True,
        "package_acceptance_claimed": False,
        "g0_acceptance_claimed": False,
        "w0_acceptance_claimed": False,
        "w1_authority_claimed": False,
        "checks": validation.checks,
        "errors": validation.errors,
    }


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--json", action="store_true", help="emit canonical JSON report")
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    root = Path(args.root).resolve()
    if not root.is_dir():
        print(f"repository root does not exist: {root}", file=sys.stderr)
        return 2
    report = build_report(root)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    elif report["status"] == "PASS":
        print(f"PASS: {len(report['checks'])} structural checks; no acceptance or launch authority created")
    else:
        print(f"FAIL: {len(report['errors'])} error(s)", file=sys.stderr)
        for error in report["errors"]:
            print(f"- {error}", file=sys.stderr)
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
