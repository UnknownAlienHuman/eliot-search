from __future__ import annotations

import hashlib
import json
import re
import tomllib
from collections import defaultdict, deque
from pathlib import Path
from typing import Any, Iterable

from coverage_graph_v2 import ROOT, arr, build_graph, load, q, rows

MAP_ROOT = "swarm/coverage/package-maps"
INDEX_PATH = "swarm/coverage/package-map-index.toml"
DOC_INDEX_PATH = "swarm/coverage/documentation-file-index-v2.toml"
INTEGRATION_PATH = "swarm/coverage/integration-map-v2.toml"
HUMAN_INDEX_PATH = "docs/handoff/PACKAGE_MAP_INDEX_V2.md"


def digest_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def bool_text(value: bool) -> str:
    return "true" if value else "false"


def string_list(value: Any) -> list[str]:
    return [item for item in value if isinstance(item, str)] if isinstance(value, list) else []


def exact_internal_dependencies(path: Path, known_packages: set[str]) -> set[str]:
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    result: set[str] = set()
    for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = document.get(table_name, {})
        if isinstance(table, dict):
            result.update(name for name in table if name in known_packages)
    for target in document.get("target", {}).values() if isinstance(document.get("target"), dict) else []:
        if not isinstance(target, dict):
            continue
        for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = target.get(table_name, {})
            if isinstance(table, dict):
                result.update(name for name in table if name in known_packages)
    return result


def dependency_cycle(package_rows: dict[str, dict[str, Any]]) -> list[str]:
    indegree = {package: 0 for package in package_rows}
    outgoing: dict[str, list[str]] = defaultdict(list)
    for consumer, row in package_rows.items():
        for producer in string_list(row.get("deps")):
            outgoing[producer].append(consumer)
            indegree[consumer] += 1
    queue = deque(sorted(package for package, degree in indegree.items() if degree == 0))
    visited: list[str] = []
    while queue:
        package = queue.popleft()
        visited.append(package)
        for consumer in sorted(outgoing[package]):
            indegree[consumer] -= 1
            if indegree[consumer] == 0:
                queue.append(consumer)
    return sorted(package for package, degree in indegree.items() if degree > 0)


