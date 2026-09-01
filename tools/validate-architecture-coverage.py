#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import tomllib
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
NONE = "NONE"
RESERVED_SIGNATURE_WORDS = {"if", "for", "while", "match", "loop", "return"}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


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


def fail(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def section_between(text: str, start: str, end: str | None) -> str:
    begin = text.find(start)
    if begin < 0:
        raise ValueError(f"missing section heading: {start}")
    begin += len(start)
    if end is None:
        return text[begin:]
    finish = text.find(end, begin)
    if finish < 0:
        raise ValueError(f"missing section heading: {end}")
    return text[begin:finish]


def fenced_blocks(text: str, language: str | None = None) -> list[str]:
    if language is None:
        pattern = r"```[^\n]*\n(.*?)```"
    else:
        pattern = rf"```{re.escape(language)}\s*\n(.*?)```"
    return re.findall(pattern, text, flags=re.DOTALL)


def normalize_type_name(name: str) -> str:
    return name.split("<", 1)[0]


def operation_names(text: str) -> list[str]:
    names: set[str] = set()
    for match in re.finditer(r"^#{2,3}\s+`([a-z][a-z0-9_]*)\b", text, flags=re.MULTILINE):
        names.add(match.group(1))
    for match in re.finditer(r"`([a-z][a-z0-9_]*)\([^`]*\)`", text):
        if match.group(1) not in RESERVED_SIGNATURE_WORDS:
            names.add(match.group(1))
    for block in fenced_blocks(text):
        for match in re.finditer(
            r"^(?:pub\s+)?(?:async\s+)?(?:fn\s+)?([a-z][a-z0-9_]*)\s*\(",
            block,
            flags=re.MULTILINE,
        ):
            if match.group(1) not in RESERVED_SIGNATURE_WORDS:
                names.add(match.group(1))
    return sorted(names)


def top_level_yaml_labels(text: str) -> set[str]:
    labels: set[str] = set()
    for block in fenced_blocks(text, "yaml"):
        for line in block.splitlines():
            match = re.match(r"^([A-Za-z][A-Za-z0-9_<>@,.-]*):(?:\s|$)", line)
            if match:
                labels.add(match.group(1))
    return labels


def text_block_lines(section: str) -> list[list[str]]:
    return [[line.strip() for line in block.splitlines() if line.strip()] for block in fenced_blocks(section, "text")]


def validate_module_ref(
    errors: list[str],
    ref: str,
    modules: dict[str, set[str]],
    owner: str,
) -> None:
    if ":" not in ref:
        errors.append(f"{owner}: invalid module ref {ref}")
        return
    package, module = ref.split(":", 1)
    if package not in modules:
        errors.append(f"{owner}: unknown package in module ref {ref}")
    elif module not in modules[package]:
        errors.append(f"{owner}: unknown module in ref {ref}")


def validate_owner_pair(
    errors: list[str],
    package: Any,
    module: Any,
    modules: dict[str, set[str]],
    owner: str,
    allow_none: bool = True,
) -> None:
    if allow_none and package == NONE and module == NONE:
        return
    if not isinstance(package, str) or not isinstance(module, str):
        errors.append(f"{owner}: owner package/module must be strings")
        return
    validate_module_ref(errors, f"{package}:{module}", modules, owner)


def exact_type_registry_symbols(type_registry: str) -> set[str]:
    symbols: set[str] = set()

    bounds = section_between(type_registry, "## Bounds and collections", "## Opaque and display wrappers")
    for block in fenced_blocks(bounds, "text"):
        for line in block.splitlines():
            match = re.match(r"^(Bounded[A-Za-z0-9_]+(?:<[^>]+>)?)", line.strip())
            if match:
                symbols.add(match.group(1))
    symbols.update(top_level_yaml_labels(bounds))

    opaque = section_between(type_registry, "## Opaque and display wrappers", "## Identity and reference registry")
    for match in re.finditer(r"^\| `([^`]+)` \|", opaque, flags=re.MULTILINE):
        symbols.add(match.group(1))

    identity = section_between(type_registry, "## Identity and reference registry", "## Baseline semantic registries")
    identity_text_blocks = text_block_lines(identity)
    if len(identity_text_blocks) < 3:
        raise ValueError("TYPE_REGISTRY identity section must contain three text registries")
    for block in identity_text_blocks[:3]:
        for line in block:
            for token in [part.strip() for part in line.split(",")]:
                token = token.rstrip(".")
                if token:
                    symbols.add(token)
    symbols.update(top_level_yaml_labels(identity))

    semantic = section_between(type_registry, "## Baseline semantic registries", "## Coverage and freshness records")
    for block in fenced_blocks(semantic, "text"):
        for line in block.splitlines():
            match = re.match(r"^([A-Z][A-Za-z0-9_]+)\s*=", line.strip())
            if match:
                symbols.add(match.group(1))
    if "`EntityKind`" in semantic:
        symbols.add("EntityKind")

    coverage = section_between(type_registry, "## Coverage and freshness records", "## Port-support records")
    symbols.update(top_level_yaml_labels(coverage))
    port_support = section_between(type_registry, "## Port-support records", "## Ownership and visibility summary")
    symbols.update(top_level_yaml_labels(port_support))

    return symbols


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.parse_args()

    errors: list[str] = []
    warnings: list[str] = []

    try:
        manifest = load("swarm/coverage/manifest.toml")
        package_doc = load(manifest["package_registry"])
        function_doc = load(manifest["function_registry"])
        module_doc = load(manifest["module_registry"])
        task_doc = load(manifest["task_registry"])
        section_doc = load(manifest["architecture_section_registry"])
        capability_doc = load(manifest["capability_registry"])
        invariant_doc = load(manifest["invariant_registry"])
        port_doc = load(manifest["port_registry"])
        schema_doc = load(manifest["schema_registry"])
        recipe_doc = load(manifest["recipe_registry"])
        delivery_doc = load(manifest["delivery_registry"])
        reason_doc = load(manifest["reason_registry"])
        config_doc = load(manifest["configuration_registry"])
        launch = load("swarm/launch-state.toml")
        p00_manifest = load("docs/contracts/p00/manifest.toml")
        architecture = read(manifest["architecture_master"])
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    package_rows = rows(package_doc, "package", "name")
    packages = set(package_rows)
    fail(errors, package_doc.get("package_count") == 45 and len(packages) == 45, "package registry must contain 45 packages")
    fail(errors, manifest.get("package_count") == 45, "coverage manifest package count mismatch")

    # Function and assignment closure.
    foundation_rows = rows(function_doc, "foundation", "package")
    function_rows = rows(function_doc, "package", "name")
    foundation = {"search-contracts", "search-domain", "search-ports"}
    fail(errors, set(foundation_rows) == foundation, "foundation package set mismatch")
    fail(errors, set(function_rows) == packages - foundation, "package function source set mismatch")
    fail(errors, len(function_rows) == 42, "expected 42 package-local function sources")

    assignment_paths: set[str] = set()
    for package, row in package_rows.items():
        assignment = row.get("assignment")
        fail(errors, isinstance(assignment, str), f"{package}: assignment path missing")
        if isinstance(assignment, str):
            fail(errors, (ROOT / assignment).is_file(), f"{package}: assignment file missing: {assignment}")
            fail(errors, assignment not in assignment_paths, f"duplicate assignment path {assignment}")
            assignment_paths.add(assignment)
            if (ROOT / assignment).is_file():
                text = read(assignment)
                fail(errors, package in text, f"{package}: assignment does not name package")
                fail(errors, len(text.strip()) > 100, f"{package}: assignment is empty/underspecified")
    actual_assignments = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "swarm/assignments").glob("*.md")
        if path.name != "README.md"
    }
    fail(errors, actual_assignments == assignment_paths, f"orphan/missing assignments: {sorted(actual_assignments ^ assignment_paths)}")

    # Module closure.
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
        fail(errors, (ROOT / path).is_file(), f"missing module packet {path}")
        if not (ROOT / path).is_file():
            continue
        document = load(path)
        entries = rows(document, "package", "name")
        fail(errors, document.get("package_count") == len(entries), f"{path}: package count mismatch")
        declared_packet_modules = 0
        for package, entry in entries.items():
            if package in module_rows:
                errors.append(f"duplicate module packet for {package}")
                continue
            module_rows[package] = entry
            names = entry.get("modules")
            if not isinstance(names, list) or not all(isinstance(name, str) for name in names):
                errors.append(f"{package}: modules must be a string array")
                names = []
            fail(errors, len(names) == len(set(names)), f"{package}: duplicate module name")
            fail(errors, entry.get("module_count") == len(names), f"{package}: module count mismatch")
            fail(errors, len(names) <= module_doc.get("max_modules_per_package"), f"{package}: module count exceeds maximum")
            fail(errors, entry.get("public_entry_module") in names, f"{package}: public entry module missing")
            for name in names:
                fail(errors, re.fullmatch(r"[a-z][a-z0-9_]*", name) is not None, f"{package}: invalid module name {name}")
            modules[package] = set(names)
            declared_packet_modules += len(names)
            module_total += len(names)
        fail(errors, document.get("module_count") == declared_packet_modules, f"{path}: declared module count mismatch")
        fail(errors, packet.get("package_count") == len(entries), f"{path}: summary package count mismatch")
        fail(errors, packet.get("module_count") == declared_packet_modules, f"{path}: summary module count mismatch")

    fail(errors, set(module_rows) == packages, f"module package closure mismatch: {sorted(set(module_rows) ^ packages)}")
    fail(errors, module_doc.get("package_count") == 45, "module registry package count mismatch")
    fail(errors, module_doc.get("module_count") == module_total == 479, "module total must be 479")

    for package, entry in module_rows.items():
        registry = package_rows.get(package, {})
        fail(errors, entry.get("path") == registry.get("path"), f"{package}: module path differs from package registry")
        expected_source = (
            foundation_rows[package].get("primary_contract") if package in foundation_rows else function_rows[package].get("functions")
        )
        fail(errors, entry.get("operation_source") == expected_source, f"{package}: module operation source mismatch")
        fail(errors, entry.get("all_public_operations_enter_through_public_entry") is True, f"{package}: public entry invariant disabled")
        fail(errors, entry.get("package_state_must_remain_inside_declared_modules") is True, f"{package}: state containment invariant disabled")
        fail(errors, entry.get("cross_package_module_imports_require_public_handoff") is True, f"{package}: cross-package handoff invariant disabled")

    # Source-derived operation ownership.
    qualified_operations: set[str] = set()
    operation_count = 0
    registered_function_paths: set[str] = set()
    for package, row in function_rows.items():
        path = row.get("functions")
        fail(errors, isinstance(path, str), f"{package}: function source missing")
        if not isinstance(path, str):
            continue
        registered_function_paths.add(path)
        fail(errors, path.startswith(package_rows[package]["path"] + "/"), f"{package}: function source is not package-local")
        fail(errors, path.endswith("/FUNCTIONS.md"), f"{package}: function source must be FUNCTIONS.md")
        fail(errors, (ROOT / path).is_file(), f"{package}: function source does not exist")
        if not (ROOT / path).is_file():
            continue
        operations = operation_names(read(path))
        fail(errors, len(operations) > 0, f"{package}: no source-derived operations")
        for operation in operations:
            identity = f"{package}::{operation}"
            fail(errors, identity not in qualified_operations, f"duplicate qualified operation {identity}")
            qualified_operations.add(identity)
        operation_count += len(operations)
    actual_function_files = {
        path.relative_to(ROOT).as_posix()
        for root in (ROOT / "crates", ROOT / "bins")
        if root.exists()
        for path in root.rglob("FUNCTIONS.md")
    }
    fail(errors, actual_function_files == registered_function_paths, f"orphan/missing function packets: {sorted(actual_function_files ^ registered_function_paths)}")

    # Architecture section closure.
    source_sections = {
        match.group(1): match.group(2).strip()
        for match in re.finditer(r"^## (S\d+)\. (.+)$", architecture, flags=re.MULTILINE)
    }
    section_rows = rows(section_doc, "section", "id")
    fail(errors, set(source_sections) == {f"S{i}" for i in range(40)}, "architecture source must contain S0-S39")
    fail(errors, set(section_rows) == set(source_sections), "architecture section registry mismatch")
    for section_id, row in section_rows.items():
        fail(errors, row.get("heading") == source_sections.get(section_id), f"{section_id}: heading mismatch")
        owners = row.get("primary_packages")
        refs = row.get("modules")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{section_id}: owner packages missing")
        fail(errors, isinstance(refs, list) and len(refs) > 0, f"{section_id}: module refs missing")
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{section_id}: unknown owner package {package}")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, section_id)

    # Capability cells.
    source_cells: dict[str, str] = {}
    for match in re.finditer(r"^\| (C\d{2}) ([^|]+?) \|", architecture, flags=re.MULTILINE):
        source_cells[match.group(1)] = match.group(2).strip()
    capability_rows = rows(capability_doc, "cell", "id")
    fail(errors, set(source_cells) == {f"C{i:02d}" for i in range(31)}, "architecture source must contain C00-C30")
    fail(errors, set(capability_rows) == set(source_cells), "capability registry mismatch")
    for cell_id, row in capability_rows.items():
        fail(errors, row.get("name") == source_cells.get(cell_id), f"{cell_id}: capability name mismatch")
        owners = row.get("primary_packages")
        refs = row.get("modules")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{cell_id}: primary owner missing")
        fail(errors, isinstance(refs, list) and len(refs) > 0, f"{cell_id}: module refs missing")
        for key in ("primary_packages", "supporting_packages", "state_owner_packages"):
            values = row.get(key, [])
            fail(errors, isinstance(values, list), f"{cell_id}: {key} must be an array")
            for package in values if isinstance(values, list) else []:
                fail(errors, package in packages, f"{cell_id}: unknown package {package}")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, cell_id)

    # Invariants.
    source_invariants = set(re.findall(r"^(INV-\d{2}):", architecture, flags=re.MULTILINE))
    invariant_rows = rows(invariant_doc, "invariant", "id")
    fail(errors, source_invariants == {f"INV-{i:02d}" for i in range(1, 31)}, "architecture source must contain INV-01..INV-30")
    fail(errors, set(invariant_rows) == source_invariants, "invariant registry mismatch")
    for invariant_id, row in invariant_rows.items():
        owners = row.get("enforcement_packages")
        refs = row.get("modules")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{invariant_id}: enforcement owner missing")
        fail(errors, isinstance(refs, list) and len(refs) > 0, f"{invariant_id}: module refs missing")
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{invariant_id}: unknown package {package}")
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, invariant_id)

    # Shared ports and exact methods.
    port_source = read("docs/contracts/p00/PORT_OPERATIONS.md")
    heading_matches = list(re.finditer(r"^### `([A-Za-z][A-Za-z0-9]+Port)`\s*$", port_source, flags=re.MULTILINE))
    source_port_methods: dict[str, list[str]] = {}
    for index, match in enumerate(heading_matches):
        start = match.end()
        end = heading_matches[index + 1].start() if index + 1 < len(heading_matches) else len(port_source)
        body = port_source[start:end]
        source_port_methods[match.group(1)] = re.findall(r"^- `([a-z][a-z0-9_]*)\(", body, flags=re.MULTILINE)
    port_rows = rows(port_doc, "port", "name")
    fail(errors, len(source_port_methods) == 23, "PORT_OPERATIONS must define 23 ports")
    fail(errors, set(port_rows) == set(source_port_methods), "port registry set mismatch")
    for port_name, row in port_rows.items():
        fail(errors, row.get("methods") == source_port_methods.get(port_name), f"{port_name}: method inventory mismatch")
        package = row.get("implementation_package")
        module = row.get("implementation_module")
        validate_owner_pair(errors, package, module, modules, port_name, allow_none=False)
    fail(errors, port_rows.get("ResidencyPolicyPort", {}).get("implementation_package") == "search-revision-store", "ResidencyPolicyPort must be implemented by search-revision-store")
    fail(errors, port_rows.get("ClockPort", {}).get("implementation_package") == "eliot-searchd", "ClockPort must be the daemon private adapter")

    # Configuration ownership.
    config_rows = rows(config_doc, "section", "name")
    fail(errors, len(config_rows) == 20, "configuration registry must contain 20 sections")
    config_packets: set[str] = set()
    for name, row in config_rows.items():
        owner = row.get("owner")
        packet = row.get("packet")
        fail(errors, owner in packages, f"config {name}: unknown owner {owner}")
        fail(errors, isinstance(packet, str) and (ROOT / packet).is_file(), f"config {name}: missing packet {packet}")
        if isinstance(packet, str):
            fail(errors, packet not in config_packets, f"config {name}: duplicate packet path {packet}")
            config_packets.add(packet)

    # Schema/type closure.
    schema_packets = schema_doc.get("packet")
    if not isinstance(schema_packets, list):
        errors.append("schema packet registry missing")
        schema_packets = []
    schema_names: set[str] = set()
    registered_bases: set[str] = set()
    schema_total = 0
    primitive_families = 0
    for packet in schema_packets:
        if not isinstance(packet, dict) or not isinstance(packet.get("path"), str):
            errors.append("invalid schema packet entry")
            continue
        path = packet["path"]
        fail(errors, (ROOT / path).is_file(), f"missing schema packet {path}")
        if not (ROOT / path).is_file():
            continue
        document = load(path)
        groups = document.get("group")
        if not isinstance(groups, list):
            errors.append(f"{path}: group array missing")
            continue
        packet_count = 0
        for group in groups:
            if not isinstance(group, dict):
                errors.append(f"{path}: invalid group")
                continue
            names = group.get("schemas")
            fail(errors, isinstance(names, list) and len(names) > 0, f"{path}:{group.get('id')}: schemas missing")
            if not isinstance(names, list):
                continue
            for name in names:
                fail(errors, isinstance(name, str) and name != "", f"{path}: invalid schema name")
                if not isinstance(name, str):
                    continue
                fail(errors, name not in schema_names, f"duplicate schema/type name {name}")
                schema_names.add(name)
                registered_bases.add(normalize_type_name(name))
                packet_count += 1
            source_files = group.get("source_files")
            fail(errors, isinstance(source_files, list) and len(source_files) > 0, f"{path}:{group.get('id')}: source files missing")
            combined = ""
            for source in source_files if isinstance(source_files, list) else []:
                fail(errors, isinstance(source, str) and (ROOT / source).is_file(), f"{path}: missing schema source {source}")
                if isinstance(source, str) and (ROOT / source).is_file():
                    combined += "\n" + read(source)
            for name in names:
                if isinstance(name, str):
                    fail(errors, normalize_type_name(name) in combined, f"{path}: schema/type {name} absent from declared sources")
            validate_owner_pair(errors, group.get("shape_owner_package"), group.get("shape_owner_module"), modules, f"schema {group.get('id')}")
            validate_owner_pair(errors, group.get("meaning_owner_package"), group.get("meaning_owner_module"), modules, f"schema {group.get('id')}")
            validate_owner_pair(errors, group.get("state_owner_package"), group.get("state_owner_module"), modules, f"schema {group.get('id')}")
            for package in group.get("secondary_state_owner_packages", []):
                fail(errors, package in packages, f"schema {group.get('id')}: unknown secondary owner {package}")
        fail(errors, document.get("schema_count") == packet_count, f"{path}: schema count mismatch")
        fail(errors, packet.get("schema_count") == packet_count, f"{path}: summary schema count mismatch")
        schema_total += packet_count
        primitive_families += len(document.get("primitive_family", [])) if isinstance(document.get("primitive_family", []), list) else 0

    fail(errors, schema_total == schema_doc.get("schema_or_registry_count") == 217, "schema/type total must be 217")
    fail(errors, schema_doc.get("type_registry_named_symbol_count") == 115, "TYPE_REGISTRY symbol count must be 115")
    fail(errors, schema_doc.get("completion_symbol_count") == 5, "type completion count must be 5")
    fail(errors, primitive_families == schema_doc.get("canonical_primitive_family_count") == 12, "canonical primitive family count must be 12")

    type_registry_symbols = exact_type_registry_symbols(read("docs/contracts/p00/TYPE_REGISTRY.md"))
    completion_symbols = {"RecipeIdV1", "RecipeBodyV1", "ComparisonAxis", "ProtocolRange", "PackageOpaque"}
    primitive_packet = load("swarm/coverage/schemas-primitives.toml")
    primitive_registered = {
        name
        for group in primitive_packet.get("group", [])
        if isinstance(group, dict)
        for name in group.get("schemas", [])
        if isinstance(name, str)
    }
    fail(errors, len(type_registry_symbols) == 115, f"TYPE_REGISTRY derived symbol count is {len(type_registry_symbols)}, expected 115")
    fail(errors, type_registry_symbols | completion_symbols == primitive_registered, f"primitive registry mismatch: {sorted((type_registry_symbols | completion_symbols) ^ primitive_registered)}")

    schema_source_files = [
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/SOURCE_GRAPH.md",
        "docs/contracts/p00/RECIPES.md",
        "docs/contracts/p00/QUERY_AND_RESULTS.md",
        "docs/contracts/p00/RECIPE_RESULTS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
    ]
    source_labels: set[str] = set()
    for path in schema_source_files:
        source_labels.update(top_level_yaml_labels(read(path)))
    recipe_ids = set(re.findall(r"^([a-z][a-z0-9_]+@1)$", read("docs/contracts/p00/RECIPES.md"), flags=re.MULTILINE))
    unregistered_labels = {
        label for label in source_labels
        if label not in schema_names and label not in recipe_ids
    }
    fail(errors, not unregistered_labels, f"unregistered top-level P00 schema labels: {sorted(unregistered_labels)}")

    # Recipes.
    recipe_rows = rows(recipe_doc, "recipe", "id")
    fail(errors, len(recipe_rows) == 11, "recipe registry must contain 11 recipes")
    fail(errors, set(recipe_rows) == recipe_ids, f"recipe registry mismatch: {sorted(set(recipe_rows) ^ recipe_ids)}")
    for recipe_id, row in recipe_rows.items():
        fail(errors, row.get("request_schema") in schema_names, f"{recipe_id}: unknown request schema")
        fail(errors, row.get("result_schema") in schema_names, f"{recipe_id}: unknown result schema")
        owners = row.get("primary_execution_packages")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{recipe_id}: execution owners missing")
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{recipe_id}: unknown execution package {package}")

    # Reasons.
    reason_text = read("docs/contracts/p00/REASON_CODES.md")
    source_reason_codes = set(re.findall(r"^([A-Z][A-Z0-9_]+)$", reason_text, flags=re.MULTILINE))
    registry_reason_codes: set[str] = set()
    for key in ("search_reason_codes", "protocol_error_codes", "contract_error_codes"):
        values = reason_doc.get(key, {}).get("values")
        fail(errors, isinstance(values, list), f"reason registry {key} missing")
        registry_reason_codes.update(values if isinstance(values, list) else [])
    fail(errors, source_reason_codes == registry_reason_codes, f"reason code registry mismatch: {sorted(source_reason_codes ^ registry_reason_codes)}")
    fail(errors, len(registry_reason_codes) == 51, "reason code total must be 51")

    # Delivery tasks.
    source_delivery = set(re.findall(r"^### (P\d{2}) —", architecture, flags=re.MULTILINE))
    delivery_rows = rows(delivery_doc, "slice", "id")
    fail(errors, source_delivery == {f"P{i:02d}" for i in range(19)}, "architecture source must contain P00-P18")
    fail(errors, set(delivery_rows) == source_delivery, "delivery slice registry mismatch")
    delivery_packages: set[str] = set()
    for slice_id, row in delivery_rows.items():
        owners = row.get("primary_packages")
        refs = row.get("modules")
        required_outputs = row.get("required_outputs")
        exit_evidence = row.get("exit_evidence")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{slice_id}: package owners missing")
        fail(errors, isinstance(refs, list) and len(refs) > 0, f"{slice_id}: module refs missing")
        fail(errors, isinstance(required_outputs, list) and len(required_outputs) > 0, f"{slice_id}: required outputs missing")
        fail(errors, isinstance(exit_evidence, list) and len(exit_evidence) > 0, f"{slice_id}: exit evidence missing")
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{slice_id}: unknown package {package}")
            delivery_packages.add(package)
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, slice_id)
    fail(errors, delivery_packages == packages, f"packages absent from delivery slices: {sorted(packages - delivery_packages)}")

    # Manifest count and authority closure.
    count_checks = {
        "architecture_section_count": len(section_rows),
        "architecture_invariant_count": len(invariant_rows),
        "capability_cell_count": len(capability_rows),
        "shared_port_count": len(port_rows),
        "configuration_section_count": len(config_rows),
        "p00_schema_or_registry_count": schema_total,
        "recipe_count": len(recipe_rows),
        "delivery_slice_count": len(delivery_rows),
        "module_packet_count": len(module_rows),
        "package_assignment_task_count": len(assignment_paths),
    }
    for key, actual in count_checks.items():
        fail(errors, manifest.get(key) == actual, f"coverage manifest count mismatch for {key}: {manifest.get(key)} != {actual}")
    fail(errors, manifest.get("type_registry_named_symbol_count") == len(type_registry_symbols), "coverage manifest TYPE_REGISTRY count mismatch")
    fail(errors, manifest.get("named_type_completion_count") == len(completion_symbols), "coverage manifest completion count mismatch")
    fail(errors, manifest.get("canonical_primitive_family_count") == primitive_families, "coverage manifest primitive family count mismatch")
    fail(errors, manifest.get("architecture_section_sha256") == p00_manifest.get("architecture_sha256"), "coverage/P00 architecture digest mismatch")
    fail(errors, manifest.get("architecture_section_sha256") == package_doc.get("architecture_sha256"), "coverage/package registry architecture digest mismatch")
    for key in (
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ):
        fail(errors, manifest.get(key) is False, f"coverage manifest authority flag {key}")

    fail(errors, launch.get("active_stage") == "P00" and launch.get("active_wave") == 0, "launch must remain P00/W0")
    fail(errors, launch.get("authorized_packages") == ["search-contracts"], "only search-contracts may be authorized")

    workflow = ROOT / ".github/workflows/architecture-coverage.yml"
    if not workflow.is_file():
        errors.append("architecture coverage workflow missing")
    else:
        workflow_text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false", "validate-architecture-coverage.py"):
            fail(errors, token in workflow_text, f"coverage workflow missing {token}")
        for token in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:", "\n  repository_dispatch:"):
            fail(errors, token not in workflow_text, f"automatic coverage workflow trigger {token.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(packages),
        "assignments": len(assignment_paths),
        "function_sources": len(function_rows),
        "derived_package_qualified_operations": operation_count,
        "modules": module_total,
        "architecture_sections": len(section_rows),
        "capability_cells": len(capability_rows),
        "invariants": len(invariant_rows),
        "ports": len(port_rows),
        "configuration_sections": len(config_rows),
        "schema_and_type_symbols": schema_total,
        "recipes": len(recipe_rows),
        "reason_codes": len(registry_reason_codes),
        "delivery_slices": len(delivery_rows),
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "warnings": warnings,
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
