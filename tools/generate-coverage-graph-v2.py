#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from coverage_graph_v2 import (
    ROOT,
    build_graph,
    digest_text,
    render_dependency_registry,
    render_documentation_registry,
    render_module_registry,
    render_operation_registry,
)


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content.rstrip() + "\n", encoding="utf-8", newline="\n")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def patch_legacy_validator() -> None:
    path = ROOT / "tools/validate-architecture-coverage.py"
    text = path.read_text(encoding="utf-8")
    text = replace_once(
        text,
        'semantic = section_between(type_registry, "## Baseline semantic registries", "## Coverage and freshness records")',
        'semantic = section_between(type_registry, "## Baseline semantic registries", "## Coverage records")',
        "semantic heading",
    )
    text = replace_once(
        text,
        'coverage = section_between(type_registry, "## Coverage and freshness records", "## Port-support records")',
        'coverage = section_between(type_registry, "## Coverage records", "## Port support records — owned by `search-ports`")',
        "coverage heading",
    )
    text = replace_once(
        text,
        'port_support = section_between(type_registry, "## Port-support records", "## Ownership and visibility summary")',
        'port_support = section_between(type_registry, "## Port support records — owned by `search-ports`", "## New-type rule")',
        "port support heading",
    )

    old_config = '''        owner = row.get("owner")
        packet = row.get("packet")
        fail(errors, owner in packages, f"config {name}: unknown owner {owner}")
        fail(errors, isinstance(packet, str) and (ROOT / packet).is_file(), f"config {name}: missing packet {packet}")
        if isinstance(packet, str):
            fail(errors, packet not in config_packets, f"config {name}: duplicate packet path {packet}")
            config_packets.add(packet)
'''
    new_config = '''        owner = row.get("owner")
        owner_module = row.get("owner_module")
        packet = row.get("contract")
        fail(errors, owner in packages, f"config {name}: unknown owner {owner}")
        fail(errors, isinstance(owner_module, str), f"config {name}: owner module missing")
        if isinstance(owner, str) and isinstance(owner_module, str):
            validate_module_ref(errors, f"{owner}:{owner_module}", modules, f"config {name}")
        fail(errors, isinstance(packet, str) and (ROOT / packet).is_file(), f"config {name}: missing contract {packet}")
        if isinstance(packet, str):
            fail(errors, packet not in config_packets, f"config {name}: duplicate contract path {packet}")
            config_packets.add(packet)
'''
    text = replace_once(text, old_config, new_config, "configuration ownership block")

    old_recipe = '''        owners = row.get("primary_execution_packages")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{recipe_id}: execution owners missing")
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{recipe_id}: unknown execution package {package}")
'''
    new_recipe = '''        owners = row.get("primary_execution_packages")
        refs = row.get("execution_modules")
        fail(errors, isinstance(owners, list) and len(owners) > 0, f"{recipe_id}: execution owners missing")
        fail(errors, isinstance(refs, list) and len(refs) == len(owners if isinstance(owners, list) else []), f"{recipe_id}: one execution module per owner required")
        ref_packages: set[str] = set()
        for ref in refs if isinstance(refs, list) else []:
            validate_module_ref(errors, ref, modules, recipe_id)
            if isinstance(ref, str) and ":" in ref:
                ref_packages.add(ref.split(":", 1)[0])
        for package in owners if isinstance(owners, list) else []:
            fail(errors, package in packages, f"{recipe_id}: unknown execution package {package}")
        fail(errors, ref_packages == set(owners if isinstance(owners, list) else []), f"{recipe_id}: execution module/package mismatch")
'''
    text = replace_once(text, old_recipe, new_recipe, "recipe ownership block")
    path.write_text(text, encoding="utf-8", newline="\n")


