from __future__ import annotations

import argparse
import json
import tomllib
from typing import Any

from .control import validate_control
from .schemas import validate_schemas
from .topology import validate_topology


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate ELIOT Search architecture-to-crate coverage closure")
    parser.add_argument("--json", action="store_true", help="emit one machine-readable JSON document")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    errors: list[str] = []
    topology: dict[str, Any] = {}
    schemas: dict[str, Any] = {}
    control: dict[str, Any] = {}

    try:
        topology = validate_topology(errors)
        schemas = validate_schemas(errors, topology)
        control = validate_control(errors, topology, schemas)
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError, KeyError, TypeError) as exc:
        errors.append(f"validator exception: {type(exc).__name__}: {exc}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": topology.get("package_count", 0),
        "package_function_sources": topology.get("function_source_count", 0),
        "foundation_contract_sources": topology.get("foundation_source_count", 0),
        "logical_modules": topology.get("module_total", 0),
        "derived_package_operations": topology.get("operation_count", 0),
        "architecture_sections": topology.get("section_count", 0),
        "capability_cells": topology.get("capability_count", 0),
        "invariants": topology.get("invariant_count", 0),
        "shared_ports": topology.get("port_count", 0),
        "configuration_sections": topology.get("config_count", 0),
        "p00_named_symbols": schemas.get("schema_total", 0),
        "type_registry_symbols": schemas.get("type_registry_symbols", 0),
        "named_type_completions": schemas.get("completion_symbols", 0),
        "recipe_bodies": schemas.get("recipe_count", 0),
        "reason_codes": schemas.get("reason_count", 0),
        "assignment_tasks": control.get("assignment_tasks", 0),
        "delivery_slices": control.get("delivery_slices", 0),
        "qualification_cases": control.get("qualification_cases", 0),
        "errors": errors,
    }

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print("ELIOT Search architecture coverage closure")
        print(
            "packages={packages} modules={modules} operations={operations} "
            "sections={sections} capabilities={capabilities} schemas={schemas} ports={ports}".format(
                packages=result["packages"],
                modules=result["logical_modules"],
                operations=result["derived_package_operations"],
                sections=result["architecture_sections"],
                capabilities=result["capability_cells"],
                schemas=result["p00_named_symbols"],
                ports=result["shared_ports"],
            )
        )
        for error in errors:
            print(f"ERROR: {error}")
        print(result["status"])

    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
