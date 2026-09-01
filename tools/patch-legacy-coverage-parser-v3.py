#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEGACY = ROOT / "tools/validate-architecture-coverage.py"
GRAPH = ROOT / "tools/coverage_graph_v2.py"
MAPS = ROOT / "tools/package_maps_v2.py"
MAP_VALIDATOR = ROOT / "tools/validate-package-maps-v2.py"


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


def patch_legacy_validator() -> None:
    replace_once(
        LEGACY,
        'source_invariants = set(re.findall(r"^(INV-\\d{2}):", architecture, flags=re.MULTILINE))',
        'source_invariants = set(re.findall(r"^\\s*(INV-\\d{2}):", architecture, flags=re.MULTILINE))',
        "indented invariant parser",
    )
    replace_once(
        LEGACY,
        'heading_matches = list(re.finditer(r"^### `([A-Za-z][A-Za-z0-9]+Port)`\\s*$", port_source, flags=re.MULTILINE))',
        'heading_matches = list(re.finditer(r"^### `([A-Za-z][A-Za-z0-9]+Port)`(?:[ \\t]+[—-].*)?[ \\t]*$", port_source, flags=re.MULTILINE))',
        "port heading suffix parser",
    )
    old_port = '''    for port_name, row in port_rows.items():
        fail(errors, row.get("methods") == source_port_methods.get(port_name), f"{port_name}: method inventory mismatch")
        package = row.get("implementation_package")
        module = row.get("implementation_module")
        validate_owner_pair(errors, package, module, modules, port_name, allow_none=False)
'''
    new_port = '''    fail(errors, port_doc.get("schema_version") == 2, "port registry must be schema v2")
    fail(errors, port_doc.get("method_count") == sum(len(value) for value in source_port_methods.values()), "port method total mismatch")
    for port_name, row in port_rows.items():
        methods = row.get("methods")
        method_modules = row.get("method_modules")
        fail(errors, methods == source_port_methods.get(port_name), f"{port_name}: method inventory mismatch")
        fail(errors, isinstance(method_modules, list) and len(method_modules) == len(methods if isinstance(methods, list) else []), f"{port_name}: one method module per method required")
        package = row.get("implementation_package")
        module = row.get("implementation_module")
        validate_owner_pair(errors, package, module, modules, port_name, allow_none=False)
        for method_name, method_module in zip(methods if isinstance(methods, list) else [], method_modules if isinstance(method_modules, list) else []):
            validate_owner_pair(errors, package, method_module, modules, f"{port_name}.{method_name}", allow_none=False)
'''
    replace_once(LEGACY, old_port, new_port, "port method-module validation")
    replace_once(
        LEGACY,
        'for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false", "validate-architecture-coverage.py"):',
        'for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false", "validate-architecture-coverage"):',
        "workflow wrapper validation",
    )


def patch_graph_port_relations() -> None:
    old = '''    for row in rows(load(manifest["port_registry"]), "port", "name").values():
        relation_counts[f"{row['implementation_package']}:{row['implementation_module']}"]["port_relations"] += 1
'''
    new = '''    for row in rows(load(manifest["port_registry"]), "port", "name").values():
        package = row["implementation_package"]
        relation_counts[f"{package}:{row['implementation_module']}"]["port_relations"] += 1
        for method_module in row.get("method_modules", []):
            relation_counts[f"{package}:{method_module}"]["port_method_relations"] += 1
'''
    replace_once(GRAPH, old, new, "port method relation aggregation")

    old_row = '''                    "port_relation_count": counts.get("port_relations", 0),
                    "schema_relation_count": counts.get("schema_relations", 0),
'''
    new_row = '''                    "port_relation_count": counts.get("port_relations", 0),
                    "port_method_relation_count": counts.get("port_method_relations", 0),
                    "schema_relation_count": counts.get("schema_relations", 0),
'''
    replace_once(GRAPH, old_row, new_row, "module port method count")

    old_render = '''                f"port_relation_count = {row['port_relation_count']}",
                f"schema_relation_count = {row['schema_relation_count']}",
'''
    new_render = '''                f"port_relation_count = {row['port_relation_count']}",
                f"port_method_relation_count = {row['port_method_relation_count']}",
                f"schema_relation_count = {row['schema_relation_count']}",
'''
    replace_once(GRAPH, old_render, new_render, "module port method rendering")