def patch_manifest(graph: dict) -> None:
    path = ROOT / "swarm/coverage/manifest.toml"
    text = path.read_text(encoding="utf-8")
    text = re.sub(r"^schema_version = \d+$", "schema_version = 2", text, count=1, flags=re.MULTILINE)
    text = re.sub(
        r'^status = ".*"$',
        'status = "STATIC_OWNERSHIP_AND_RELATION_COVERAGE_CLOSED_NOT_IMPLEMENTED"',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    marker = 'operation_registry = "swarm/coverage/operations.toml"\n'
    addition = (
        marker
        + 'operation_module_registry = "swarm/coverage/operation-modules.toml"\n'
        + 'documentation_node_registry = "swarm/coverage/documentation-nodes.toml"\n'
        + 'dependency_edge_registry = "swarm/coverage/dependency-edges.toml"\n'
        + 'module_coverage_registry = "swarm/coverage/module-coverage.toml"\n'
    )
    if "operation_module_registry" not in text:
        text = replace_once(text, marker, addition, "manifest registry links")
    count_marker = 'operation_count = "DERIVED_BY_VALIDATOR"\n'
    counts = (
        count_marker
        + f"exact_operation_module_count = {len(graph['operation_rows'])}\n"
        + f"documentation_source_file_count = {len(graph['selected_markdown'])}\n"
        + f"documentation_node_count = {len(graph['documentation_rows'])}\n"
        + f"dependency_edge_count = {len(graph['dependency_rows'])}\n"
        + f"logical_module_count = {len(graph['module_rows'])}\n"
        + f"weak_logical_module_count = {len(graph['weak_modules'])}\n"
    )
    existing_pattern = re.compile(
        r'operation_count = "DERIVED_BY_VALIDATOR"\n(?:exact_operation_module_count = \d+\n)?(?:documentation_source_file_count = \d+\n)?(?:documentation_node_count = \d+\n)?(?:dependency_edge_count = \d+\n)?(?:logical_module_count = \d+\n)?(?:weak_logical_module_count = \d+\n)?'
    )
    text = existing_pattern.sub(counts, text, count=1)
    path.write_text(text, encoding="utf-8", newline="\n")


def write_human_report(graph: dict) -> None:
    route_counts: dict[str, int] = {}
    for row in graph["operation_rows"]:
        route_counts[row["route_kind"]] = route_counts.get(row["route_kind"], 0) + 1
    governance = sum(1 for row in graph["documentation_rows"] if row["kind"] in {"governance", "navigation"})
    implementation = len(graph["documentation_rows"]) - governance
    body = f'''# Coverage graph v2

This is the exact machine-checked ownership graph from architecture and package contracts to Cargo
packages and package-local logical modules. It does not claim Rust implementation.

## Closed relations

- **{len(graph['package_rows'])} Cargo packages** and **{len(graph['module_rows'])} declared logical modules**;
- **{len(graph['operation_rows'])} package-qualified operations** mapped to exactly one module in the same package;
- **{len(graph['documentation_rows'])} Markdown heading nodes** across **{len(graph['selected_markdown'])} tracked documentation files**;
- **{implementation} implementation/principle/qualification nodes** mapped to package modules;
- **{governance} governance/navigation nodes** explicitly classified as non-crate-owned rather than forced into a fake product crate;
- **{len(graph['dependency_rows'])} Cargo dependency edges** mapped from a consumer module to the producer public entry;
- all 20 configuration sections bound to an owner module;
- all 11 recipes bound to one execution module per primary execution package;
- **{len(graph['weak_modules'])} weak implementation modules** after relation aggregation.

## Operation routing quality

```text
{json.dumps(route_counts, indent=2, sort_keys=True)}
```

`public_facade` is permitted only when the documented operation itself is the package entry/facade
operation. The committed operation registry records the exact source file, source section, selected
module, routing class and score for review.

## Validation

```powershell
python tools/generate-coverage-graph-v2.py --check
python tools/validate-coverage-graph-v2.py --json
python tools/validate-architecture-coverage.py --json
```

The validators reject missing or orphan operations, stale documentation headings, cross-package module
routes, configuration/recipe owner drift, missing dependency edges, weak implementation modules and any
change that reintroduces an automatic permanent workflow trigger.

## Authority ceiling

These registries are design/ownership evidence only. They create no ticket, lease, accepted package
handoff, gate receipt, wave receipt or implementation authority. Launch state remains P00/W0.
'''
    write("docs/handoff/COVERAGE_GRAPH_V2.md", body)


def generate() -> dict:
    graph = build_graph()
    write("swarm/coverage/operation-modules.toml", render_operation_registry(graph))
    write("swarm/coverage/documentation-nodes.toml", render_documentation_registry(graph))
    write("swarm/coverage/dependency-edges.toml", render_dependency_registry(graph))
    write("swarm/coverage/module-coverage.toml", render_module_registry(graph))
    patch_manifest(graph)
    patch_legacy_validator()
    write_human_report(graph)
    return graph


def check() -> dict:
    graph = build_graph()
    expected = {
        "swarm/coverage/operation-modules.toml": render_operation_registry(graph).rstrip() + "\n",
        "swarm/coverage/documentation-nodes.toml": render_documentation_registry(graph).rstrip() + "\n",
        "swarm/coverage/dependency-edges.toml": render_dependency_registry(graph).rstrip() + "\n",
        "swarm/coverage/module-coverage.toml": render_module_registry(graph).rstrip() + "\n",
    }
    stale = []
    for path, content in expected.items():
        target = ROOT / path
        if not target.is_file() or target.read_text(encoding="utf-8") != content:
            stale.append(path)
    return {"graph": graph, "stale": stale}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    if args.check:
        result = check()
        summary = {
            "status": "PASS" if not result["stale"] else "FAIL",
            "stale": result["stale"],
            "operations": len(result["graph"]["operation_rows"]),
            "documentation_nodes": len(result["graph"]["documentation_rows"]),
            "dependency_edges": len(result["graph"]["dependency_rows"]),
            "modules": len(result["graph"]["module_rows"]),
            "weak_modules": result["graph"]["weak_modules"],
        }
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0 if summary["status"] == "PASS" else 1
    graph = generate()
    summary = {
        "status": "GENERATED",
        "operations": len(graph["operation_rows"]),
        "documentation_nodes": len(graph["documentation_rows"]),
        "dependency_edges": len(graph["dependency_rows"]),
        "modules": len(graph["module_rows"]),
        "weak_modules": graph["weak_modules"],
        "operation_registry_sha256": digest_text(render_operation_registry(graph)),
        "documentation_registry_sha256": digest_text(render_documentation_registry(graph)),
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
