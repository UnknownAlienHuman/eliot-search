from __future__ import annotations

import re
from typing import Any

from .common import (
    ROOT,
    exact_type_registry_symbols,
    load,
    normalize_type_name,
    read,
    require,
    rows,
    top_level_yaml_labels,
    validate_owner_pair,
)

_COMPLETIONS = {"RecipeIdV1", "RecipeBodyV1", "ComparisonAxis", "ProtocolRange", "PackageOpaque"}


def validate_schemas(errors: list[str], topology: dict[str, Any]) -> dict[str, Any]:
    manifest = topology["manifest"]
    modules = topology["modules"]
    packages = topology["packages"]
    schema_doc = load(manifest["schema_registry"])
    recipe_doc = load(manifest["recipe_registry"])
    reason_doc = load(manifest["reason_registry"])

    packet_rows = schema_doc.get("packet")
    if not isinstance(packet_rows, list):
        errors.append("schema packet registry missing")
        packet_rows = []

    schema_names: set[str] = set()
    schema_total = 0
    primitive_families = 0
    primitive_registered: set[str] = set()

    for packet in packet_rows:
        if not isinstance(packet, dict) or not isinstance(packet.get("path"), str):
            errors.append("invalid schema packet entry")
            continue
        path = packet["path"]
        require(errors, (ROOT / path).is_file(), f"missing schema packet {path}")
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
            group_id = group.get("id")
            names = group.get("schemas")
            require(errors, isinstance(names, list) and len(names) > 0, f"{path}:{group_id}: schemas missing")
            if not isinstance(names, list):
                continue
            for name in names:
                require(errors, isinstance(name, str) and name != "", f"{path}:{group_id}: invalid schema name")
                if not isinstance(name, str):
                    continue
                require(errors, name not in schema_names, f"duplicate schema/type name {name}")
                schema_names.add(name)
                packet_count += 1
                if packet.get("id") == "primitives":
                    primitive_registered.add(name)

            source_files = group.get("source_files")
            require(errors, isinstance(source_files, list) and len(source_files) > 0, f"{path}:{group_id}: source files missing")
            combined = ""
            for source in source_files if isinstance(source_files, list) else []:
                require(errors, isinstance(source, str) and (ROOT / source).is_file(), f"{path}:{group_id}: missing schema source {source}")
                if isinstance(source, str) and (ROOT / source).is_file():
                    combined += "\n" + read(source)
            for name in names:
                if isinstance(name, str):
                    require(errors, normalize_type_name(name) in combined, f"{path}:{group_id}: {name} absent from declared sources")

            validate_owner_pair(errors, group.get("shape_owner_package"), group.get("shape_owner_module"), modules, f"schema {group_id}")
            validate_owner_pair(errors, group.get("meaning_owner_package"), group.get("meaning_owner_module"), modules, f"schema {group_id}")
            validate_owner_pair(errors, group.get("state_owner_package"), group.get("state_owner_module"), modules, f"schema {group_id}")
            secondary = group.get("secondary_state_owner_packages", [])
            require(errors, isinstance(secondary, list), f"schema {group_id}: secondary owners must be an array")
            for package in secondary if isinstance(secondary, list) else []:
                require(errors, package in packages, f"schema {group_id}: unknown secondary owner {package}")

        require(errors, document.get("schema_count") == packet_count, f"{path}: schema count mismatch")
        require(errors, packet.get("schema_count") == packet_count, f"{path}: summary schema count mismatch")
        schema_total += packet_count
        families = document.get("primitive_family", [])
        require(errors, isinstance(families, list), f"{path}: primitive_family must be an array")
        primitive_families += len(families) if isinstance(families, list) else 0

    require(errors, schema_total == schema_doc.get("schema_or_registry_count") == 217, "schema/type total must be 217")
    require(errors, schema_doc.get("type_registry_named_symbol_count") == 115, "TYPE_REGISTRY symbol count must be 115")
    require(errors, schema_doc.get("completion_symbol_count") == 5, "type completion count must be 5")
    require(errors, primitive_families == schema_doc.get("canonical_primitive_family_count") == 12, "canonical primitive family count must be 12")

    type_registry_symbols = exact_type_registry_symbols(read("docs/contracts/p00/TYPE_REGISTRY.md"))
    require(errors, len(type_registry_symbols) == 115, f"TYPE_REGISTRY derived symbol count is {len(type_registry_symbols)}, expected 115")
    require(errors, type_registry_symbols | _COMPLETIONS == primitive_registered, f"primitive registry mismatch: {sorted((type_registry_symbols | _COMPLETIONS) ^ primitive_registered)}")

    completion_text = read("docs/contracts/p00/TYPE_COMPLETIONS.md")
    for symbol in sorted(_COMPLETIONS):
        require(errors, symbol in completion_text, f"TYPE_COMPLETIONS missing {symbol}")
    p00_manifest = load("docs/contracts/p00/manifest.toml")
    required_files = p00_manifest.get("required_files")
    require(errors, isinstance(required_files, list) and len(required_files) == 13, "P00 required_files must contain 13 entries")
    require(errors, isinstance(required_files, list) and "TYPE_COMPLETIONS.md" in required_files, "P00 manifest does not include TYPE_COMPLETIONS.md")

    source_schema_files = [
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/SOURCE_GRAPH.md",
        "docs/contracts/p00/RECIPES.md",
        "docs/contracts/p00/QUERY_AND_RESULTS.md",
        "docs/contracts/p00/RECIPE_RESULTS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
    ]
    source_labels: set[str] = set()
    for path in source_schema_files:
        source_labels.update(top_level_yaml_labels(read(path)))

    recipes_text = read("docs/contracts/p00/RECIPES.md")
    recipe_ids = set(
        re.findall(
            r"^(?:-\s*)?([a-z][a-z0-9_]+@1)(?::)?\s*$",
            recipes_text,
            flags=re.MULTILINE,
        )
    )
    require(errors, recipe_ids == {
        "locate@1",
        "find_text@1",
        "inspect_entity@1",
        "compare_implementations@1",
        "explore_entity@1",
        "corpus_profile@1",
        "corpus_delta@1",
        "provenance@1",
        "compile_exact_scan@1",
        "execute_exact_scan@1",
        "expand_handle@1",
    }, "RECIPES.md exact recipe set mismatch")

    unregistered_labels = {
        label for label in source_labels
        if label not in schema_names and label not in recipe_ids
    }
    require(errors, not unregistered_labels, f"unregistered top-level P00 schema labels: {sorted(unregistered_labels)}")

    recipe_rows = rows(recipe_doc, "recipe", "id")
    require(errors, len(recipe_rows) == 11, "recipe registry must contain 11 recipes")
    require(errors, set(recipe_rows) == recipe_ids, f"recipe registry mismatch: {sorted(set(recipe_rows) ^ recipe_ids)}")
    for recipe_id, row in recipe_rows.items():
        require(errors, row.get("request_schema") in schema_names, f"{recipe_id}: unknown request schema")
        require(errors, row.get("result_schema") in schema_names, f"{recipe_id}: unknown result schema")
        owners = row.get("primary_execution_packages")
        require(errors, isinstance(owners, list) and len(owners) > 0, f"{recipe_id}: execution owners missing")
        for package in owners if isinstance(owners, list) else []:
            require(errors, package in packages, f"{recipe_id}: unknown execution package {package}")

    reason_text = read("docs/contracts/p00/REASON_CODES.md")
    source_reason_codes = set(re.findall(r"^([A-Z][A-Z0-9_]+)$", reason_text, flags=re.MULTILINE))
    registry_reason_codes: set[str] = set()
    expected_sizes = {
        "search_reason_codes": 31,
        "protocol_error_codes": 10,
        "contract_error_codes": 10,
    }
    for key, size in expected_sizes.items():
        values = reason_doc.get(key, {}).get("values")
        require(errors, isinstance(values, list), f"reason registry {key} missing")
        require(errors, isinstance(values, list) and len(values) == size, f"reason registry {key} count mismatch")
        registry_reason_codes.update(values if isinstance(values, list) else [])
    require(errors, source_reason_codes == registry_reason_codes, f"reason code registry mismatch: {sorted(source_reason_codes ^ registry_reason_codes)}")
    require(errors, len(registry_reason_codes) == 51, "reason code total must be 51")

    return {
        "schema_total": schema_total,
        "type_registry_symbols": len(type_registry_symbols),
        "completion_symbols": len(_COMPLETIONS),
        "primitive_families": primitive_families,
        "recipe_count": len(recipe_rows),
        "reason_count": len(registry_reason_codes),
    }