def patch_map_port_relations() -> None:
    old = '''                "module": row.get("implementation_module"),
                "methods": string_list(row.get("methods")),
'''
    new = '''                "module": row.get("implementation_module"),
                "methods": string_list(row.get("methods")),
                "method_modules": string_list(row.get("method_modules")),
'''
    replace_once(MAPS, old, new, "package map port method modules")

    old_overview = '''                f"port_relation_count = {module['port_relation_count']}",
                f"schema_relation_count = {module['schema_relation_count']}",
'''
    new_overview = '''                f"port_relation_count = {module['port_relation_count']}",
                f"port_method_relation_count = {module['port_method_relation_count']}",
                f"schema_relation_count = {module['schema_relation_count']}",
'''
    replace_once(MAPS, old_overview, new_overview, "package overview port method count")

    old_port_render = '''                f"module = {q(str(row['module']))}",
                f"methods = {arr(row['methods'])}",
'''
    new_port_render = '''                f"module = {q(str(row['module']))}",
                f"methods = {arr(row['methods'])}",
                f"method_modules = {arr(row['method_modules'])}",
'''
    replace_once(MAPS, old_port_render, new_port_render, "package relation port method modules")


def patch_graph_validator_fields() -> None:
    old = '''        "specific_documentation_node_count", "architecture_relation_count", "port_relation_count",
        "schema_relation_count", "configuration_relation_count", "recipe_relation_count",
'''
    new = '''        "specific_documentation_node_count", "architecture_relation_count", "port_relation_count",
        "port_method_relation_count", "schema_relation_count", "configuration_relation_count", "recipe_relation_count",
'''
    replace_once(ROOT / "tools/validate-coverage-graph-v2.py", old, new, "module port method validator field")


def patch_package_map_validator() -> None:
    marker = '''    # Every implementation module is related; structural-only modules require explicit rationale.
'''
    block = '''    # Port methods are source-exact and each enters one package-local module.
    port_doc = read_toml(str(manifest.get("port_registry")), errors)
    port_rows = rows(port_doc, "port", "name") if port_doc else {}
    fail(errors, port_doc.get("schema_version") == 2, "port registry must be schema v2")
    fail(errors, port_doc.get("port_count") == len(port_rows) == 23, "port registry count mismatch")
    fail(errors, port_doc.get("method_count") == sum(len(row.get("methods", [])) for row in port_rows.values()), "port method count mismatch")
    valid_module_refs = {row["id"] for row in graph["module_rows"]}
    for port_name, row in port_rows.items():
        methods = row.get("methods")
        method_modules = row.get("method_modules")
        package = row.get("implementation_package")
        fail(errors, isinstance(methods, list) and isinstance(method_modules, list) and len(methods) == len(method_modules), f"{port_name}: one method module per method required")
        for method_name, method_module in zip(methods if isinstance(methods, list) else [], method_modules if isinstance(method_modules, list) else []):
            fail(errors, f"{package}:{method_module}" in valid_module_refs, f"{port_name}.{method_name}: invalid package-local method module")

'''
    text = MAP_VALIDATOR.read_text(encoding="utf-8")
    if text.count(marker) != 1:
        raise RuntimeError("package map validator port marker missing")
    MAP_VALIDATOR.write_text(text.replace(marker, block + marker, 1), encoding="utf-8", newline="\n")


def main() -> int:
    patch_legacy_validator()
    patch_graph_port_relations()
    patch_map_port_relations()
    patch_graph_validator_fields()
    patch_package_map_validator()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
