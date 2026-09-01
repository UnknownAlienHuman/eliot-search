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

    counts = (
        'operation_count = "DERIVED_BY_VALIDATOR"\n'
        + f"exact_operation_module_count = {len(graph['operation_rows'])}\n"
        + f"documentation_source_file_count = {len(graph['selected_markdown'])}\n"
        + f"documentation_node_count = {len(graph['documentation_rows'])}\n"
        + f"dependency_edge_count = {len(graph['dependency_rows'])}\n"
        + f"logical_module_count = {len(graph['module_rows'])}\n"
        + f"weak_logical_module_count = {len(graph['weak_modules'])}\n"
    )
    existing_pattern = re.compile(
        r'operation_count = "DERIVED_BY_VALIDATOR"\n'
        r'(?:exact_operation_module_count = \d+\n)?'
        r'(?:documentation_source_file_count = \d+\n)?'
        r'(?:documentation_node_count = \d+\n)?'
        r'(?:dependency_edge_count = \d+\n)?'
        r'(?:logical_module_count = \d+\n)?'
        r'(?:weak_logical_module_count = \d+\n)?'
    )
    text, replacements = existing_pattern.subn(counts, text, count=1)
    if replacements != 1:
        raise RuntimeError("manifest relation-count block missing")
    path.write_text(text, encoding="utf-8", newline="\n")


def write_human_report(graph: dict) -> None:
    route_counts: dict[str, int] = {}
    for row in graph["operation_rows"]:
        route_counts[row["route_kind"]] = route_counts.get(row["route_kind"], 0) + 1
    governance = sum(
        1 for row in graph["documentation_rows"] if row["kind"] in {"governance", "navigation"}
    )
    implementation = len(graph["documentation_rows"]) - governance
    progressive = sum(row["requires_stage_reentry"] for row in graph["dependency_rows"])
    body = f'''# Coverage graph v2

This is the exact machine-checked ownership graph from architecture and package contracts to Cargo
packages and package-local logical modules. It does not claim Rust implementation.

## Closed relations

- **{len(graph['package_rows'])} Cargo packages** and **{len(graph['module_rows'])} declared logical modules**;
- **{len(graph['operation_rows'])} package-qualified operations** mapped to exactly one reviewed module in the same package;
- **{len(graph['documentation_rows'])} Markdown heading nodes** across **{len(graph['selected_markdown'])} tracked documentation files**;
- **{implementation} implementation/principle/qualification nodes** mapped to package modules;
- **{governance} governance/navigation nodes** explicitly classified as non-crate-owned rather than forced into a fake product crate;
- **{len(graph['dependency_rows'])} Cargo dependency edges** mapped from a consumer module to the producer public entry;
- **{progressive} later-wave dependency edges** bound to exact progressive stage re-entry records;
- all 20 configuration sections bound to an owner module;
- all 11 recipes bound to one execution module per primary execution package;
- all 23 shared ports and 80 port methods bound to package-local modules;
- **{len(graph['weak_modules'])} weak implementation modules** after relation aggregation.

## Operation routing quality

```text
{json.dumps(route_counts, indent=2, sort_keys=True)}
```

`public_facade` and `semantic_low` routes are merge-blocking. The committed operation registry records
the exact source file, source section, selected module, routing class and score for review.

## Validation

```powershell
python tools/generate-coverage-graph-v2.py --check
python tools/generate-package-maps-v2.py --check
python tools/validate-coverage-graph-v2.py --json
python tools/validate-package-maps-v2.py --json
python tools/validate-architecture-coverage.py --json
```

The validators reject missing or orphan operations, stale documentation headings, cross-package module
routes, configuration/recipe/port owner drift, missing dependency or re-entry edges, weak implementation
modules and any automatic trigger in the permanent validation workflow.

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
    stale: list[str] = []
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
