#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import tomllib
from collections import Counter
from pathlib import Path
from typing import Any

from coverage_graph_v2 import ROOT, build_graph, load, rows
from package_maps_v2 import (
    DOC_INDEX_PATH,
    HUMAN_INDEX_PATH,
    INDEX_PATH,
    INTEGRATION_PATH,
    MAP_ROOT,
    build_outputs,
    dependency_cycle,
    exact_internal_dependencies,
    package_paths,
)


def fail(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def read_toml(path: str, errors: list[str]) -> dict[str, Any]:
    try:
        return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"{path}: {exc}")
        return {}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.parse_args()

    errors: list[str] = []
    warnings: list[str] = []
    try:
        graph = build_graph()
        expected_outputs, stats = build_outputs()
    except Exception as exc:
        print(json.dumps({"status": "FAIL", "errors": [f"map derivation failed: {exc}"]}, indent=2))
        return 1

    # Exact generated-file closure.
    for relative, expected in sorted(expected_outputs.items()):
        path = ROOT / relative
        fail(errors, path.is_file(), f"missing generated map {relative}")
        if path.is_file():
            actual = path.read_text(encoding="utf-8")
            fail(errors, actual == expected, f"stale generated map {relative}")
            fail(errors, actual.count("\n") + 1 < 10000, f"map exceeds 10k lines: {relative}")
    expected_package_files = {
        path for path in expected_outputs if path.startswith(MAP_ROOT + "/")
    }
    actual_package_files = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / MAP_ROOT).rglob("*.toml")
    } if (ROOT / MAP_ROOT).exists() else set()
    fail(errors, actual_package_files == expected_package_files, f"orphan/missing package maps: {sorted(actual_package_files ^ expected_package_files)}")

    manifest = read_toml("swarm/coverage/manifest.toml", errors)
    fail(errors, manifest.get("package_map_index") == INDEX_PATH, "manifest package-map index mismatch")
    fail(errors, manifest.get("documentation_file_index") == DOC_INDEX_PATH, "manifest documentation index mismatch")
    fail(errors, manifest.get("integration_documentation_map") == INTEGRATION_PATH, "manifest integration map mismatch")
    fail(errors, manifest.get("package_map_count") == len(graph["package_rows"]) == 45, "package-map count mismatch")
    fail(errors, manifest.get("package_map_file_count") == len(graph["package_rows"]) * 4, "package-map file count mismatch")
    fail(errors, manifest.get("weak_logical_module_count") == 0, "weak module count must be zero")

    index = read_toml(INDEX_PATH, errors)
    index_rows = rows(index, "package", "name") if index else {}
    fail(errors, set(index_rows) == set(graph["package_rows"]), "package-map index package closure mismatch")
    fail(errors, index.get("package_count") == 45, "package-map index count mismatch")
    fail(errors, index.get("map_file_count") == 180, "package-map index file count mismatch")
    fail(errors, index.get("operation_count") == len(graph["operation_rows"]), "package-map operation count mismatch")
    fail(errors, index.get("documentation_node_count") == len(graph["documentation_rows"]), "package-map documentation count mismatch")
    fail(errors, index.get("dependency_edge_count") == len(graph["dependency_rows"]), "package-map dependency count mismatch")
    fail(errors, index.get("logical_module_count") == len(graph["module_rows"]), "package-map module count mismatch")

    # Workspace and Cargo dependency parity.
    root_cargo = read_toml("Cargo.toml", errors)
    workspace = root_cargo.get("workspace", {}) if root_cargo else {}
    members = set(workspace.get("members", [])) if isinstance(workspace, dict) else set()
    package_paths_set = {row["path"] for row in graph["package_rows"].values()}
    fail(errors, members == package_paths_set, f"workspace member/package registry mismatch: {sorted(members ^ package_paths_set)}")

    workspace_dependencies = root_cargo.get("workspace", {}).get("dependencies", {}) if isinstance(root_cargo.get("workspace"), dict) else {}
    # TOML dotted tables are represented at root as workspace.dependencies by tomllib only when written inline;
    # the current root manifest uses [workspace.dependencies], which is nested under workspace.
    if not isinstance(workspace_dependencies, dict):
        workspace_dependencies = {}
    library_packages = {name for name, row in graph["package_rows"].items() if row.get("kind") == "lib"}
    fail(errors, set(workspace_dependencies) == library_packages, f"workspace dependency/package library mismatch: {sorted(set(workspace_dependencies) ^ library_packages)}")
    for package, row in graph["package_rows"].items():
        manifest_path = ROOT / row["path"] / "Cargo.toml"
        fail(errors, manifest_path.is_file(), f"{package}: Cargo.toml missing")
        if not manifest_path.is_file():
            continue
        package_manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        fail(errors, package_manifest.get("package", {}).get("name") == package, f"{package}: Cargo package name mismatch")
        declared = set(row.get("deps", []))
        cargo_internal = exact_internal_dependencies(manifest_path, set(graph["package_rows"]))
        fail(errors, cargo_internal == declared, f"{package}: Cargo/registry dependency mismatch: cargo={sorted(cargo_internal)} registry={sorted(declared)}")
        consumer_wave = int(row.get("wave", 0))
        stage_readsets = read_toml("swarm/stage-readsets.toml", errors)
        reentry_overrides = {
            item.get("id"): item
            for item in stage_readsets.get("override", [])
            if isinstance(item, dict) and isinstance(item.get("id"), str)
        }
        for producer in declared:
            producer_wave = int(graph["package_rows"][producer].get("wave", 0))
            if producer_wave > consumer_wave:
                override_id = f"W{producer_wave}.{package}"
                override = reentry_overrides.get(override_id, {})
                fail(errors, override.get("package") == package, f"{package}: later-wave dependency {producer} lacks exact {override_id} reentry")
                fail(errors, override.get("wave") == producer_wave, f"{package}: {override_id} wave mismatch")
                fail(errors, override.get("replace_previous_stage_context") is True, f"{package}: {override_id} must replace prior context")
                fail(errors, override.get("accepted_prior_stage_handoff_only") is True, f"{package}: {override_id} must consume accepted handoff only")
                fail(errors, override.get("dependency_implementation_reads_allowed") is False, f"{package}: {override_id} may not read dependency implementation")
    cycle = dependency_cycle(graph["package_rows"])
    fail(errors, not cycle, f"package dependency cycle: {cycle}")

    # Exact operation, module and document coverage in package-local maps.
    operation_occurrences: Counter[str] = Counter()
    module_occurrences: Counter[str] = Counter()
    document_occurrences: Counter[tuple[str, str]] = Counter()
    product_docs = [row for row in graph["documentation_rows"] if row["packages"]]

    for package in sorted(graph["package_rows"]):
        paths = package_paths(package)
        overview = read_toml(paths["overview"], errors)
        operations = read_toml(paths["operations"], errors)
        documents = read_toml(paths["documents"], errors)
        relations = read_toml(paths["relations"], errors)
        fail(errors, overview.get("package") == package, f"{package}: overview package mismatch")
        fail(errors, operations.get("package") == package, f"{package}: operations map package mismatch")
        fail(errors, documents.get("package") == package, f"{package}: documents map package mismatch")
        fail(errors, relations.get("package") == package, f"{package}: relations map package mismatch")
        fail(errors, overview.get("operations_map") == paths["operations"], f"{package}: operations link mismatch")
        fail(errors, overview.get("documents_map") == paths["documents"], f"{package}: documents link mismatch")
        fail(errors, overview.get("relations_map") == paths["relations"], f"{package}: relations link mismatch")

        for row in overview.get("module", []) if isinstance(overview.get("module"), list) else []:
            if isinstance(row, dict) and isinstance(row.get("name"), str):
                module_occurrences[f"{package}:{row['name']}"] += 1
        for row in operations.get("operation", []) if isinstance(operations.get("operation"), list) else []:
            if not isinstance(row, dict) or not isinstance(row.get("id"), str):
                continue
            operation_occurrences[row["id"]] += 1
            fail(errors, row.get("route_kind") not in {"public_facade", "semantic_low"}, f"{row['id']}: unreviewed operation route")
            fail(errors, f"{package}:{row.get('module')}" in {entry["id"] for entry in graph["module_rows"]}, f"{row['id']}: package map module invalid")
        for row in documents.get("node", []) if isinstance(documents.get("node"), list) else []:
            if isinstance(row, dict) and isinstance(row.get("id"), str):
                document_occurrences[(package, row["id"])] += 1
                refs = row.get("modules", [])
                fail(errors, isinstance(refs, list) and bool(refs), f"{package}:{row['id']}: document route empty")
                for ref in refs if isinstance(refs, list) else []:
                    fail(errors, isinstance(ref, str) and ref.startswith(package + ":"), f"{package}:{row['id']}: foreign module in package map")

    expected_operations = {row["id"] for row in graph["operation_rows"]}
    expected_modules = {row["id"] for row in graph["module_rows"]}
    fail(errors, set(operation_occurrences) == expected_operations, f"operation package-map closure mismatch: {sorted(set(operation_occurrences) ^ expected_operations)}")
    fail(errors, all(count == 1 for count in operation_occurrences.values()), "operation appears in more than one package map")
    fail(errors, set(module_occurrences) == expected_modules, f"module package-map closure mismatch: {sorted(set(module_occurrences) ^ expected_modules)}")
    fail(errors, all(count == 1 for count in module_occurrences.values()), "module appears in more than one package overview")
    for row in product_docs:
        for package in row["packages"]:
            fail(errors, document_occurrences[(package, row["id"])] == 1, f"{row['id']}: missing/duplicate document route in {package}")

    # Integration nodes are explicit and cannot conceal product semantics.
    integration = read_toml(INTEGRATION_PATH, errors)
    integration_ids = {
        row.get("id") for row in integration.get("node", [])
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    expected_integration_ids = {row["id"] for row in graph["documentation_rows"] if not row["packages"]}
    fail(errors, integration_ids == expected_integration_ids, "integration documentation map closure mismatch")
    for row in graph["documentation_rows"]:
        if row["id"] in integration_ids:
            fail(errors, row["kind"] in {"governance", "navigation"}, f"{row['id']}: product-bearing node misclassified as integration")

    # Port methods are source-exact and each enters one package-local module.
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

    # Every implementation module is related; structural-only modules require explicit rationale.
    fail(errors, not graph["weak_modules"], f"weak implementation modules remain: {graph['weak_modules']}")
    for row in graph["module_rows"]:
        if row["role"] in {"public_entry", "structural_boundary", "structural_support"}:
            fail(errors, bool(str(row.get("structural_rationale", "")).strip()), f"{row['id']}: structural rationale missing")

    # Permanent validator must be manual and read-only.
    workflow_path = ROOT / ".github/workflows/package-map-coverage-v2.yml"
    fail(errors, workflow_path.is_file(), "permanent package-map workflow missing")
    if workflow_path.is_file():
        workflow_text = workflow_path.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            fail(errors, token in workflow_text, f"permanent workflow missing {token}")
        for token in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:", "\n  repository_dispatch:"):
            fail(errors, token not in workflow_text, f"permanent workflow has automatic trigger {token.strip()}")

    for key in (
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ):
        fail(errors, manifest.get(key) is False, f"manifest authority changed: {key}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(graph["package_rows"]),
        "package_map_files": len(expected_package_files),
        "logical_modules": len(graph["module_rows"]),
        "operations": len(graph["operation_rows"]),
        "documentation_nodes": len(graph["documentation_rows"]),
        "product_documentation_nodes": len(product_docs),
        "integration_documentation_nodes": len(expected_integration_ids),
        "dependency_edges": len(graph["dependency_rows"]),
        "dependency_cycles": cycle,
        "weak_modules": graph["weak_modules"],
        "warnings": warnings,
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
