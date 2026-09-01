from __future__ import annotations

import re
from typing import Any

from .common import ROOT, load, read, require, rows, validate_module_ref


def _manual_workflow(errors: list[str], path: str) -> None:
    require(errors, (ROOT / path).is_file(), f"missing workflow {path}")
    if not (ROOT / path).is_file():
        return
    text = read(path)
    for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
        require(errors, token in text, f"{path}: missing {token}")
    for forbidden in (
        "\n  push:",
        "\n  pull_request:",
        "\n  pull_request_target:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
        "\n  workflow_call:",
    ):
        require(errors, forbidden not in text, f"{path}: automatic trigger {forbidden.strip()}")


def validate_control(
    errors: list[str],
    topology: dict[str, Any],
    schemas: dict[str, Any],
) -> dict[str, Any]:
    manifest = topology["manifest"]
    package_rows = topology["package_rows"]
    packages = topology["packages"]
    modules = topology["modules"]

    task_doc = load(manifest["task_registry"])
    operation_doc = load(manifest["operation_registry"])
    delivery_doc = load(manifest["delivery_slice_registry"])
    qualification_doc = load("qualification/architecture-coverage/cases-v1.toml")
    launch = load(manifest["launch_authority"])
    package_doc = load(manifest["package_registry"])
    p00_manifest = load(manifest["p00_contract_manifest"])

    require(errors, manifest.get("schema_version") == 1, "coverage manifest schema version mismatch")
    require(errors, manifest.get("status") == "STATIC_COVERAGE_CLOSED_NOT_IMPLEMENTED", "coverage status mismatch")
    require(errors, manifest.get("implementation_authorized") is False, "coverage manifest authorizes implementation")
    require(errors, manifest.get("runtime_evidence_available") is False, "coverage manifest claims runtime evidence")
    require(errors, manifest.get("package_acceptance_claimed") is False, "coverage manifest claims package acceptance")
    require(errors, manifest.get("gate_or_wave_acceptance_claimed") is False, "coverage manifest claims gate/wave acceptance")
    require(errors, manifest.get("launch_state_changed") is False, "coverage manifest claims launch-state change")

    count_pairs = {
        "package_count": len(package_rows),
        "package_function_packet_count": len(topology["function_rows"]),
        "foundation_contract_package_count": len(topology["foundation_rows"]),
        "module_packet_count": len(topology["module_rows"]),
        "architecture_section_count": topology["section_count"],
        "capability_cell_count": topology["capability_count"],
        "invariant_count": topology["invariant_count"],
        "shared_port_count": topology["port_count"],
        "p00_schema_or_registry_count": schemas["schema_total"],
        "p00_type_registry_symbol_count": schemas["type_registry_symbols"],
        "p00_named_type_completion_count": schemas["completion_symbols"],
        "p00_recipe_body_count": schemas["recipe_count"],
        "reason_code_count": schemas["reason_count"],
        "configuration_section_count": topology["config_count"],
    }
    for key, actual in count_pairs.items():
        require(errors, manifest.get(key) == actual, f"coverage manifest {key} mismatch: {manifest.get(key)} != {actual}")
    require(
        errors,
        manifest.get("p00_support_and_record_schema_count")
        == schemas["schema_total"] - schemas["type_registry_symbols"] - schemas["completion_symbols"],
        "coverage manifest support/record schema count mismatch",
    )

    architecture_digest = manifest.get("architecture_section_sha256")
    require(errors, architecture_digest == p00_manifest.get("architecture_sha256"), "architecture digest differs from P00 manifest")
    require(errors, architecture_digest == package_doc.get("architecture_sha256"), "architecture digest differs from package registry")
    require(errors, architecture_digest == launch.get("architecture_sha256"), "architecture digest differs from launch state")
    require(errors, manifest.get("base_commit") == "73ab0c415960ec1322f4b367a2325ce7916301b0", "coverage audit base commit changed")

    require(errors, launch.get("active_stage") == "P00", "launch stage moved from P00")
    require(errors, launch.get("active_wave") == 0, "launch wave moved from W0")
    require(errors, launch.get("authorized_packages") == ["search-contracts"], "authorized package set changed")

    require(errors, task_doc.get("status") == "STATIC_TASK_OWNERSHIP_NOT_EXECUTED", "task registry status mismatch")
    require(errors, task_doc.get("package_assignment_task_count") == 45, "task registry package count mismatch")
    require(errors, task_doc.get("delivery_slice_task_count") == 19, "task registry delivery count mismatch")
    require(errors, task_doc.get("one_assignment_per_package") is True, "task registry one-assignment rule disabled")
    require(errors, task_doc.get("orphan_assignment_blocks_merge") is True, "task registry orphan guard disabled")
    require(errors, task_doc.get("assignment_may_change_package_dependencies") is False, "task registry permits dependency changes")
    require(errors, task_doc.get("assignment_may_authorize_implementation") is False, "task registry authorizes implementation")
    require(errors, task_doc.get("delivery_slice_may_accept_itself") is False, "delivery slice can self-accept")
    require(errors, task_doc.get("gate_or_wave_acceptance_created") is False, "task registry creates gate/wave acceptance")
    require(errors, task_doc.get("launch_state_changed") is False, "task registry changes launch state")

    assignment_paths = {
        row.get("assignment")
        for row in package_rows.values()
        if isinstance(row.get("assignment"), str)
    }
    require(errors, len(assignment_paths) == 45, "package assignments must be unique for all 45 packages")
    actual_assignments = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "swarm/assignments").glob("*.md")
        if path.name != "README.md"
    }
    require(errors, actual_assignments == assignment_paths, f"assignment task closure mismatch: {sorted(actual_assignments ^ assignment_paths)}")

    delivery_rows = rows(delivery_doc, "slice", "id")
    source_delivery_ids = {
        match.group(1)
        for match in re.finditer(
            r"^###\s+(P\d{2})\s+[—-]\s+.+$",
            read(manifest["architecture_master"]),
            flags=re.MULTILINE,
        )
    }
    require(errors, source_delivery_ids == {f"P{i:02d}" for i in range(19)}, "architecture source must contain P00-P18")
    require(errors, set(delivery_rows) == source_delivery_ids, "delivery-slice registry mismatch")
    require(errors, manifest.get("delivery_slice_count") == len(delivery_rows), "coverage manifest delivery-slice count mismatch")

    packages_with_delivery: set[str] = set()
    for slice_id, row in delivery_rows.items():
        owners = row.get("primary_packages")
        refs = row.get("modules")
        require(errors, isinstance(owners, list) and len(owners) > 0, f"{slice_id}: primary packages missing")
        require(errors, isinstance(refs, list) and len(refs) > 0, f"{slice_id}: module refs missing")
        for package in owners if isinstance(owners, list) else []:
            require(errors, package in packages, f"{slice_id}: unknown package {package}")
            packages_with_delivery.add(package)
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, slice_id)
        qualification_roots = row.get("qualification_roots", [])
        require(errors, isinstance(qualification_roots, list), f"{slice_id}: qualification_roots must be an array")
        for root in qualification_roots if isinstance(qualification_roots, list) else []:
            require(errors, isinstance(root, str) and (ROOT / root).exists(), f"{slice_id}: missing qualification root {root}")
    require(errors, packages_with_delivery == packages, f"packages without delivery ownership: {sorted(packages - packages_with_delivery)}")

    require(errors, operation_doc.get("status") == "SOURCE_DERIVED_OPERATION_OWNERSHIP_NOT_IMPLEMENTED", "operation registry status mismatch")
    require(errors, operation_doc.get("package_function_source_count") == 42, "operation registry function-source count mismatch")
    require(errors, operation_doc.get("foundation_operation_source_count") == 3, "operation registry foundation count mismatch")
    require(errors, operation_doc.get("operation_identity") == "<package>::<operation>", "operation identity rule mismatch")
    require(errors, operation_doc.get("operation_count") == "DERIVED_FROM_REGISTERED_MARKDOWN", "operation count must remain source-derived")
    require(errors, operation_doc.get("all_registered_operations_have_exact_package_owner") is True, "operation package ownership disabled")
    require(errors, operation_doc.get("all_operations_enter_public_entry_module") is True, "operation entry-module rule disabled")
    require(errors, operation_doc.get("undocumented_public_operation_allowed") is False, "undocumented operations allowed")
    require(errors, operation_doc.get("dynamic_operation_registration_allowed") is False, "dynamic operation registration allowed")
    require(errors, operation_doc.get("operation_inventory_authorizes_implementation") is False, "operation registry authorizes implementation")
    require(errors, operation_doc.get("operation_inventory_accepts_package") is False, "operation registry accepts package")
    require(errors, operation_doc.get("operation_inventory_changes_launch_state") is False, "operation registry changes launch state")
    require(errors, operation_doc.get("function_registry") == manifest.get("function_registry"), "operation/function registry path mismatch")
    require(errors, operation_doc.get("module_registry") == manifest.get("module_registry"), "operation/module registry path mismatch")
    require(errors, topology["operation_count"] > 0, "source-derived operation inventory is empty")

    foundation_rows = rows(operation_doc, "foundation", "package")
    require(errors, set(foundation_rows) == {"search-contracts", "search-domain", "search-ports"}, "operation foundation set mismatch")
    for package, row in foundation_rows.items():
        require(errors, row.get("source") == topology["foundation_rows"][package].get("primary_contract"), f"{package}: foundation source mismatch")
        require(errors, row.get("public_entry_module") in modules.get(package, set()), f"{package}: foundation public entry module missing")

    case_rows = rows(qualification_doc, "case", "id")
    require(errors, qualification_doc.get("case_count") == 40, "architecture coverage case count must be 40")
    require(errors, len(case_rows) == 40, "architecture coverage must contain 40 unique cases")
    for case_id, row in case_rows.items():
        require(errors, row.get("mandatory") is True, f"coverage case {case_id} must be mandatory")
        require(errors, row.get("result") == "UNAVAILABLE", f"coverage case {case_id} has premature evidence")

    required_files = p00_manifest.get("required_files")
    require(errors, p00_manifest.get("required_file_count") == 13, "P00 required-file count must be 13")
    require(errors, isinstance(required_files, list) and len(required_files) == 13, "P00 required_files array must contain 13 entries")
    require(errors, isinstance(required_files, list) and "TYPE_COMPLETIONS.md" in required_files, "TYPE_COMPLETIONS is not part of P00 contract pack")
    completion_text = read(manifest["type_completion_contract"])
    for token in ("RecipeIdV1", "RecipeBodyV1", "ComparisonAxis", "ProtocolRange", "PackageOpaque", "PC-018"):
        source_text = completion_text if token != "PC-018" else read("docs/contracts/p00/CONTRACT_CHALLENGES.md")
        require(errors, token in source_text, f"P00 type completion evidence missing {token}")

    _manual_workflow(errors, ".github/workflows/architecture-coverage.yml")

    return {
        "assignment_tasks": len(assignment_paths),
        "delivery_slices": len(delivery_rows),
        "derived_operations": topology["operation_count"],
        "qualification_cases": len(case_rows),
    }
