from __future__ import annotations

from typing import Any

from .common import ROOT, load, require, rows, validate_module_ref


def validate_control(
    errors: list[str],
    topology: dict[str, Any],
    schemas: dict[str, Any],
) -> dict[str, Any]:
    manifest = topology["manifest"]
    packages = topology["packages"]
    modules = topology["modules"]
    operations = load(manifest["operation_registry"])
    tasks = load(manifest["task_registry"])
    delivery_doc = load(manifest["delivery_registry"])
    cases_doc = load("qualification/architecture-coverage/cases-v1.toml")
    launch = load("swarm/launch-state.toml")

    require(errors, operations.get("status") == "SOURCE_DERIVED_OPERATION_OWNERSHIP_CLOSED_NOT_IMPLEMENTED", "operation registry status changed")
    require(errors, operations.get("function_registry") == "swarm/function-packets.toml", "operation function registry mismatch")
    require(errors, operations.get("module_registry") == "swarm/module-packets.toml", "operation module registry mismatch")
    require(errors, operations.get("package_count") == 45, "operation package count mismatch")
    require(errors, operations.get("foundation_package_count") == 3, "operation foundation count mismatch")
    require(errors, operations.get("package_function_source_count") == 42, "operation function-source count mismatch")
    require(errors, operations.get("operation_count") == "DERIVED_BY_VALIDATOR", "operation count must remain source-derived")
    require(errors, operations.get("implementation_authorized_by_this_registry") is False, "operation registry authorizes implementation")

    identity = operations.get("identity", {})
    require(errors, identity.get("format") == "<package>::<operation>", "qualified operation identity format changed")
    require(errors, identity.get("package_qualified_identity_required") is True, "package-qualified operation identity disabled")
    require(errors, identity.get("duplicate_unqualified_operation_names_allowed") is True, "unqualified operation collision rule changed")
    require(errors, identity.get("duplicate_package_qualified_operation_names_allowed") is False, "qualified operation collisions allowed")

    extraction = operations.get("extraction", {})
    require(errors, extraction.get("minimum_operations_per_nonfoundation_package") == 1, "minimum operation count weakened")
    require(errors, extraction.get("ignore_test_fixture_and_example_blocks") is True, "fixture/example exclusion disabled")
    require(errors, extraction.get("ignore_Rust_trait_methods_in_package_FUNCTIONS") is False, "trait operations excluded")

    ownership = operations.get("ownership", {})
    for key in (
        "every_operation_enters_through_public_entry_module",
        "internal_delegation_must_remain_within_declared_package_modules",
        "operation_source_must_be_package_local",
    ):
        require(errors, ownership.get(key) is True, f"operation ownership invariant disabled: {key}")
    require(errors, ownership.get("operation_owner_source") == "function_registry_package", "operation owner source changed")
    require(errors, ownership.get("public_entry_module_source") == "module_registry_package", "public entry source changed")
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

    expected_foundations = {
        "search_contracts": ("search-contracts", "docs/contracts/p00/README.md", "lib"),
        "search_domain": ("search-domain", "docs/contracts/p00/SUPPORT_SCHEMAS.md", "lib"),
        "search_ports": ("search-ports", "docs/contracts/p00/PORT_OPERATIONS.md", "lib"),
    }
    foundation_table = operations.get("foundation", {})
    for key, (package, source, public_entry) in expected_foundations.items():
        row = foundation_table.get(key, {})
        require(errors, row.get("package") == package, f"operation foundation {key}: package mismatch")
        require(errors, row.get("source") == source, f"operation foundation {key}: source mismatch")
        require(errors, row.get("public_entry_module") == public_entry, f"operation foundation {key}: public entry mismatch")

    require(errors, tasks.get("status") == "STATIC_TASK_OWNERSHIP_CLOSED_NOT_IMPLEMENTED", "task registry status changed")
    require(errors, tasks.get("package_registry") == "swarm/crates.toml", "task package registry mismatch")
    require(errors, tasks.get("delivery_registry") == "swarm/coverage/delivery-slices.toml", "task delivery registry mismatch")
    require(errors, tasks.get("package_assignment_task_count") == 45, "task assignment count mismatch")
    require(errors, tasks.get("delivery_slice_task_count") == 19, "task delivery count mismatch")
    require(errors, tasks.get("implementation_authorized_by_this_registry") is False, "task registry authorizes implementation")

    package_tasks = tasks.get("package_assignment_tasks", {})
    for key in (
        "one_assignment_per_package",
        "assignment_file_required",
        "assignment_must_name_owned_state_or_behavior",
        "assignment_must_name_forbidden_or_non_owned_behavior",
        "assignment_must_name_exact_package_write_scope",
        "assignment_must_not_override_dependency_or_function_registry",
    ):
        require(errors, package_tasks.get(key) is True, f"package task invariant disabled: {key}")
    require(errors, package_tasks.get("source") == "swarm/crates.toml::package.assignment", "package task source mismatch")

    delivery_tasks = tasks.get("delivery_tasks", {})
    for key in (
        "one_registry_entry_per_delivery_slice",
        "primary_package_set_required",
        "module_set_required",
        "required_outputs_required",
        "exit_evidence_required",
    ):
        require(errors, delivery_tasks.get(key) is True, f"delivery task invariant disabled: {key}")
    require(errors, delivery_tasks.get("source") == "docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md#H17", "delivery task source mismatch")

    qualification_tasks = tasks.get("qualification_tasks", {})
    require(errors, qualification_tasks.get("source_roots") == ["qualification", "tests"], "qualification task roots mismatch")
    for root in qualification_tasks.get("source_roots", []):
        require(errors, (ROOT / root).is_dir(), f"qualification source root missing: {root}")
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

    delivery_rows = rows(delivery_doc, "slice", "id")
    require(errors, set(delivery_rows) == {f"P{i:02d}" for i in range(19)}, "delivery registry must contain P00-P18")
    covered_packages: set[str] = set()
    for slice_id, row in delivery_rows.items():
        owners = row.get("primary_packages")
        refs = row.get("modules")
        outputs = row.get("required_outputs")
        evidence = row.get("exit_evidence")
        require(errors, isinstance(owners, list) and len(owners) > 0, f"{slice_id}: package owners missing")
        require(errors, isinstance(refs, list) and len(refs) > 0, f"{slice_id}: module refs missing")
        require(errors, isinstance(outputs, list) and len(outputs) > 0, f"{slice_id}: required outputs missing")
        require(errors, isinstance(evidence, list) and len(evidence) > 0, f"{slice_id}: exit evidence missing")
        for package in owners if isinstance(owners, list) else []:
            require(errors, package in packages, f"{slice_id}: unknown package {package}")
            covered_packages.add(package)
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, slice_id)
    require(errors, covered_packages == packages, f"packages absent from delivery slices: {sorted(packages - covered_packages)}")

    case_rows = cases_doc.get("case")
    require(errors, cases_doc.get("schema_version") == 1, "coverage case schema mismatch")
    require(errors, cases_doc.get("suite") == "architecture_coverage_closure_v1", "coverage case suite mismatch")
    require(errors, cases_doc.get("status") == "STRUCTURAL_NOT_EXECUTED", "coverage case status changed")
    require(errors, cases_doc.get("case_count") == 40, "coverage case_count must be 40")
    require(errors, isinstance(case_rows, list) and len(case_rows) == 40, "coverage case inventory must contain 40 rows")
    if isinstance(case_rows, list):
        ids = [row.get("id") for row in case_rows if isinstance(row, dict)]
        require(errors, len(ids) == len(set(ids)) == 40, "coverage case IDs must be unique")
        for row in case_rows:
            require(errors, isinstance(row, dict) and row.get("mandatory") is True, "coverage case must be mandatory")
            require(errors, isinstance(row, dict) and row.get("result") == "UNAVAILABLE", "coverage case must remain UNAVAILABLE")

    count_checks = {
        "package_count": len(packages),
        "foundation_package_count": len(topology["foundation_rows"]),
        "package_function_packet_count": len(topology["function_rows"]),
        "module_packet_count": len(topology["module_rows"]),
        "package_assignment_task_count": len(topology["assignment_paths"]),
        "architecture_section_count": topology["section_count"],
        "architecture_invariant_count": topology["invariant_count"],
        "capability_cell_count": topology["capability_count"],
        "shared_port_count": topology["port_count"],
        "configuration_section_count": topology["config_count"],
        "type_registry_named_symbol_count": schemas["type_registry_symbols"],
        "named_type_completion_count": schemas["completion_symbols"],
        "p00_schema_or_registry_count": schemas["schema_total"],
        "canonical_primitive_family_count": schemas["primitive_families"],
        "recipe_count": schemas["recipe_count"],
        "delivery_slice_count": len(delivery_rows),
    }
    for key, actual in count_checks.items():
        require(errors, manifest.get(key) == actual, f"coverage manifest count mismatch for {key}: {manifest.get(key)} != {actual}")
    require(errors, manifest.get("support_and_record_schema_count") == 97, "support/record schema count must be 97")

    for key in (
        "one_shape_owner_per_schema",
        "one_state_owner_per_mutable_schema",
        "one_trait_owner_per_shared_port",
        "one_concrete_implementation_owner_per_shared_port",
        "one_primary_owner_set_per_capability_cell",
        "one_package_module_packet_per_package",
        "one_assignment_task_per_package",
        "all_packages_covered_by_delivery_slice",
    ):
        require(errors, manifest.get(key) is True, f"coverage invariant disabled: {key}")
    for key in (
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ):
        require(errors, manifest.get(key) is False, f"coverage authority flag enabled: {key}")

    corrections = manifest.get("known_corrections", {})
    require(errors, corrections.get("source_owner_generation") == "BLAKE3_DIGEST_PART_I_WINS", "SourceOwnerGeneration correction missing")
    require(errors, corrections.get("residency_policy_port_implementation") == "search-revision-store", "ResidencyPolicyPort correction missing")
    require(errors, corrections.get("clock_port_implementation") == "eliot-searchd_private_platform_adapter", "ClockPort correction missing")
    require(errors, corrections.get("missing_named_type_contracts") == "CLOSED_BY_TYPE_COMPLETIONS_PC_018", "named type correction missing")

    require(errors, launch.get("active_stage") == "P00" and launch.get("active_wave") == 0, "launch authority moved from P00/W0")
    require(errors, launch.get("authorized_packages") == ["search-contracts"], "authorized package set changed")

    workflow = ROOT / ".github/workflows/architecture-coverage.yml"
    require(errors, workflow.is_file(), "architecture coverage workflow missing")
    if workflow.is_file():
        workflow_text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false", "validate-architecture-coverage.ps1"):
            require(errors, token in workflow_text, f"coverage workflow missing {token}")
        for token in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:", "\n  repository_dispatch:"):
            require(errors, token not in workflow_text, f"automatic coverage workflow trigger {token.strip()}")

    return {
        "delivery_count": len(delivery_rows),
        "qualification_case_count": len(case_rows) if isinstance(case_rows, list) else 0,
    }
