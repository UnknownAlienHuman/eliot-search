from __future__ import annotations

import re
from typing import Any

from .common import ROOT, load, operation_names, read, require, rows, validate_module_ref


def validate_topology(errors: list[str]) -> dict[str, Any]:
    manifest = load("swarm/coverage/manifest.toml")
    package_doc = load(manifest["package_registry"])
    function_doc = load(manifest["function_registry"])
    module_doc = load(manifest["module_registry"])
    section_doc = load(manifest["architecture_section_registry"])
    capability_doc = load(manifest["capability_registry"])
    invariant_doc = load(manifest["invariant_registry"])
    port_doc = load(manifest["port_registry"])
    config_doc = load(manifest["configuration_registry"])
    architecture = read(manifest["architecture_master"])

    package_rows = rows(package_doc, "package", "name")
    packages = set(package_rows)
    require(errors, package_doc.get("package_count") == 45 and len(packages) == 45, "package registry must contain exactly 45 packages")
    require(errors, manifest.get("package_count") == 45, "coverage manifest package count mismatch")

    foundation_rows = rows(function_doc, "foundation", "package")
    function_rows = rows(function_doc, "package", "name")
    foundation = {"search-contracts", "search-domain", "search-ports"}
    require(errors, set(foundation_rows) == foundation, "foundation package set mismatch")
    require(errors, set(function_rows) == packages - foundation, "package function source set mismatch")
    require(errors, len(function_rows) == 42, "expected 42 package-local function sources")

    assignment_paths: set[str] = set()
    for package, row in package_rows.items():
        assignment = row.get("assignment")
        require(errors, isinstance(assignment, str), f"{package}: assignment path missing")
        if isinstance(assignment, str):
            require(errors, (ROOT / assignment).is_file(), f"{package}: assignment file missing: {assignment}")
            require(errors, assignment not in assignment_paths, f"duplicate assignment path {assignment}")
            assignment_paths.add(assignment)
            if (ROOT / assignment).is_file():
                assignment_text = read(assignment)
                require(errors, package in assignment_text, f"{package}: assignment does not name package")
                require(errors, len(assignment_text.strip()) > 100, f"{package}: assignment is empty or underspecified")
    actual_assignments = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "swarm/assignments").glob("*.md")
        if path.name != "README.md"
    }
    require(errors, actual_assignments == assignment_paths, f"orphan/missing assignments: {sorted(actual_assignments ^ assignment_paths)}")

    module_packets = module_doc.get("packet")
    if not isinstance(module_packets, list):
        errors.append("module registry packet list missing")
        module_packets = []
    module_rows: dict[str, dict[str, Any]] = {}
    modules: dict[str, set[str]] = {}
    module_total = 0
    for packet in module_packets:
        if not isinstance(packet, dict) or not isinstance(packet.get("path"), str):
            errors.append("invalid module packet entry")
            continue
        path = packet["path"]
        require(errors, (ROOT / path).is_file(), f"missing module packet {path}")
        if not (ROOT / path).is_file():
            continue
        document = load(path)
        entries = rows(document, "package", "name")
        require(errors, document.get("package_count") == len(entries), f"{path}: package count mismatch")
        packet_module_total = 0
        for package, entry in entries.items():
            require(errors, package not in module_rows, f"duplicate module packet for {package}")
            if package in module_rows:
                continue
            module_rows[package] = entry
            names = entry.get("modules")
            if not isinstance(names, list) or not all(isinstance(name, str) for name in names):
                errors.append(f"{package}: modules must be a string array")
                names = []
            require(errors, len(names) == len(set(names)), f"{package}: duplicate module name")
            require(errors, entry.get("module_count") == len(names), f"{package}: module count mismatch")
            require(errors, len(names) <= module_doc.get("max_modules_per_package"), f"{package}: module count exceeds maximum")
            require(errors, entry.get("public_entry_module") in names, f"{package}: public entry module missing")
            for name in names:
                require(errors, re.fullmatch(r"[a-z][a-z0-9_]*", name) is not None, f"{package}: invalid module name {name}")
            modules[package] = set(names)
            packet_module_total += len(names)
            module_total += len(names)
        require(errors, document.get("module_count") == packet_module_total, f"{path}: declared module count mismatch")
        require(errors, packet.get("package_count") == len(entries), f"{path}: summary package count mismatch")
        require(errors, packet.get("module_count") == packet_module_total, f"{path}: summary module count mismatch")

    require(errors, set(module_rows) == packages, f"module package closure mismatch: {sorted(set(module_rows) ^ packages)}")
    require(errors, module_doc.get("package_count") == 45, "module registry package count mismatch")
    require(errors, module_doc.get("module_count") == module_total == 479, "logical module total must be 479")
    require(errors, module_doc.get("max_modules_per_package") == 15, "module ceiling must remain 15")
    require(errors, module_doc.get("implementation_authorized_by_this_registry") is False, "module registry authorizes implementation")

    for package, entry in module_rows.items():
        registry = package_rows.get(package, {})
        require(errors, entry.get("path") == registry.get("path"), f"{package}: module path differs from package registry")
        expected_source = foundation_rows[package].get("primary_contract") if package in foundation_rows else function_rows[package].get("functions")
        require(errors, entry.get("operation_source") == expected_source, f"{package}: module operation source mismatch")
        require(errors, entry.get("all_public_operations_enter_through_public_entry") is True, f"{package}: public entry invariant disabled")
        require(errors, entry.get("package_state_must_remain_inside_declared_modules") is True, f"{package}: state containment invariant disabled")
        require(errors, entry.get("cross_package_module_imports_require_public_handoff") is True, f"{package}: cross-package handoff invariant disabled")

    qualified_operations: set[str] = set()
    registered_function_paths: set[str] = set()
    operation_count = 0
    for package, row in function_rows.items():
        path = row.get("functions")
        require(errors, isinstance(path, str), f"{package}: function source missing")
        if not isinstance(path, str):
            continue
        registered_function_paths.add(path)
        require(errors, path.startswith(package_rows[package]["path"] + "/"), f"{package}: function source is not package-local")
        require(errors, path.endswith("/FUNCTIONS.md"), f"{package}: function source must be FUNCTIONS.md")
        require(errors, (ROOT / path).is_file(), f"{package}: function source does not exist")
        if not (ROOT / path).is_file():
            continue
        operations = operation_names(read(path))
        require(errors, len(operations) > 0, f"{package}: no source-derived operations")
        for operation in operations:
            identity = f"{package}::{operation}"
            require(errors, identity not in qualified_operations, f"duplicate qualified operation {identity}")
            qualified_operations.add(identity)
        operation_count += len(operations)
    actual_function_files = {
        path.relative_to(ROOT).as_posix()
        for package_root in (ROOT / "crates", ROOT / "bins")
        if package_root.exists()
        for path in package_root.rglob("FUNCTIONS.md")
    }
    require(errors, actual_function_files == registered_function_paths, f"orphan/missing function packets: {sorted(actual_function_files ^ registered_function_paths)}")

    source_sections = {
        match.group(1): match.group(2).strip()
        for match in re.finditer(r"^##\s+(S\d+)\.\s+(.+?)\s*$", architecture, flags=re.MULTILINE)
    }
    section_rows = rows(section_doc, "section", "id")
    require(errors, set(source_sections) == {f"S{i}" for i in range(40)}, "architecture source must contain S0-S39")
    require(errors, set(section_rows) == set(source_sections), "architecture section registry mismatch")
    for section_id, row in section_rows.items():
        require(errors, isinstance(row.get("heading"), str) and row["heading"].strip() != "", f"{section_id}: heading missing")
        owners = row.get("primary_packages")
        refs = row.get("modules")
        require(errors, isinstance(owners, list) and len(owners) > 0, f"{section_id}: owner packages missing")
        require(errors, isinstance(refs, list) and len(refs) > 0, f"{section_id}: module refs missing")
        for package in owners if isinstance(owners, list) else []:
            require(errors, package in packages, f"{section_id}: unknown owner package {package}")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, section_id)

    source_cells: dict[str, str] = {}
    for match in re.finditer(r"^\|\s*(C\d{2})\s+([^|]+?)\s*\|", architecture, flags=re.MULTILINE):
        source_cells[match.group(1)] = match.group(2).strip()
    capability_rows = rows(capability_doc, "cell", "id")
    require(errors, set(source_cells) == {f"C{i:02d}" for i in range(31)}, "architecture source must contain C00-C30")
    require(errors, set(capability_rows) == set(source_cells), "capability registry mismatch")
    for cell_id, row in capability_rows.items():
        require(errors, isinstance(row.get("name"), str) and row["name"].strip() != "", f"{cell_id}: capability name missing")
        for key in ("primary_packages", "supporting_packages", "state_owner_packages"):
            values = row.get(key, [])
            require(errors, isinstance(values, list), f"{cell_id}: {key} must be an array")
            if key == "primary_packages":
                require(errors, isinstance(values, list) and len(values) > 0, f"{cell_id}: primary owner missing")
            for package in values if isinstance(values, list) else []:
                require(errors, package in packages, f"{cell_id}: unknown package {package}")
        refs = row.get("modules")
        require(errors, isinstance(refs, list) and len(refs) > 0, f"{cell_id}: module refs missing")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, cell_id)

    source_invariants = set(re.findall(r"^\s*(INV-\d{2}):", architecture, flags=re.MULTILINE))
    invariant_rows = rows(invariant_doc, "invariant", "id")
    require(errors, source_invariants == {f"INV-{i:02d}" for i in range(1, 31)}, "architecture source must contain INV-01..INV-30")
    require(errors, set(invariant_rows) == source_invariants, "invariant registry mismatch")
    for invariant_id, row in invariant_rows.items():
        owners = row.get("enforcement_packages")
        refs = row.get("modules")
        require(errors, isinstance(owners, list) and len(owners) > 0, f"{invariant_id}: enforcement owner missing")
        require(errors, isinstance(refs, list) and len(refs) > 0, f"{invariant_id}: module refs missing")
        for package in owners if isinstance(owners, list) else []:
            require(errors, package in packages, f"{invariant_id}: unknown package {package}")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, invariant_id)

    port_source = read("docs/contracts/p00/PORT_OPERATIONS.md")
    heading_matches = list(re.finditer(r"^###\s+`([A-Za-z][A-Za-z0-9]+Port)`\s*$", port_source, flags=re.MULTILINE))
    source_port_methods: dict[str, list[str]] = {}
    for index, match in enumerate(heading_matches):
        start = match.end()
        end = heading_matches[index + 1].start() if index + 1 < len(heading_matches) else len(port_source)
        body = port_source[start:end]
        source_port_methods[match.group(1)] = re.findall(r"^- `([a-z][a-z0-9_]*)\(", body, flags=re.MULTILINE)
    port_rows = rows(port_doc, "port", "name")
    require(errors, len(source_port_methods) == 23, "PORT_OPERATIONS must define 23 ports")
    require(errors, set(port_rows) == set(source_port_methods), "port registry set mismatch")
    for port_name, row in port_rows.items():
        require(errors, row.get("methods") == source_port_methods.get(port_name), f"{port_name}: method inventory mismatch")
        implementation_package = row.get("implementation_package")
        implementation_module = row.get("implementation_module")
        require(errors, implementation_package in packages, f"{port_name}: unknown implementation package")
        if isinstance(implementation_package, str) and isinstance(implementation_module, str):
            validate_module_ref(errors, f"{implementation_package}:{implementation_module}", modules, port_name)
        else:
            errors.append(f"{port_name}: implementation owner incomplete")
    require(errors, port_rows.get("ResidencyPolicyPort", {}).get("implementation_package") == "search-revision-store", "ResidencyPolicyPort must be implemented by search-revision-store")
    require(errors, port_rows.get("ClockPort", {}).get("implementation_package") == "eliot-searchd", "ClockPort must be the daemon private adapter")

    config_rows = rows(config_doc, "section", "name")
    require(errors, len(config_rows) == 20, "configuration registry must contain 20 sections")
    config_packets: set[str] = set()
    for name, row in config_rows.items():
        owner = row.get("owner")
        packet = row.get("packet")
        require(errors, owner in packages, f"config {name}: unknown owner {owner}")
        require(errors, isinstance(packet, str) and (ROOT / packet).is_file(), f"config {name}: missing packet {packet}")
        if isinstance(packet, str):
            require(errors, packet not in config_packets, f"config {name}: duplicate packet path {packet}")
            config_packets.add(packet)

    return {
        "manifest": manifest,
        "package_rows": package_rows,
        "packages": packages,
        "foundation_rows": foundation_rows,
        "function_rows": function_rows,
        "assignment_paths": assignment_paths,
        "modules": modules,
        "module_rows": module_rows,
        "module_total": module_total,
        "operation_count": operation_count,
        "section_count": len(section_rows),
        "capability_count": len(capability_rows),
        "invariant_count": len(invariant_rows),
        "port_count": len(port_rows),
        "config_count": len(config_rows),
    }
