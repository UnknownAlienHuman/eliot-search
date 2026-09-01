#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FOUNDATION = {"search-contracts", "search-domain", "search-ports"}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def rows(document: dict[str, Any], key: str, name_key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(name_key), str):
            raise ValueError(f"invalid {key} row")
        name = row[name_key]
        if name in result:
            raise ValueError(f"duplicate {key} row {name}")
        result[name] = row
    return result


def require(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def operation_names(text: str) -> set[str]:
    names: set[str] = set()
    names.update(re.findall(r"^#{2,3}\s+`([a-z][a-z0-9_]*)\b", text, flags=re.MULTILINE))
    names.update(re.findall(r"`([a-z][a-z0-9_]*)\([^`]*\)`", text))
    for block in re.findall(r"```[^\n]*\n(.*?)```", text, flags=re.DOTALL):
        names.update(
            re.findall(
                r"^(?:pub\s+)?(?:async\s+)?(?:fn\s+)?([a-z][a-z0-9_]*)\s*\(",
                block,
                flags=re.MULTILINE,
            )
        )
    return names - {"if", "for", "while", "match", "loop", "return"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.parse_args()

    errors: list[str] = []
    try:
        manifest = load("swarm/coverage/manifest.toml")
        packages = rows(load("swarm/crates.toml"), "package", "name")
        functions_doc = load("swarm/function-packets.toml")
        foundations = rows(functions_doc, "foundation", "package")
        functions = rows(functions_doc, "package", "name")
        modules_doc = load("swarm/module-packets.toml")
        operations = load("swarm/coverage/operations.toml")
        tasks = load("swarm/coverage/tasks.toml")
        delivery = rows(load("swarm/coverage/delivery-slices.toml"), "slice", "id")
        cases_doc = load("qualification/architecture-coverage/cases-v1.toml")
        launch = load("swarm/launch-state.toml")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    package_names = set(packages)
    require(errors, package_names and len(package_names) == 45, "package set must contain 45 packages")

    # Operation registry contract.
    require(errors, operations.get("status") == "SOURCE_DERIVED_OPERATION_OWNERSHIP_CLOSED_NOT_IMPLEMENTED", "operation registry status changed")
    require(errors, operations.get("function_registry") == "swarm/function-packets.toml", "operation registry function source mismatch")
    require(errors, operations.get("module_registry") == "swarm/module-packets.toml", "operation registry module source mismatch")
    require(errors, operations.get("package_count") == 45, "operation registry package_count mismatch")
    require(errors, operations.get("foundation_package_count") == 3, "operation registry foundation count mismatch")
    require(errors, operations.get("package_function_source_count") == 42, "operation registry function-source count mismatch")
    require(errors, operations.get("operation_count") == "DERIVED_BY_VALIDATOR", "operation count must remain source-derived")
    require(errors, operations.get("implementation_authorized_by_this_registry") is False, "operation registry authorizes implementation")

    identity = operations.get("identity", {})
    require(errors, identity.get("format") == "<package>::<operation>", "qualified operation identity format changed")
    require(errors, identity.get("package_qualified_identity_required") is True, "package-qualified operation IDs disabled")
    require(errors, identity.get("duplicate_unqualified_operation_names_allowed") is True, "unqualified operation collision rule changed")
    require(errors, identity.get("duplicate_package_qualified_operation_names_allowed") is False, "qualified operation collisions allowed")

    extraction = operations.get("extraction", {})
    require(errors, extraction.get("minimum_operations_per_nonfoundation_package") == 1, "minimum operation count weakened")
    require(errors, extraction.get("ignore_test_fixture_and_example_blocks") is True, "fixture/example exclusion disabled")
    require(errors, extraction.get("ignore_Rust_trait_methods_in_package_FUNCTIONS") is False, "trait methods excluded from ownership")

    ownership = operations.get("ownership", {})
    require(errors, ownership.get("operation_owner_source") == "function_registry_package", "operation owner source changed")
    require(errors, ownership.get("public_entry_module_source") == "module_registry_package", "public entry source changed")
    for key in (
        "every_operation_enters_through_public_entry_module",
        "internal_delegation_must_remain_within_declared_package_modules",
        "operation_source_must_be_package_local",
    ):
        require(errors, ownership.get(key) is True, f"operation ownership invariant disabled: {key}")
    require(errors, ownership.get("cross_package_operation_implementation_allowed") is False, "cross-package operation implementation allowed")

    operation_invariants = operations.get("invariants", {})
    for key in (
        "missing_function_source_blocks_merge",
        "function_source_without_registered_package_blocks_merge",
        "registered_package_without_discovered_operation_blocks_merge",
        "operation_without_declared_public_entry_blocks_merge",
        "operation_implementation_outside_package_write_scope_blocks_merge",
    ):
        require(errors, operation_invariants.get(key) is True, f"operation merge guard disabled: {key}")
    require(errors, operation_invariants.get("placeholder_success_allowed") is False, "placeholder success allowed")

    expected_foundation_sources = {
        "search-contracts": ("docs/contracts/p00/README.md", "lib"),
        "search-domain": ("docs/contracts/p00/SUPPORT_SCHEMAS.md", "lib"),
        "search-ports": ("docs/contracts/p00/PORT_OPERATIONS.md", "lib"),
    }
    require(errors, set(foundations) == FOUNDATION, "foundation function registry mismatch")
    for package, (source, entry) in expected_foundation_sources.items():
        registry_row = foundations.get(package, {})
        operation_row = operations.get("foundation", {}).get(package.replace("-", "_"), {})
        require(errors, registry_row.get("primary_contract") == source, f"{package}: foundation contract source mismatch")
        require(errors, operation_row.get("package") == package, f"{package}: operation foundation package mismatch")
        require(errors, operation_row.get("source") == source, f"{package}: operation foundation source mismatch")
        require(errors, operation_row.get("public_entry_module") == entry, f"{package}: operation public entry mismatch")

    require(errors, set(functions) == package_names - FOUNDATION, "non-foundation function package set mismatch")
    qualified_operations: set[str] = set()
    source_operation_count = 0
    for package, row in functions.items():
        path = row.get("functions")
        require(errors, isinstance(path, str) and (ROOT / path).is_file(), f"{package}: function source missing")
        require(errors, isinstance(path, str) and path.startswith(packages[package]["path"] + "/"), f"{package}: function source outside package")
        if not isinstance(path, str) or not (ROOT / path).is_file():
            continue
        names = operation_names((ROOT / path).read_text(encoding="utf-8"))
        require(errors, len(names) >= 1, f"{package}: no source-derived operations")
        for name in names:
            qualified = f"{package}::{name}"
            require(errors, qualified not in qualified_operations, f"duplicate qualified operation {qualified}")
            qualified_operations.add(qualified)
        source_operation_count += len(names)

    # Task registry contract.
    require(errors, tasks.get("status") == "STATIC_TASK_OWNERSHIP_CLOSED_NOT_IMPLEMENTED", "task registry status changed")
    require(errors, tasks.get("package_registry") == "swarm/crates.toml", "task package registry mismatch")
    require(errors, tasks.get("delivery_registry") == "swarm/coverage/delivery-slices.toml", "task delivery registry mismatch")
    require(errors, tasks.get("package_assignment_task_count") == 45, "task assignment count mismatch")
    require(errors, tasks.get("delivery_slice_task_count") == 19, "task delivery count mismatch")
    require(errors, tasks.get("implementation_authorized_by_this_registry") is False, "task registry authorizes implementation")

    package_tasks = tasks.get("package_assignment_tasks", {})
    require(errors, package_tasks.get("source") == "swarm/crates.toml::package.assignment", "package task source mismatch")
    for key in (
        "one_assignment_per_package",
        "assignment_file_required",
        "assignment_must_name_owned_state_or_behavior",
        "assignment_must_name_forbidden_or_non_owned_behavior",
        "assignment_must_name_exact_package_write_scope",
    ):
        require(errors, package_tasks.get(key) is True, f"package task invariant disabled: {key}")
    require(errors, package_tasks.get("assignment_must_not_override_dependency_or_function_registry") is True, "assignment override guard disabled")

    assignment_paths: set[str] = set()
    for package, row in packages.items():
        path = row.get("assignment")
        require(errors, isinstance(path, str) and (ROOT / path).is_file(), f"{package}: assignment file missing")
        if not isinstance(path, str) or not (ROOT / path).is_file():
            continue
        require(errors, path not in assignment_paths, f"duplicate assignment path {path}")
        assignment_paths.add(path)
        text = (ROOT / path).read_text(encoding="utf-8")
        require(errors, package in text, f"{package}: assignment does not identify package")
        require(errors, "Own" in text or "Ownership" in text or "Mission" in text, f"{package}: assignment lacks owned behavior")
        require(errors, "Forbidden" in text or "Do not" in text or "Never" in text, f"{package}: assignment lacks non-owned behavior")

    delivery_tasks = tasks.get("delivery_tasks", {})
    require(errors, delivery_tasks.get("source") == "docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md#H17", "delivery task source mismatch")
    for key in (
        "one_registry_entry_per_delivery_slice",
        "primary_package_set_required",
        "module_set_required",
        "required_outputs_required",
        "exit_evidence_required",
    ):
        require(errors, delivery_tasks.get(key) is True, f"delivery task invariant disabled: {key}")
    require(errors, set(delivery) == {f"P{i:02d}" for i in range(19)}, "delivery task set must be P00-P18")
    for slice_id, row in delivery.items():
        require(errors, isinstance(row.get("primary_packages"), list) and len(row["primary_packages"]) > 0, f"{slice_id}: primary package set missing")
        require(errors, isinstance(row.get("modules"), list) and len(row["modules"]) > 0, f"{slice_id}: module set missing")
        require(errors, isinstance(row.get("required_outputs"), list) and len(row["required_outputs"]) > 0, f"{slice_id}: required outputs missing")
        require(errors, isinstance(row.get("exit_evidence"), list) and len(row["exit_evidence"]) > 0, f"{slice_id}: exit evidence missing")

    qualification_tasks = tasks.get("qualification_tasks", {})
    roots = qualification_tasks.get("source_roots")
    require(errors, roots == ["qualification", "tests"], "qualification task roots mismatch")
    for root in roots if isinstance(roots, list) else []:
        require(errors, (ROOT / root).is_dir(), f"qualification task root missing: {root}")
    for key in (
        "qualification_is_evidence_requirement_not_success",
        "unavailable_or_unexecuted_state_must_not_authorize",
        "package_writer_may_not_accept_own_evidence",
    ):
        require(errors, qualification_tasks.get(key) is True, f"qualification task invariant disabled: {key}")

    task_invariants = tasks.get("invariants", {})
    for key in (
        "package_without_assignment_blocks_merge",
        "orphan_assignment_file_blocks_merge",
        "delivery_slice_without_owner_blocks_merge",
        "delivery_slice_without_exit_evidence_blocks_merge",
        "package_absent_from_all_delivery_slices_blocks_merge",
    ):
        require(errors, task_invariants.get(key) is True, f"task merge guard disabled: {key}")
    require(errors, task_invariants.get("assignment_or_delivery_presence_authorizes_implementation") is False, "task presence authorizes implementation")

    # Qualification inventory and manifest links.
    case_rows = cases_doc.get("case")
    require(errors, cases_doc.get("schema_version") == 1, "coverage case schema mismatch")
    require(errors, cases_doc.get("suite") == "architecture_coverage_closure_v1", "coverage case suite mismatch")
    require(errors, cases_doc.get("status") == "STRUCTURAL_NOT_EXECUTED", "coverage case status changed")
    require(errors, cases_doc.get("case_count") == 40, "coverage case_count must be 40")
    require(errors, isinstance(case_rows, list) and len(case_rows) == 40, "coverage case inventory must contain 40 rows")
    if isinstance(case_rows, list):
        ids = [row.get("id") for row in case_rows if isinstance(row, dict)]
        require(errors, len(ids) == len(set(ids)) == 40, "coverage case IDs must be 40 unique values")
        for row in case_rows:
            require(errors, isinstance(row, dict) and row.get("mandatory") is True, "coverage case must be mandatory")
            require(errors, isinstance(row, dict) and row.get("result") == "UNAVAILABLE", "coverage case must remain UNAVAILABLE")

    require(errors, manifest.get("operation_registry") == "swarm/coverage/operations.toml", "coverage manifest operation registry link mismatch")
    require(errors, manifest.get("task_registry") == "swarm/coverage/tasks.toml", "coverage manifest task registry link mismatch")
    require(errors, manifest.get("package_assignment_task_count") == len(assignment_paths) == 45, "coverage manifest assignment count mismatch")
    require(errors, manifest.get("delivery_slice_count") == len(delivery) == 19, "coverage manifest delivery task count mismatch")
    require(errors, modules_doc.get("package_count") == 45 and modules_doc.get("module_count") == 479, "module registry summary mismatch")
    require(errors, launch.get("active_stage") == "P00" and launch.get("active_wave") == 0, "launch authority moved from P00/W0")
    require(errors, launch.get("authorized_packages") == ["search-contracts"], "authorized package set changed")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(package_names),
        "assignment_tasks": len(assignment_paths),
        "delivery_tasks": len(delivery),
        "package_function_sources": len(functions),
        "derived_package_qualified_operations": source_operation_count,
        "qualification_cases": len(case_rows) if isinstance(case_rows, list) else 0,
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
