#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


def load(relative: str) -> dict[str, Any]:
    with (ROOT / relative).open("rb") as handle:
        return tomllib.load(handle)


def index_rows(document: dict[str, Any], key: str, identity: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(identity), str):
            raise ValueError(f"invalid {key} row")
        row_id = row[identity]
        if row_id in result:
            raise ValueError(f"duplicate {key} identity: {row_id}")
        result[row_id] = row
    return result


def fail(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate the non-authoritative implementation program")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    errors: list[str] = []
    warnings: list[str] = []

    try:
        program = load("swarm/implementation-program.toml")
        launch = load("swarm/launch-state.toml")
        stages_doc = load("swarm/stages.toml")
        gates_doc = load("swarm/gates.toml")
        packages_doc = load("swarm/crates.toml")
        metrics = load("qualification/product-pulse/metrics.toml")
        coverage = load("swarm/coverage/manifest.toml")
        cases = load("qualification/implementation-program/cases-v1.toml")

        program_stages = index_rows(program, "stage", "id")
        stages = index_rows(stages_doc, "stage", "id")
        gates = index_rows(gates_doc, "gate", "id")
        packages = index_rows(packages_doc, "package", "name")
        targets = index_rows(program, "target", "id")
        integration_steps = index_rows(program, "integration_step", "id")
        next_steps = index_rows(program, "next_step", "id")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError, KeyError, TypeError) as exc:
        result = {"status": "FAIL", "errors": [f"{type(exc).__name__}: {exc}"]}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 1

    expected_paths = {
        "launch_authority": "swarm/launch-state.toml",
        "package_registry": "swarm/crates.toml",
        "function_registry": "swarm/function-packets.toml",
        "module_registry": "swarm/module-packets.toml",
        "package_map_index": "swarm/coverage/package-map-index.toml",
        "stage_registry": "swarm/stages.toml",
        "stage_readset_registry": "swarm/stage-readsets.toml",
        "gate_registry": "swarm/gates.toml",
        "configuration_registry": "config/sections.toml",
    }
    for key, expected in expected_paths.items():
        fail(errors, program.get(key) == expected, f"program path mismatch: {key}")
        fail(errors, (ROOT / expected).is_file(), f"program path missing: {expected}")

    fail(errors, program.get("schema_version") == 1, "implementation program schema version mismatch")
    fail(errors, program.get("status") == "PLANNED_NOT_AUTHORIZED", "implementation program status is not non-authoritative")
    fail(errors, re.fullmatch(r"[0-9a-f]{40}", str(program.get("source_main_commit", ""))) is not None, "source main commit is not an exact SHA")
    for key in (
        "implementation_authorized_by_this_program",
        "launch_state_changed",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ):
        fail(errors, program.get(key) is False, f"authority/non-claim flag changed: {key}")

    discipline = program.get("discipline", {})
    for key in (
        "one_writer_one_package",
        "one_worktree_one_task",
        "package_write_scope_only",
        "accepted_public_handoffs_only",
    ):
        fail(errors, discipline.get(key) is True, f"discipline invariant disabled: {key}")
    for key in (
        "dependency_implementation_reads_allowed",
        "package_writer_may_edit_shared_registries",
        "package_writer_may_self_review",
        "package_writer_may_advance_launch_state",
    ):
        fail(errors, discipline.get(key) is False, f"discipline prohibition disabled: {key}")
    fail(errors, discipline.get("ordinary_architecture_master_access") == "exception-only", "architecture access policy mismatch")
    fail(errors, discipline.get("maximum_static_context_files") == 16, "static context ceiling mismatch")
    fail(errors, discipline.get("normal_handwritten_src_target") == 7500, "normal source target mismatch")
    fail(errors, discipline.get("mandatory_split_review_lines") == 8500, "split-review line threshold mismatch")
    fail(errors, discipline.get("hard_handwritten_line_limit") == packages_doc.get("hard_handwritten_rust_line_limit") == 10000, "hard line limit mismatch")

    current = program.get("current_state", {})
    draft = launch.get("draft_control", {})
    coverage_state = coverage.get("current_state", {})
    fail(errors, current.get("active_stage") == launch.get("active_stage") == "P00", "current active stage mismatch")
    fail(errors, current.get("active_wave") == launch.get("active_wave") == 0, "current active wave mismatch")
    fail(errors, current.get("authorized_packages") == launch.get("authorized_packages") == ["search-contracts"], "authorized package mismatch")
    fail(errors, current.get("conditional_packages") == launch.get("conditional_packages") == ["search-domain", "search-ports"], "conditional package mismatch")
    fail(errors, current.get("implemented_packages") == coverage_state.get("implemented_packages") == 0, "implemented package count mismatch")
    fail(errors, current.get("materialized_writer_contexts") == draft.get("materialized_contexts") == 0, "materialized context count mismatch")
    fail(errors, current.get("issued_implementation_tickets") == draft.get("issued_tickets") == 0, "issued ticket count mismatch")
    fail(errors, current.get("active_writer_leases") == draft.get("active_leases") == 0, "active lease count mismatch")
    fail(errors, current.get("accepted_package_handoffs") == draft.get("accepted_package_handoffs") == 0, "accepted handoff count mismatch")
    fail(errors, current.get("accepted_gate_receipts") == coverage_state.get("accepted_gates") == 0, "accepted gate count mismatch")
    fail(errors, current.get("accepted_wave_receipts") == coverage_state.get("accepted_wave_receipts") == 0, "accepted wave count mismatch")
    fail(errors, current.get("cargo_lock_present") == (ROOT / "Cargo.lock").is_file(), "Cargo.lock presence is reported incorrectly")
    for key in (
        "windows_toolchain_selected",
        "qdrant_profile_selected",
        "rust_parser_profile_selected",
        "product_pulse_accepted",
        "optional_depth_selected",
    ):
        fail(errors, current.get(key) is False, f"current unselected state changed: {key}")

    expected_stage_ids = [f"W{number}" for number in range(11)]
    fail(errors, list(stages) == expected_stage_ids, "central stage order is not W0-W10")
    fail(errors, list(program_stages) == expected_stage_ids, "program stage order is not W0-W10")
    fail(errors, set(program_stages) == set(stages), "program/central stage set mismatch")

    covered_packages: set[str] = set()
    for stage_id in expected_stage_ids:
        source = stages.get(stage_id, {})
        row = program_stages.get(stage_id, {})
        fail(errors, row.get("name") == source.get("name"), f"{stage_id}: name mismatch")
        fail(errors, row.get("required_gates") == source.get("requires_accepted_gates"), f"{stage_id}: gate prerequisite mismatch")
        fail(errors, row.get("required_receipts") == source.get("requires_accepted_receipts"), f"{stage_id}: receipt prerequisite mismatch")
        fail(errors, row.get("packages") == source.get("packages"), f"{stage_id}: package set/order mismatch")
        completion = source.get("completion_receipt")
        expected_closes = [completion] if isinstance(completion, str) and completion else []
        if source.get("closes_gate") is True and source.get("contributes_to_gate"):
            expected_closes.append(source["contributes_to_gate"])
        fail(errors, row.get("closes") == expected_closes, f"{stage_id}: completion/gate closure mismatch")
        for package in row.get("packages", []):
            fail(errors, package in packages, f"{stage_id}: unknown package {package}")
            covered_packages.add(package)
        fail(errors, isinstance(row.get("required_product_result"), str) and bool(row["required_product_result"].strip()), f"{stage_id}: required product result missing")

    fail(errors, covered_packages == set(packages), f"program package closure mismatch: {sorted(set(packages) ^ covered_packages)}")
    fail(errors, len(stages_doc.get("stage", [])) == stages_doc.get("stage_count") == 11, "central stage count mismatch")
    fail(errors, set(gates) == {f"G{number}" for number in range(7)}, "gate registry is not G0-G6")

    boundary = program.get("release_boundary", {})
    fail(errors, boundary.get("first_bootable_stage") == "W1", "bootable stage mismatch")
    fail(errors, boundary.get("first_direct_source_stage") == "W2", "DIRECT stage mismatch")
    fail(errors, boundary.get("first_useful_search_stage") == "W4", "useful baseline stage mismatch")
    fail(errors, boundary.get("release_candidate_stage") == "W9", "release-candidate stage mismatch")
    fail(errors, boundary.get("optional_depth_stage") == "W10", "optional-depth stage mismatch")
    fail(errors, boundary.get("baseline_release_requires") == ["G0", "G1", "G2", "G3", "W7_LIFECYCLE", "G4", "G5"], "baseline release gate/receipt sequence mismatch")
    fail(errors, boundary.get("baseline_release_requires_g6") is False, "G6 became a baseline requirement")
    fail(errors, boundary.get("baseline_release_requires_w10") is False, "W10 became a baseline requirement")

    expected_targets = {
        "buildable_workspace": "W0",
        "bootable_service_shell": "W1",
        "direct_source_product": "W2",
        "useful_baseline_search": "W4",
        "release_candidate": "W9",
        "optional_depth": "W10",
    }
    fail(errors, set(targets) == set(expected_targets), "target state set mismatch")
    for target_id, stage_id in expected_targets.items():
        row = targets.get(target_id, {})
        fail(errors, row.get("required_stage") == stage_id, f"{target_id}: required stage mismatch")
        fail(errors, isinstance(row.get("claim"), str) and bool(row["claim"].strip()), f"{target_id}: claim missing")
    fail(errors, targets.get("optional_depth", {}).get("baseline_release_dependency") is False, "optional depth became a baseline dependency")

    target_slo = program.get("architecture_targets", {})
    metric_slo = metrics.get("candidate_slo", {})
    percentiles = metrics.get("percentiles", {})
    for key in (
        "warm_exact_keyword_navigation_p95_ms",
        "warm_single_scope_lexical_p95_ms",
        "warm_cross_repository_comparison_p95_ms",
        "first_useful_progressive_card_ms",
    ):
        fail(errors, target_slo.get(key) == metric_slo.get(key), f"architecture target mismatch: {key}")
    fail(errors, target_slo.get("minimum_percentile_samples") == percentiles.get("minimum_measured_samples") == 30, "percentile sample floor mismatch")
    fail(errors, target_slo.get("status") == "TARGET_NOT_MEASURED", "performance targets are overclaimed")

    blockers = program.get("release_hard_blockers", {})
    for key in (
        "false_complete_negative_claim_count",
        "stale_leakage_count",
        "access_leakage_count",
        "secret_or_content_leakage_count",
        "protocol_resource_leak_count",
    ):
        fail(errors, blockers.get(key) == 0, f"release hard blocker is not zero: {key}")
    fail(errors, blockers.get("required_fault_cell_recovery_rate") == 1.0, "fault recovery acceptance is not 100%")
    for key in (
        "all_mandatory_package_handoffs_required",
        "all_active_leases_closed",
        "all_unreviewed_submissions_closed",
        "windows_install_upgrade_recovery_rollback_uninstall_required",
        "independent_g5_review_required",
    ):
        fail(errors, blockers.get(key) is True, f"release prerequisite disabled: {key}")

    expected_integration = [
        "pin_windows_toolchain",
        "lock_dependency_graph",
        "freeze_build_profiles",
        "establish_test_harness",
        "freeze_artifact_and_data_layout",
    ]
    ordered_integration = [row["id"] for row in sorted(program.get("integration_step", []), key=lambda item: item.get("order", -1))]
    fail(errors, ordered_integration == expected_integration, "integration bootstrap order mismatch")
    fail(errors, list(range(1, 6)) == sorted(row.get("order") for row in integration_steps.values()), "integration step ordinals mismatch")

    expected_next = [
        "integration_bootstrap_pr",
        "search_contracts_implementation",
        "search_contracts_review_and_handoff",
        "search_domain_implementation",
        "search_ports_implementation",
        "w0_g0_evidence_and_acceptance",
        "advance_launch_to_w1",
    ]
    ordered_next = [row["id"] for row in sorted(program.get("next_step", []), key=lambda item: item.get("order", -1))]
    fail(errors, ordered_next == expected_next, "first implementation sequence mismatch")
    fail(errors, list(range(1, 8)) == sorted(row.get("order") for row in next_steps.values()), "next-step ordinals mismatch")

    fail(errors, coverage.get("implementation_program") == "swarm/implementation-program.toml", "coverage manifest does not link the implementation program")

    for section in ("error_model", "resources", "security", "persistence", "observability", "packaging", "testing"):
        row = program.get("cross_cutting", {}).get(section, {})
        fail(errors, isinstance(row.get("required"), list) and bool(row["required"]), f"cross-cutting requirement set missing: {section}")
    fail(errors, program.get("cross_cutting", {}).get("testing", {}).get("compile_or_unit_tests_alone_pass_gate") is False, "compile/unit tests can incorrectly pass a gate")

    case_rows = cases.get("case")
    fail(errors, cases.get("status") == "STRUCTURAL_NOT_EXECUTED", "qualification case status mismatch")
    fail(errors, cases.get("case_count") == 24, "qualification case count mismatch")
    fail(errors, isinstance(case_rows, list) and len(case_rows) == 24, "qualification case inventory mismatch")
    if isinstance(case_rows, list):
        fail(errors, len({row.get("id") for row in case_rows}) == 24, "qualification case IDs are not unique")
        fail(errors, all(row.get("mandatory") is True and row.get("result") == "UNAVAILABLE" for row in case_rows), "qualification cases contain premature evidence")

    workflow_path = ROOT / ".github/workflows/implementation-program.yml"
    fail(errors, workflow_path.is_file(), "implementation program workflow missing")
    if workflow_path.is_file():
        workflow = workflow_path.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false", "validate-implementation-program.py"):
            fail(errors, token in workflow, f"implementation workflow missing token: {token}")
        for trigger in (
            "\n  push:",
            "\n  pull_request:",
            "\n  pull_request_target:",
            "\n  merge_group:",
            "\n  schedule:",
            "\n  workflow_run:",
            "\n  repository_dispatch:",
            "\n  workflow_call:",
        ):
            fail(errors, trigger not in workflow, f"automatic workflow trigger present: {trigger.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "stages": len(program_stages),
        "packages": len(packages),
        "targets": len(targets),
        "integration_steps": len(integration_steps),
        "next_steps": len(next_steps),
        "baseline_release_requirements": len(boundary.get("baseline_release_requires", [])),
        "current_stage": current.get("active_stage"),
        "current_wave": current.get("active_wave"),
        "cargo_lock_present": (ROOT / "Cargo.lock").is_file(),
        "warnings": warnings,
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
