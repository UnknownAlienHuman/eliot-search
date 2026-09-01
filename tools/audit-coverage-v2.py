#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from collections import defaultdict
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
RESERVED = {"if", "for", "while", "match", "loop", "return"}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def table_rows(doc: dict[str, Any], key: str, id_key: str) -> dict[str, dict[str, Any]]:
    value = doc.get(key, [])
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(id_key), str):
            raise ValueError(f"invalid {key} row")
        identity = row[id_key]
        if identity in result:
            raise ValueError(f"duplicate {key} identity: {identity}")
        result[identity] = row
    return result


def fenced_blocks(text: str) -> list[str]:
    return re.findall(r"```[^\n]*\n(.*?)```", text, flags=re.DOTALL)


def operation_names(text: str) -> list[str]:
    names: set[str] = set()
    for match in re.finditer(r"^#{2,3}\s+`([a-z][a-z0-9_]*)\b", text, flags=re.MULTILINE):
        names.add(match.group(1))
    for match in re.finditer(r"`([a-z][a-z0-9_]*)\([^`]*\)`", text):
        if match.group(1) not in RESERVED:
            names.add(match.group(1))
    for block in fenced_blocks(text):
        for match in re.finditer(
            r"^(?:pub\s+)?(?:async\s+)?(?:fn\s+)?([a-z][a-z0-9_]*)\s*\(",
            block,
            flags=re.MULTILINE,
        ):
            if match.group(1) not in RESERVED:
                names.add(match.group(1))
    return sorted(names)


def section_operations(text: str) -> dict[str, list[str]]:
    headings = list(re.finditer(r"^(##|###)\s+(.+?)\s*$", text, flags=re.MULTILINE))
    result: dict[str, list[str]] = {}
    current_h2 = "UNSCOPED"
    for index, match in enumerate(headings):
        level, title = match.group(1), match.group(2)
        if level == "##":
            current_h2 = re.sub(r"[`*_]", "", title).strip()
            result.setdefault(current_h2, [])
            continue
        op = re.match(r"`([a-z][a-z0-9_]*)\b", title)
        if op:
            result.setdefault(current_h2, []).append(op.group(1))
    return {heading: sorted(set(ops)) for heading, ops in result.items() if ops}


def split_ref(ref: str) -> tuple[str, str]:
    if ":" not in ref:
        return ref, ""
    return tuple(ref.split(":", 1))  # type: ignore[return-value]


def tracked_files() -> list[str]:
    output = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return [line.strip() for line in output.splitlines() if line.strip()]