def architecture_relations(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    specs = [
        ("architecture_section", manifest["architecture_section_registry"], "section", "id", "modules"),
        ("capability", manifest["capability_registry"], "cell", "id", "modules"),
        ("invariant", manifest["invariant_registry"], "invariant", "id", "modules"),
        ("delivery", manifest["delivery_registry"], "slice", "id", "modules"),
    ]
    for kind, path, table, identity_key, module_key in specs:
        for identity, row in rows(load(path), table, identity_key).items():
            result.append(
                {
                    "kind": kind,
                    "id": identity,
                    "modules": string_list(row.get(module_key)),
                    "required_outputs": string_list(row.get("required_outputs")),
                    "exit_evidence": string_list(row.get("exit_evidence")),
                }
            )
    return result


def port_relations(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for name, row in rows(load(manifest["port_registry"]), "port", "name").items():
        result.append(
            {
                "name": name,
                "package": row.get("implementation_package"),
                "module": row.get("implementation_module"),
                "methods": string_list(row.get("methods")),
            }
        )
    return result


def schema_relations(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    schema_registry = load(manifest["schema_registry"])
    for packet in schema_registry.get("packet", []):
        if not isinstance(packet, dict) or not isinstance(packet.get("path"), str):
            continue
        packet_path = packet["path"]
        document = load(packet_path)
        for group in document.get("group", []):
            if not isinstance(group, dict):
                continue
            owners: list[dict[str, str]] = []
            for owner_kind in ("shape_owner", "meaning_owner", "state_owner"):
                package = group.get(f"{owner_kind}_package")
                module = group.get(f"{owner_kind}_module")
                if isinstance(package, str) and isinstance(module, str) and package != "NONE" and module != "NONE":
                    owners.append({"kind": owner_kind, "package": package, "module": module})
            result.append(
                {
                    "packet": packet_path,
                    "group": str(group.get("id", "UNNAMED")),
                    "schemas": string_list(group.get("schemas")),
                    "source_files": string_list(group.get("source_files")),
                    "owners": owners,
                }
            )
    return result


def package_paths(package: str) -> dict[str, str]:
    root = f"{MAP_ROOT}/{package}"
    return {
        "overview": f"{root}/overview.toml",
        "operations": f"{root}/operations.toml",
        "documents": f"{root}/documents.toml",
        "relations": f"{root}/relations.toml",
    }


def render_overview(
    package: str,
    package_row: dict[str, Any],
    modules: list[dict[str, Any]],
    counts: dict[str, int],
    paths: dict[str, str],
) -> str:
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "BOUNDED_PACKAGE_MAP_NOT_IMPLEMENTED"',
        f"package = {q(package)}",
        f"path = {q(str(package_row.get('path')))}",
        f"kind = {q(str(package_row.get('kind')))}",
        f"family = {q(str(package_row.get('family')))}",
        f"cell = {q(str(package_row.get('cell')))}",
        f"earliest_wave = {int(package_row.get('wave', 0))}",
        f"optional = {bool_text(bool(package_row.get('optional')))}",
        f"soft_src_line_target = {int(package_row.get('soft_src_line_target', 0))}",
        f"assignment = {q(str(package_row.get('assignment')))}",
        f"function_source = {q(str(package_row.get('functions', 'FOUNDATION_CONTRACT')))}",
        f"qualification = {q(str(package_row.get('qualification', 'NONE')))}",
        f"config_sections = {arr(string_list(package_row.get('config_sections')))}",
        f"declared_dependencies = {arr(string_list(package_row.get('deps')))}",
        f"module_count = {counts['modules']}",
        f"operation_count = {counts['operations']}",
        f"documentation_node_count = {counts['documents']}",
        f"principle_node_count = {counts['principles']}",
        f"outbound_dependency_count = {counts['outbound_dependencies']}",
        f"inbound_dependency_count = {counts['inbound_dependencies']}",
        f"architecture_relation_count = {counts['architecture']}",
        f"configuration_relation_count = {counts['configuration']}",
        f"recipe_relation_count = {counts['recipes']}",
        f"port_relation_count = {counts['ports']}",
        f"schema_relation_count = {counts['schemas']}",
        f"operations_map = {q(paths['operations'])}",
        f"documents_map = {q(paths['documents'])}",
        f"relations_map = {q(paths['relations'])}",
        "one_agent_one_package = true",
        "cross_package_reads_require_public_handoff = true",
        "implementation_authorized_by_this_map = false",
        "",
    ]
    for module in modules:
        lines.extend(
            [
                "[[module]]",
                f"name = {q(module['module'])}",
                f"role = {q(module['role'])}",
                f"structural_rationale = {q(module.get('structural_rationale', ''))}",
                f"operation_count = {module['operation_count']}",
                f"documentation_node_count = {module['documentation_node_count']}",
                f"architecture_relation_count = {module['architecture_relation_count']}",
                f"port_relation_count = {module['port_relation_count']}",
                f"schema_relation_count = {module['schema_relation_count']}",
                f"configuration_relation_count = {module['configuration_relation_count']}",
                f"recipe_relation_count = {module['recipe_relation_count']}",
                f"dependency_relation_count = {module['dependency_relation_count']}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def render_operations(package: str, operation_rows: list[dict[str, Any]]) -> str:
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "EXACT_PACKAGE_OPERATION_MAP_NOT_IMPLEMENTED"',
        f"package = {q(package)}",
        f"operation_count = {len(operation_rows)}",
        "one_internal_module_per_operation = true",
        "public_facade_or_low_confidence_routes_allowed = false",
        "implementation_authorized_by_this_map = false",
        "",
    ]
    for row in operation_rows:
        lines.extend(
            [
                "[[operation]]",
                f"id = {q(row['id'])}",
                f"name = {q(row['operation'])}",
                f"module = {q(row['module'])}",
                f"public_entry_module = {q(row['public_entry_module'])}",
                f"sources = {arr(row['sources'])}",
                f"source_contexts = {arr(row['source_contexts'])}",
                f"route_kind = {q(row['route_kind'])}",
                f"score = {int(row['score'])}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def render_documents(package: str, document_rows: list[dict[str, Any]]) -> str:
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "PACKAGE_DOCUMENT_NODE_MAP_NOT_IMPLEMENTED"',
        f"package = {q(package)}",
        f"node_count = {len(document_rows)}",
        f"principle_count = {sum(1 for row in document_rows if row['kind'] == 'principle_or_invariant')}",
        "every_node_has_package_local_module_route = true",
        "implementation_authorized_by_this_map = false",
        "",
    ]
    for row in document_rows:
        package_modules = sorted(ref for ref in row["modules"] if ref.startswith(package + ":"))
        lines.extend(
            [
                "[[node]]",
                f"id = {q(row['id'])}",
                f"path = {q(row['path'])}",
                f"line = {int(row['line'])}",
                f"level = {int(row['level'])}",
                f"heading = {q(row['heading'])}",
                f"kind = {q(row['kind'])}",
                f"modules = {arr(package_modules)}",
                f"route_kind = {q(row['route_kind'])}",
                f"rationale = {q(row['rationale'])}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def render_relations(
    package: str,
    outbound: list[dict[str, Any]],
    inbound: list[dict[str, Any]],
    architecture: list[dict[str, Any]],
    configurations: list[dict[str, Any]],
    recipes: list[dict[str, Any]],
    ports: list[dict[str, Any]],
    schemas: list[dict[str, Any]],
) -> str:
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "PACKAGE_RELATION_MAP_NOT_IMPLEMENTED"',
        f"package = {q(package)}",
        f"outbound_dependency_count = {len(outbound)}",
        f"inbound_dependency_count = {len(inbound)}",
        f"architecture_relation_count = {len(architecture)}",
        f"configuration_relation_count = {len(configurations)}",
        f"recipe_relation_count = {len(recipes)}",
        f"port_relation_count = {len(ports)}",
        f"schema_relation_count = {len(schemas)}",
        "dependency_targets_enter_through_public_entry = true",
        "implementation_authorized_by_this_map = false",
        "",
    ]
    for direction, values in (("outbound", outbound), ("inbound", inbound)):
        for row in values:
            lines.extend(
                [
                    "[[dependency]]",
                    f"direction = {q(direction)}",
                    f"id = {q(row['id'])}",
                    f"consumer = {q(row['consumer'])}",
                    f"consumer_module = {q(row['consumer_module'])}",
                    f"producer = {q(row['producer'])}",
                    f"producer_module = {q(row['producer_module'])}",
                    f"relationship = {q(row['relationship'])}",
                    f"contract_source = {q(row['contract_source'])}",
                    f"exact_accepted_handoff_required = {bool_text(bool(row['exact_accepted_handoff_required']))}",
                    "",
                ]
            )
    for row in architecture:
        package_modules = sorted(ref for ref in row["modules"] if ref.startswith(package + ":"))
        lines.extend(
            [
                "[[architecture]]",
                f"kind = {q(row['kind'])}",
                f"id = {q(row['id'])}",
                f"modules = {arr(package_modules)}",
                f"required_outputs = {arr(row['required_outputs'])}",
                f"exit_evidence = {arr(row['exit_evidence'])}",
                "",
            ]
        )
    for row in configurations:
        lines.extend(
            [
                "[[configuration]]",
                f"section = {q(row['name'])}",
                f"module = {q(row['owner_module'])}",
                f"contract = {q(row['contract'])}",
                f"reload = {q(str(row.get('reload')))}",
                "",
            ]
        )
    for row in recipes:
        lines.extend(
            [
                "[[recipe]]",
                f"id = {q(row['id'])}",
                f"module = {q(row['module'])}",
                f"request_schema = {q(str(row.get('request_schema')))}",
                f"result_schema = {q(str(row.get('result_schema')))}",
                "",
            ]
        )
    for row in ports:
        lines.extend(
            [
                "[[port]]",
                f"name = {q(row['name'])}",
                f"module = {q(str(row['module']))}",
                f"methods = {arr(row['methods'])}",
                "",
            ]
        )
    for row in schemas:
        lines.extend(
            [
                "[[schema_group]]",
                f"packet = {q(row['packet'])}",
                f"group = {q(row['group'])}",
                f"owner_roles = {arr(row['owner_roles'])}",
                f"modules = {arr(row['modules'])}",
                f"schemas = {arr(row['schemas'])}",
                f"source_files = {arr(row['source_files'])}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def build_outputs() -> tuple[dict[str, str], dict[str, Any]]:
    graph = build_graph()
    manifest = load("swarm/coverage/manifest.toml")
    architecture = architecture_relations(manifest)
    ports = port_relations(manifest)
    schemas = schema_relations(manifest)
    config_rows = rows(load("config/sections.toml"), "section", "name")
    recipe_rows = rows(load(manifest["recipe_registry"]), "recipe", "id")

    outputs: dict[str, str] = {}
    index_rows: list[dict[str, Any]] = []

    for package in sorted(graph["package_rows"]):
        package_row = graph["package_rows"][package]
        paths = package_paths(package)
        package_modules = [row for row in graph["module_rows"] if row["package"] == package]
        package_operations = [row for row in graph["operation_rows"] if row["package"] == package]
        package_documents = [row for row in graph["documentation_rows"] if package in row["packages"]]
        outbound = [row for row in graph["dependency_rows"] if row["consumer"] == package]
        inbound = [row for row in graph["dependency_rows"] if row["producer"] == package]
        package_architecture = [row for row in architecture if any(ref.startswith(package + ":") for ref in row["modules"])]
        package_configs = [dict(row, name=name) for name, row in config_rows.items() if row.get("owner") == package]
        package_recipes: list[dict[str, Any]] = []
        for recipe_id, row in recipe_rows.items():
            for ref in string_list(row.get("execution_modules")):
                if ref.startswith(package + ":"):
                    package_recipes.append(dict(row, id=recipe_id, module=ref.split(":", 1)[1]))
        package_ports = [row for row in ports if row["package"] == package]
        package_schemas: list[dict[str, Any]] = []
        for row in schemas:
            owner_rows = [owner for owner in row["owners"] if owner["package"] == package]
            if owner_rows:
                package_schemas.append(
                    dict(
                        row,
                        owner_roles=sorted(owner["kind"] for owner in owner_rows),
                        modules=sorted({owner["module"] for owner in owner_rows}),
                    )
                )

        counts = {
            "modules": len(package_modules),
            "operations": len(package_operations),
            "documents": len(package_documents),
            "principles": sum(1 for row in package_documents if row["kind"] == "principle_or_invariant"),
            "outbound_dependencies": len(outbound),
            "inbound_dependencies": len(inbound),
            "architecture": len(package_architecture),
            "configuration": len(package_configs),
            "recipes": len(package_recipes),
            "ports": len(package_ports),
            "schemas": len(package_schemas),
        }
        outputs[paths["overview"]] = render_overview(package, package_row, package_modules, counts, paths)
        outputs[paths["operations"]] = render_operations(package, package_operations)
        outputs[paths["documents"]] = render_documents(package, package_documents)
        outputs[paths["relations"]] = render_relations(
            package,
            outbound,
            inbound,
            package_architecture,
            package_configs,
            package_recipes,
            package_ports,
            package_schemas,
        )
        index_rows.append(
            {
                "package": package,
                "path": package_row["path"],
                "wave": int(package_row.get("wave", 0)),
                "family": str(package_row.get("family")),
                "counts": counts,
                "paths": paths,
            }
        )

    integration_nodes = [row for row in graph["documentation_rows"] if not row["packages"]]
    integration_lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "EXPLICIT_NON_CRATE_DOCUMENTATION_MAP"',
        f"node_count = {len(integration_nodes)}",
        "product_semantics_allowed = false",
        "implementation_authorized_by_this_map = false",
        "",
    ]
    for row in integration_nodes:
        integration_lines.extend(
            [
                "[[node]]",
                f"id = {q(row['id'])}",
                f"path = {q(row['path'])}",
                f"line = {int(row['line'])}",
                f"heading = {q(row['heading'])}",
                f"kind = {q(row['kind'])}",
                f"route_kind = {q(row['route_kind'])}",
                f"rationale = {q(row['rationale'])}",
                "",
            ]
        )
    outputs[INTEGRATION_PATH] = "\n".join(integration_lines).rstrip() + "\n"

    by_path: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in graph["documentation_rows"]:
        by_path[row["path"]].append(row)
    doc_index_lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "DOCUMENTATION_FILE_REVERSE_INDEX_NOT_IMPLEMENTED"',
        f"file_count = {len(by_path)}",
        f"node_count = {len(graph['documentation_rows'])}",
        "every_tracked_markdown_file_indexed = true",
        "implementation_authorized_by_this_index = false",
        "",
    ]
    for path, document_nodes in sorted(by_path.items()):
        packages = sorted({package for row in document_nodes for package in row["packages"]})
        modules = sorted({module for row in document_nodes for module in row["modules"]})
        doc_index_lines.extend(
            [
                "[[file]]",
                f"path = {q(path)}",
                f"node_count = {len(document_nodes)}",
                f"principle_count = {sum(1 for row in document_nodes if row['kind'] == 'principle_or_invariant')}",
                f"packages = {arr(packages)}",
                f"modules = {arr(modules)}",
                f"non_crate_only = {bool_text(not packages)}",
                "",
            ]
        )
    outputs[DOC_INDEX_PATH] = "\n".join(doc_index_lines).rstrip() + "\n"

    index_lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "BOUNDED_PACKAGE_MAP_INDEX_NOT_IMPLEMENTED"',
        f"package_count = {len(index_rows)}",
        f"map_file_count = {len(index_rows) * 4}",
        f"operation_count = {len(graph['operation_rows'])}",
        f"documentation_node_count = {len(graph['documentation_rows'])}",
        f"dependency_edge_count = {len(graph['dependency_rows'])}",
        f"logical_module_count = {len(graph['module_rows'])}",
        f"integration_node_count = {len(integration_nodes)}",
        f"documentation_file_index = {q(DOC_INDEX_PATH)}",
        f"integration_map = {q(INTEGRATION_PATH)}",
        "one_agent_reads_one_package_map = true",
        "global_architecture_read_required = false",
        "implementation_authorized_by_this_index = false",
        "",
    ]
    for row in index_rows:
        index_lines.extend(
            [
                "[[package]]",
                f"name = {q(row['package'])}",
                f"path = {q(row['path'])}",
                f"wave = {row['wave']}",
                f"family = {q(row['family'])}",
            ]
        )
        for key in ("overview", "operations", "documents", "relations"):
            path = row["paths"][key]
            index_lines.append(f"{key}_map = {q(path)}")
            index_lines.append(f"{key}_sha256 = {q(digest_text(outputs[path]))}")
        for key, value in row["counts"].items():
            index_lines.append(f"{key}_count = {value}")
        index_lines.append("")
    outputs[INDEX_PATH] = "\n".join(index_lines).rstrip() + "\n"

    human = [
        "# Bounded package map index v2",
        "",
        "This index is the Swarm entry point after a package is assigned. A package writer reads only:",
        "",
        "1. its assignment and issued context bundle;",
        "2. `overview.toml`;",
        "3. the package-local operation, documentation and relation maps linked by the overview;",
        "4. exact accepted dependency handoffs named by the relation map.",
        "",
        "The maps do not authorize implementation and do not replace an issued ticket or lease.",
        "",
        "| Package | Wave | Modules | Operations | Doc nodes | Dependencies | Map |",
        "|---|---:|---:|---:|---:|---:|---|",
    ]
    for row in index_rows:
        counts = row["counts"]
        human.append(
            f"| `{row['package']}` | {row['wave']} | {counts['modules']} | {counts['operations']} | "
            f"{counts['documents']} | {counts['outbound_dependencies']} | "
            f"[`overview`](/swarm/coverage/package-maps/{row['package']}/overview.toml) |"
        )
    human.extend(
        [
            "",
            "## Global reverse indexes",
            "",
            f"- `{INDEX_PATH}` — package-to-map index with exact digests.",
            f"- `{DOC_INDEX_PATH}` — documentation file to package/module reverse index.",
            f"- `{INTEGRATION_PATH}` — governance/navigation nodes explicitly outside product crates.",
            "- `swarm/coverage/operation-modules.toml` — operation to exact package-local module.",
            "- `swarm/coverage/dependency-edges.toml` — typed package/module dependency edges.",
            "- `swarm/coverage/module-coverage.toml` — reverse relation counts and structural roles.",
        ]
    )
    outputs[HUMAN_INDEX_PATH] = "\n".join(human).rstrip() + "\n"

    stats = {
        "packages": len(index_rows),
        "map_files": len(index_rows) * 4,
        "operations": len(graph["operation_rows"]),
        "documents": len(graph["documentation_rows"]),
        "modules": len(graph["module_rows"]),
        "dependencies": len(graph["dependency_rows"]),
        "integration_nodes": len(integration_nodes),
        "cycle": dependency_cycle(graph["package_rows"]),
    }
    return outputs, stats


def patch_manifest(stats: dict[str, Any]) -> None:
    path = ROOT / "swarm/coverage/manifest.toml"
    text = path.read_text(encoding="utf-8")
    marker = 'module_coverage_registry = "swarm/coverage/module-coverage.toml"\n'
    addition = (
        marker
        + f'package_map_index = "{INDEX_PATH}"\n'
        + f'documentation_file_index = "{DOC_INDEX_PATH}"\n'
        + f'integration_documentation_map = "{INTEGRATION_PATH}"\n'
    )
    if "package_map_index" not in text:
        if text.count(marker) != 1:
            raise RuntimeError("manifest module coverage marker missing or duplicated")
        text = text.replace(marker, addition, 1)
    count_marker = re.compile(
        r"weak_logical_module_count = \d+\n(?:package_map_count = \d+\n)?(?:package_map_file_count = \d+\n)?(?:integration_documentation_node_count = \d+\n)?"
    )
    replacement = (
        f"weak_logical_module_count = 0\n"
        f"package_map_count = {stats['packages']}\n"
        f"package_map_file_count = {stats['map_files']}\n"
        f"integration_documentation_node_count = {stats['integration_nodes']}\n"
    )
    text, count = count_marker.subn(replacement, text, count=1)
    if count != 1:
        raise RuntimeError("manifest map count block missing")
    path.write_text(text, encoding="utf-8", newline="\n")


def write_outputs(outputs: dict[str, str], stats: dict[str, Any]) -> None:
    package_root = ROOT / MAP_ROOT
    if package_root.exists():
        for path in sorted(package_root.rglob("*.toml"), reverse=True):
            path.unlink()
    for relative, content in outputs.items():
        path = ROOT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8", newline="\n")
    patch_manifest(stats)
