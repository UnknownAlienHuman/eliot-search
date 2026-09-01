#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import tomllib
from pathlib import Path
from typing import Any

from coverage_graph_v2 import ROOT, build_graph, rows


def load(path: str, errors: list[str]) -> dict[str, Any]:
    try:
        return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"{path}: {exc}")
        return {}


def fail(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def ref_package(ref: str) -> str:
    return ref.split(":", 1)[0] if ":" in ref else ""


def validate_owner_modules(
    errors: list[str],
    owner: str,
    refs: Any,
    declared: set[str],
    required: set[str],
    valid_refs: set[str],
) -> None:
    fail(errors, isinstance(refs, list) and bool(refs), f"{owner}: module refs missing")
    if not isinstance(refs, list):
        return
    packages: set[str] = set()
    for ref in refs:
        fail(errors, isinstance(ref, str) and ref in valid_refs, f"{owner}: invalid module ref {ref}")
        if isinstance(ref, str):
            packages.add(ref_package(ref))
    fail(errors, required <= packages, f"{owner}: declared owner lacks module route {sorted(required - packages)}")
    fail(errors, packages <= declared, f"{owner}: module route names undeclared package {sorted(packages - declared)}")


def canonical_rows(document: dict[str, Any], key: str, id_key: str, fields: list[str]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for identity, row in rows(document, key, id_key).items():
        result[identity] = {field: row.get(field) for field in fields}
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.parse_args()

    errors: list[str] = []
    warnings: list[str] = []
    try:
        graph = build_graph()
    except Exception as exc:
        print(json.dumps({"status": "FAIL", "errors": [f"graph derivation failed: {exc}"]}, indent=2))
        return 1

    manifest = load("swarm/coverage/manifest.toml", errors)
    operation_doc = load("swarm/coverage/operation-modules.toml", errors)
    documentation_doc = load("swarm/coverage/documentation-nodes.toml", errors)
    dependency_doc = load("swarm/coverage/dependency-edges.toml", errors)
    module_doc = load("swarm/coverage/module-coverage.toml", errors)
    config_doc = load("config/sections.toml", errors)
    recipe_doc = load("swarm/coverage/recipes.toml", errors)

    required_manifest_paths = {
        "operation_module_registry": "swarm/coverage/operation-modules.toml",
        "documentation_node_registry": "swarm/coverage/documentation-nodes.toml",
        "dependency_edge_registry": "swarm/coverage/dependency-edges.toml",
        "module_coverage_registry": "swarm/coverage/module-coverage.toml",
    }
    for key, value in required_manifest_paths.items():
        fail(errors, manifest.get(key) == value, f"coverage manifest path mismatch: {key}")
    fail(errors, manifest.get("schema_version") == 2, "coverage manifest must be schema v2")
    fail(errors, manifest.get("exact_operation_module_count") == len(graph["operation_rows"]), "operation count mismatch in manifest")
    fail(errors, manifest.get("documentation_source_file_count") == len(graph["selected_markdown"]), "documentation source count mismatch in manifest")
    fail(errors, manifest.get("documentation_node_count") == len(graph["documentation_rows"]), "documentation node count mismatch in manifest")
    fail(errors, manifest.get("dependency_edge_count") == len(graph["dependency_rows"]), "dependency edge count mismatch in manifest")
    fail(errors, manifest.get("logical_module_count") == len(graph["module_rows"]), "logical module count mismatch in manifest")
    fail(errors, manifest.get("weak_logical_module_count") == len(graph["weak_modules"]), "weak module count mismatch in manifest")

    operation_fields = [
        "package", "operation", "module", "public_entry_module", "sources", "source_contexts", "route_kind", "score"
    ]
    expected_operations = {row["id"]: {field: row.get(field) for field in operation_fields} for row in graph["operation_rows"]}
    actual_operations = canonical_rows(operation_doc, "operation", "id", operation_fields)
    fail(errors, actual_operations == expected_operations, f"operation-module registry stale or divergent: expected {len(expected_operations)}, actual {len(actual_operations)}")
    fail(errors, operation_doc.get("operation_count") == len(expected_operations), "operation registry count mismatch")

    valid_refs = {row["id"] for row in graph["module_rows"]}
    public_entries = graph["public_entries"]
    public_facades = 0
    semantic_low = 0
    for identity, row in actual_operations.items():
        package = row.get("package")
        module = row.get("module")
        fail(errors, isinstance(package, str) and isinstance(module, str) and f"{package}:{module}" in valid_refs, f"{identity}: invalid routed module")
        fail(errors, isinstance(package, str) and identity.startswith(package + "::"), f"{identity}: package identity mismatch")
        if row.get("route_kind") == "public_facade":
            public_facades += 1
            fail(errors, module == public_entries.get(package), f"{identity}: facade route does not use public entry")
        if row.get("route_kind") == "semantic_low":
            semantic_low += 1
    fail(errors, public_facades == 0, f"unreviewed public-entry operation routes remain: {public_facades}")
    fail(errors, semantic_low == 0, f"low-confidence operation routes remain: {semantic_low}")

    documentation_fields = [
        "path", "line", "level", "heading", "kind", "packages", "modules", "route_kind", "rationale"
    ]
    expected_docs = {row["id"]: {field: row.get(field) for field in documentation_fields} for row in graph["documentation_rows"]}
    actual_docs = canonical_rows(documentation_doc, "node", "id", documentation_fields)
    fail(errors, actual_docs == expected_docs, f"documentation-node registry stale or divergent: expected {len(expected_docs)}, actual {len(actual_docs)}")
    fail(errors, documentation_doc.get("node_count") == len(expected_docs), "documentation registry node count mismatch")
    fail(errors, documentation_doc.get("source_file_count") == len(graph["selected_markdown"]), "documentation registry source count mismatch")
    implementation_kinds = {
        "implementation_contract_root", "implementation_operation", "implementation_error_contract",
        "principle_or_invariant", "implementation_contract", "architecture_root", "architecture_section",
        "architecture_node", "delivery_contract", "configuration_contract", "qualification_contract",
        "implementation_handoff",
    }
    principle_count = 0
    governance_count = 0
    for identity, row in actual_docs.items():
        refs = row.get("modules")
        packages = row.get("packages")
        kind = row.get("kind")
        if kind in implementation_kinds:
            fail(errors, isinstance(refs, list) and bool(refs), f"{identity}: implementation-bearing node lacks module route")
            if isinstance(refs, list):
                for ref in refs:
                    fail(errors, ref in valid_refs, f"{identity}: invalid documentation module {ref}")
                fail(errors, sorted({ref_package(ref) for ref in refs}) == sorted(packages or []), f"{identity}: documentation package/module mismatch")
        else:
            governance_count += 1
            fail(errors, kind in {"governance", "navigation"}, f"{identity}: unknown nonimplementation node kind {kind}")
            fail(errors, refs == [] and packages == [], f"{identity}: governance/navigation node must not claim product module")
            fail(errors, bool(row.get("rationale")), f"{identity}: non-crate rationale missing")
        if kind == "principle_or_invariant":
            principle_count += 1
    fail(errors, principle_count > 0, "no principles or invariants were classified")

    dependency_fields = [
        "consumer", "consumer_module", "consumer_earliest_wave", "producer", "producer_module",
        "producer_earliest_wave", "relationship", "contract_source", "cargo_manifest", "route_kind",
        "requires_stage_reentry", "reentry_stage", "exact_accepted_handoff_required",
    ]
    expected_dependencies = {row["id"]: {field: row.get(field) for field in dependency_fields} for row in graph["dependency_rows"]}
    actual_dependencies = canonical_rows(dependency_doc, "edge", "id", dependency_fields)
    fail(errors, actual_dependencies == expected_dependencies, f"dependency-edge registry stale or divergent: expected {len(expected_dependencies)}, actual {len(actual_dependencies)}")
    fail(errors, dependency_doc.get("edge_count") == len(expected_dependencies), "dependency edge count mismatch")
    stage_readsets = load("swarm/stage-readsets.toml", errors)
    reentry_overrides = {
        row.get("id"): row
        for row in stage_readsets.get("override", [])
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    for identity, row in actual_dependencies.items():
        consumer_ref = f"{row.get('consumer')}:{row.get('consumer_module')}"
        producer_ref = f"{row.get('producer')}:{row.get('producer_module')}"
        fail(errors, consumer_ref in valid_refs, f"{identity}: invalid consumer module")
        fail(errors, producer_ref in valid_refs, f"{identity}: invalid producer module")
        fail(errors, row.get("producer_module") == public_entries.get(row.get("producer")), f"{identity}: dependency must enter producer public boundary")
        requires_reentry = row.get("requires_stage_reentry") is True
        if requires_reentry:
            expected_stage = f"W{row.get('producer_earliest_wave')}"
            expected_override = f"{expected_stage}.{row.get('consumer')}"
            override = reentry_overrides.get(expected_override, {})
            fail(errors, row.get("reentry_stage") == expected_stage, f"{identity}: reentry stage mismatch")
            fail(errors, row.get("relationship") == "progressive_reentry_handoff", f"{identity}: later-wave edge must be progressive reentry")
            fail(errors, override.get("package") == row.get("consumer"), f"{identity}: exact reentry override missing")
            fail(errors, override.get("wave") == row.get("producer_earliest_wave"), f"{identity}: reentry override wave mismatch")
            fail(errors, override.get("replace_previous_stage_context") is True, f"{identity}: reentry must replace prior context")
            fail(errors, override.get("accepted_prior_stage_handoff_only") is True, f"{identity}: reentry must use accepted prior handoff")
            fail(errors, override.get("dependency_implementation_reads_allowed") is False, f"{identity}: reentry may not read dependency implementation")
        else:
            fail(errors, row.get("reentry_stage") == "NONE", f"{identity}: same/earlier-wave edge has spurious reentry")
            fail(errors, int(row.get("producer_earliest_wave", -1)) <= int(row.get("consumer_earliest_wave", -1)), f"{identity}: unmodelled later-wave dependency")
        fail(errors, (ROOT / str(row.get("contract_source"))).is_file(), f"{identity}: missing contract source")
        fail(errors, (ROOT / str(row.get("cargo_manifest"))).is_file(), f"{identity}: missing Cargo manifest")

    module_fields = [
        "package", "module", "role", "structural_rationale", "operation_count", "documentation_node_count",
        "specific_documentation_node_count", "architecture_relation_count", "port_relation_count",
        "port_method_relation_count", "schema_relation_count", "configuration_relation_count", "recipe_relation_count",
        "dependency_relation_count", "weakly_covered",
    ]
    expected_modules = {row["id"]: {field: row.get(field) for field in module_fields} for row in graph["module_rows"]}
    actual_modules = canonical_rows(module_doc, "module", "id", module_fields)
    fail(errors, actual_modules == expected_modules, f"module-coverage registry stale or divergent: expected {len(expected_modules)}, actual {len(actual_modules)}")
    fail(errors, module_doc.get("module_count") == len(expected_modules), "module coverage count mismatch")
    fail(errors, module_doc.get("weak_module_count") == len(graph["weak_modules"]), "module weak count mismatch")
    for identity, row in actual_modules.items():
        role = row.get("role")
        rationale = row.get("structural_rationale")
        if role in {"public_entry", "structural_boundary", "structural_support"}:
            fail(errors, isinstance(rationale, str) and bool(rationale.strip()), f"{identity}: structural module rationale missing")
    fail(errors, not graph["weak_modules"], f"implementation modules without specific operation/document/architecture relation: {graph['weak_modules']}")

    config_rows = rows(config_doc, "section", "name") if config_doc else {}
    fail(errors, config_doc.get("schema_version") == 3, "configuration registry must be schema v3")
    fail(errors, len(config_rows) == 20, "configuration section count must be 20")
    for name, row in config_rows.items():
        ref = f"{row.get('owner')}:{row.get('owner_module')}"
        fail(errors, ref in valid_refs, f"config {name}: invalid owner module {ref}")
        fail(errors, (ROOT / str(row.get("contract"))).is_file(), f"config {name}: missing contract")

    recipe_rows = rows(recipe_doc, "recipe", "id") if recipe_doc else {}
    fail(errors, recipe_doc.get("schema_version") == 2, "recipe registry must be schema v2")
    fail(errors, len(recipe_rows) == 11, "recipe count must be 11")
    for recipe, row in recipe_rows.items():
        owners = row.get("primary_execution_packages")
        refs = row.get("execution_modules")
        fail(errors, isinstance(owners, list) and isinstance(refs, list) and len(owners) == len(refs), f"{recipe}: one module per primary package required")
        if isinstance(refs, list):
            for ref in refs:
                fail(errors, ref in valid_refs, f"{recipe}: invalid execution module {ref}")
            fail(errors, {ref_package(ref) for ref in refs} == set(owners or []), f"{recipe}: execution module/package mismatch")

    relation_registries = [
        (manifest.get("architecture_section_registry"), "section", "id", "primary_packages", "modules", "exact"),
        (manifest.get("capability_registry"), "cell", "id", "primary_packages", "modules", "subset"),
        (manifest.get("invariant_registry"), "invariant", "id", "enforcement_packages", "modules", "exact"),
        (manifest.get("delivery_registry"), "slice", "id", "primary_packages", "modules", "exact"),
    ]
    for path, key, id_key, package_key, module_key, mode in relation_registries:
        document = load(str(path), errors)
        for identity, row in rows(document, key, id_key).items() if document else []:
            primary = set(row.get(package_key, []))
            declared = set(primary)
            if key == "cell":
                declared |= set(row.get("supporting_packages", [])) | set(row.get("state_owner_packages", []))
            required = primary
            validate_owner_modules(errors, f"{key}:{identity}", row.get(module_key), declared, required, valid_refs)

    port_doc = load(str(manifest.get("port_registry")), errors)
    for name, row in rows(port_doc, "port", "name").items() if port_doc else []:
        ref = f"{row.get('implementation_package')}:{row.get('implementation_module')}"
        fail(errors, ref in valid_refs, f"port {name}: invalid implementation module {ref}")

    schema_doc = load(str(manifest.get("schema_registry")), errors)
    for packet in schema_doc.get("packet", []) if schema_doc else []:
        packet_doc = load(packet.get("path", ""), errors)
        for group in packet_doc.get("group", []) if packet_doc else []:
            for prefix in ("shape_owner", "meaning_owner", "state_owner"):
                package = group.get(f"{prefix}_package")
                module = group.get(f"{prefix}_module")
                if package == "NONE" and module == "NONE":
                    continue
                fail(errors, f"{package}:{module}" in valid_refs, f"schema {packet.get('path')}::{group.get('id')}: invalid {prefix} module")

    for key in (
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ):
        fail(errors, manifest.get(key) is False, f"coverage manifest authority flag changed: {key}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(graph["package_rows"]),
        "logical_modules": len(graph["module_rows"]),
        "operations": len(graph["operation_rows"]),
        "documentation_files": len(graph["selected_markdown"]),
        "documentation_nodes": len(graph["documentation_rows"]),
        "principle_or_invariant_nodes": principle_count,
        "governance_or_navigation_nodes": governance_count,
        "dependency_edges": len(graph["dependency_rows"]),
        "public_facade_operations": public_facades,
        "semantic_low_operations": semantic_low,
        "weak_modules": graph["weak_modules"],
        "warnings": warnings,
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