def markdown_headings(path: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_no, line in enumerate(read(path).splitlines(), start=1):
        match = re.match(r"^(#{2,4})\s+(.+?)\s*$", line)
        if not match:
            continue
        title = re.sub(r"[`*_]", "", match.group(2)).strip()
        rows.append({"line": line_no, "level": len(match.group(1)), "title": title})
    return rows


def is_normative_markdown(path: str) -> bool:
    if not path.endswith(".md"):
        return False
    if path.startswith("docs/architecture/"):
        return True
    if path.startswith("docs/contracts/"):
        return True
    if path.startswith("docs/current/"):
        return True
    if path.startswith("docs/client/"):
        return True
    if path.startswith("docs/evaluation/"):
        return True
    if path.startswith("docs/optional/"):
        return True
    if path.startswith("docs/config/"):
        return True
    if path.startswith("config/sections/"):
        return True
    name = Path(path).name
    return name in {
        "FUNCTIONS.md",
        "W7_HARDENING.md",
        "P18_SCALE.md",
        "W8_INTEGRATION.md",
        "W10_INTEGRATION.md",
        "W8_CLIENT.md",
        "W10_OPTIONAL_EVALUATION.md",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()

    manifest = load("swarm/coverage/manifest.toml")
    package_doc = load(manifest["package_registry"])
    function_doc = load(manifest["function_registry"])
    module_doc = load(manifest["module_registry"])
    section_doc = load(manifest["architecture_section_registry"])
    capability_doc = load(manifest["capability_registry"])
    invariant_doc = load(manifest["invariant_registry"])
    port_doc = load(manifest["port_registry"])
    schema_doc = load(manifest["schema_registry"])
    recipe_doc = load(manifest["recipe_registry"])
    delivery_doc = load(manifest["delivery_registry"])
    config_doc = load(manifest["configuration_registry"])

    package_rows = table_rows(package_doc, "package", "name")
    packages = set(package_rows)
    foundation_rows = table_rows(function_doc, "foundation", "package")
    function_rows = table_rows(function_doc, "package", "name")

    modules: dict[str, set[str]] = {}
    module_entries: dict[str, dict[str, Any]] = {}
    for packet in module_doc.get("packet", []):
        document = load(packet["path"])
        for package, row in table_rows(document, "package", "name").items():
            modules[package] = set(row.get("modules", []))
            module_entries[package] = row

    module_references: dict[str, list[str]] = defaultdict(list)
    owner_module_mismatches: list[dict[str, Any]] = []
    invalid_module_refs: list[dict[str, str]] = []

    def register_refs(owner_kind: str, owner_id: str, refs: Any, declared_packages: set[str], require_declared_coverage: set[str]) -> None:
        refs = refs if isinstance(refs, list) else []
        ref_packages: set[str] = set()
        for ref in refs:
            if not isinstance(ref, str):
                invalid_module_refs.append({"owner": f"{owner_kind}:{owner_id}", "ref": repr(ref)})
                continue
            package, module = split_ref(ref)
            ref_packages.add(package)
            if package not in modules or module not in modules.get(package, set()):
                invalid_module_refs.append({"owner": f"{owner_kind}:{owner_id}", "ref": ref})
            else:
                module_references[ref].append(f"{owner_kind}:{owner_id}")
        missing = sorted(require_declared_coverage - ref_packages)
        foreign = sorted(ref_packages - declared_packages)
        if missing or foreign:
            owner_module_mismatches.append(
                {
                    "owner": f"{owner_kind}:{owner_id}",
                    "declared_packages": sorted(declared_packages),
                    "module_packages": sorted(ref_packages),
                    "declared_packages_without_module": missing,
                    "module_packages_not_declared": foreign,
                }
            )

    for identity, row in table_rows(section_doc, "section", "id").items():
        declared = set(row.get("primary_packages", []))
        register_refs("section", identity, row.get("modules"), declared, declared)

    for identity, row in table_rows(capability_doc, "cell", "id").items():
        primary = set(row.get("primary_packages", []))
        declared = primary | set(row.get("supporting_packages", [])) | set(row.get("state_owner_packages", []))
        register_refs("capability", identity, row.get("modules"), declared, primary)

    for identity, row in table_rows(invariant_doc, "invariant", "id").items():
        declared = set(row.get("enforcement_packages", []))
        register_refs("invariant", identity, row.get("modules"), declared, declared)

    for identity, row in table_rows(delivery_doc, "slice", "id").items():
        declared = set(row.get("primary_packages", []))
        register_refs("delivery", identity, row.get("modules"), declared, declared)

    for identity, row in table_rows(port_doc, "port", "name").items():
        package = row.get("implementation_package")
        module = row.get("implementation_module")
        if isinstance(package, str) and isinstance(module, str):
            register_refs("port", identity, [f"{package}:{module}"], {package}, {package})

    schema_owner_pairs = 0
    for packet in schema_doc.get("packet", []):
        document = load(packet["path"])
        for group in document.get("group", []):
            identity = f"{packet['path']}::{group.get('id')}"
            declared: set[str] = set()
            refs: list[str] = []
            for prefix in ("shape_owner", "meaning_owner", "state_owner"):
                package = group.get(f"{prefix}_package")
                module = group.get(f"{prefix}_module")
                if package == "NONE" and module == "NONE":
                    continue
                if isinstance(package, str) and isinstance(module, str):
                    declared.add(package)
                    refs.append(f"{package}:{module}")
                    schema_owner_pairs += 1
            register_refs("schema", identity, refs, declared, declared)

    config_without_module: list[str] = []
    for identity, row in table_rows(config_doc, "section", "name").items():
        owner = row.get("owner")
        module = row.get("owner_module")
        if not isinstance(module, str):
            config_without_module.append(identity)
        elif isinstance(owner, str):
            register_refs("config", identity, [f"{owner}:{module}"], {owner}, {owner})

    recipe_without_modules: list[str] = []
    for identity, row in table_rows(recipe_doc, "recipe", "id").items():
        owners = set(row.get("primary_execution_packages", []))
        refs = row.get("execution_modules")
        if not isinstance(refs, list) or not refs:
            recipe_without_modules.append(identity)
        else:
            register_refs("recipe", identity, refs, owners, owners)

    function_sources: dict[str, str] = {}
    function_sources.update({package: row["functions"] for package, row in function_rows.items()})
    function_sources.update({package: row["primary_contract"] for package, row in foundation_rows.items()})

    operations: dict[str, list[str]] = {}
    operation_sections: dict[str, dict[str, list[str]]] = {}
    for package, path in sorted(function_sources.items()):
        text = read(path)
        operations[package] = operation_names(text)
        operation_sections[package] = section_operations(text)

    operation_route_path = manifest.get("operation_module_registry")
    operation_route_rows: dict[str, dict[str, Any]] = {}
    if isinstance(operation_route_path, str) and (ROOT / operation_route_path).is_file():
        operation_route_rows = table_rows(load(operation_route_path), "operation", "id")

    expected_operation_ids = {
        f"{package}::{operation}"
        for package, names in operations.items()
        for operation in names
    }
    actual_operation_ids = set(operation_route_rows)
    operations_without_module_route = sorted(expected_operation_ids - actual_operation_ids)
    orphan_operation_routes = sorted(actual_operation_ids - expected_operation_ids)
    invalid_operation_routes: list[dict[str, str]] = []
    for identity, row in operation_route_rows.items():
        package = identity.split("::", 1)[0]
        module = row.get("module")
        if not isinstance(module, str) or module not in modules.get(package, set()):
            invalid_operation_routes.append({"operation": identity, "module": str(module)})
        else:
            module_references[f"{package}:{module}"].append(f"operation:{identity}")

    all_declared_modules = {
        f"{package}:{module}"
        for package, names in modules.items()
        for module in names
    }
    unreferenced_modules = sorted(all_declared_modules - set(module_references))

    files = tracked_files()
    normative_files = [path for path in files if is_normative_markdown(path)]
    node_registry_path = manifest.get("documentation_node_registry")
    node_rows: dict[str, dict[str, Any]] = {}
    if isinstance(node_registry_path, str) and (ROOT / node_registry_path).is_file():
        node_rows = table_rows(load(node_registry_path), "node", "id")

    expected_nodes: dict[str, dict[str, Any]] = {}
    for path in normative_files:
        for heading in markdown_headings(path):
            node_id = f"{path}#L{heading['line']}"
            expected_nodes[node_id] = {"path": path, **heading}
    actual_nodes = set(node_rows)
    documentation_nodes_without_route = sorted(set(expected_nodes) - actual_nodes)
    orphan_documentation_routes = sorted(actual_nodes - set(expected_nodes))
    invalid_documentation_routes: list[dict[str, Any]] = []
    for identity, row in node_rows.items():
        refs = row.get("modules")
        declared = set(row.get("packages", []))
        before = len(invalid_module_refs)
        register_refs("doc", identity, refs, declared, declared)
        if len(invalid_module_refs) > before:
            invalid_documentation_routes.append({"node": identity, "refs": refs})

    principle_nodes = [
        identity
        for identity, row in expected_nodes.items()
        if re.search(r"\b(principle|principles|global rules|core invariants|non-negotiable|invariants)\b", row["title"], flags=re.IGNORECASE)
    ]
    principle_nodes_without_route = sorted(set(principle_nodes) - actual_nodes)

    package_operation_counts = {package: len(names) for package, names in sorted(operations.items())}
    package_module_counts = {package: len(names) for package, names in sorted(modules.items())}

    gaps = {
        "invalid_module_refs": invalid_module_refs,
        "owner_module_mismatches": owner_module_mismatches,
        "config_without_owner_module": sorted(config_without_module),
        "recipes_without_execution_modules": sorted(recipe_without_modules),
        "operations_without_internal_module_route": operations_without_module_route,
        "orphan_operation_routes": orphan_operation_routes,
        "invalid_operation_routes": invalid_operation_routes,
        "unreferenced_declared_modules": unreferenced_modules,
        "documentation_nodes_without_route": documentation_nodes_without_route,
        "orphan_documentation_routes": orphan_documentation_routes,
        "invalid_documentation_routes": invalid_documentation_routes,
        "principle_nodes_without_route": principle_nodes_without_route,
    }
    gap_counts = {key: len(value) for key, value in gaps.items()}

    report = {
        "status": "PASS" if not any(gap_counts.values()) else "GAPS_FOUND",
        "base_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "counts": {
            "packages": len(packages),
            "declared_modules": len(all_declared_modules),
            "operation_sources": len(function_sources),
            "operations": len(expected_operation_ids),
            "normative_markdown_files": len(normative_files),
            "normative_documentation_nodes": len(expected_nodes),
            "principle_nodes": len(principle_nodes),
            "schema_owner_pairs": schema_owner_pairs,
        },
        "gap_counts": gap_counts,
        "gaps": gaps,
        "package_operation_counts": package_operation_counts,
        "package_module_counts": package_module_counts,
        "operation_sections": operation_sections,
    }

    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if args.strict and report["status"] != "PASS" else 0


if __name__ == "__main__":
    raise SystemExit(main())
