#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

EXPECTED = {
    "search-config": {
        "path": "crates/search-config",
        "phase": "P01",
        "soft": 5500,
        "deps": ["search-contracts"],
        "config": [],
        "group": "A",
    },
    "search-runtime-owner": {
        "path": "crates/search-runtime/search-runtime-owner",
        "phase": "P01",
        "soft": 4500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/instance.md"],
        "group": "B",
    },
    "search-os-secrets": {
        "path": "crates/search-runtime/search-os-secrets",
        "phase": "P01",
        "soft": 3500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/secrets.md"],
        "group": "B",
    },
    "search-control-redb": {
        "path": "crates/search-control-redb",
        "phase": "P02",
        "soft": 7500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/control.md"],
        "group": "B",
    },
    "search-provider-protocol": {
        "path": "crates/search-provider-protocol",
        "phase": "P02",
        "soft": 7500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/protocol.md"],
        "group": "B",
    },
    "eliot-searchd": {
        "path": "bins/eliot-searchd",
        "phase": "P02",
        "soft": 6500,
        "deps": [
            "search-contracts", "search-domain", "search-ports", "search-config",
            "search-runtime-owner", "search-os-secrets", "search-control-redb",
            "search-provider-protocol",
        ],
        "config": ["config/sections/optional_profiles.md"],
        "group": "C",
    },
    "eliot-search": {
        "path": "bins/eliot-search",
        "phase": "P02",
        "soft": 4500,
        "deps": ["search-contracts", "search-ports", "search-config", "search-provider-protocol"],
        "config": [],
        "group": "C",
    },
}

def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))

def rows(document: dict[str, Any], key: str, name_key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} is not an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(name_key), str):
            raise ValueError(f"invalid {key} row")
        name = row[name_key]
        if name in result:
            raise ValueError(f"duplicate {key} {name}")
        result[name] = row
    return result

def main() -> int:
    errors: list[str] = []
    try:
        packet_doc = load("swarm/w1-agent-packets.toml")
        ticket_manifest = load("swarm/ticket-drafts/w1/manifest.toml")
        context_manifest = load("swarm/context-drafts/w1/manifest.toml")
        crates = rows(load("swarm/crates.toml"), "package", "name")
        functions = rows(load("swarm/function-packets.toml"), "package", "name")
        stages = rows(load("swarm/stages.toml"), "stage", "id")
        launch = load("swarm/launch-state.toml")
        cases = load("qualification/w1-agent-drafts/cases-v1.toml")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    packet_rows = rows(packet_doc, "package", "name")
    ticket_rows = rows(ticket_manifest, "draft", "package")
    context_rows = rows(context_manifest, "draft", "package")
    expected_names = set(EXPECTED)
    for label, actual in (
        ("packet", set(packet_rows)),
        ("ticket", set(ticket_rows)),
        ("context", set(context_rows)),
    ):
        if actual != expected_names:
            errors.append(f"{label} package set mismatch: {sorted(actual ^ expected_names)}")

    if packet_doc.get("status") != "BLOCKED_ON_G0_AND_W0":
        errors.append("W1 packet registry is not blocked")
    if packet_doc.get("requires_accepted_gates") != ["G0"] or packet_doc.get("requires_accepted_receipts") != ["W0"]:
        errors.append("W1 prerequisites mismatch")
    if packet_doc.get("one_writer_one_package") is not True:
        errors.append("one-writer-one-package disabled")
    if packet_doc.get("parallel_milestones_within_package") is not False:
        errors.append("parallel milestones within a package must be false")
    if packet_doc.get("implementation_authorized_by_this_registry") is not False:
        errors.append("packet registry authorizes implementation")

    w1 = stages.get("W1", {})
    if w1.get("status") != "BLOCKED" or w1.get("packages") != list(EXPECTED):
        errors.append("central W1 stage mismatch")
    if w1.get("requires_accepted_gates") != ["G0"] or w1.get("requires_accepted_receipts") != ["W0"]:
        errors.append("central W1 prerequisites mismatch")

    if launch.get("active_stage") != "P00" or launch.get("active_wave") != 0:
        errors.append("launch authority moved from P00/W0")
    if launch.get("authorized_packages") != ["search-contracts"]:
        errors.append("authorized package set changed")

    for name, spec in EXPECTED.items():
        packet = packet_rows.get(name, {})
        crate = crates.get(name, {})
        function = functions.get(name, {})
        ticket = load(f"swarm/ticket-drafts/w1/{name}.toml")
        context = load(f"swarm/context-drafts/w1/{name}.toml")

        for path in (
            packet.get("assignment"), packet.get("functions"),
            f"{spec['path']}/AGENTS.md", f"{spec['path']}/Cargo.toml",
            f"{spec['path']}/README.md",
            *spec["config"],
        ):
            if not isinstance(path, str) or not (ROOT / path).is_file():
                errors.append(f"{name}: missing referenced file {path}")

        if packet.get("path") != spec["path"] or packet.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: packet path/write scope mismatch")
        if packet.get("phase") != spec["phase"] or packet.get("execution_group") != spec["group"]:
            errors.append(f"{name}: phase/group mismatch")
        if packet.get("required_handoff_packages") != spec["deps"]:
            errors.append(f"{name}: packet dependency handoffs mismatch")
        if packet.get("config_packets") != spec["config"]:
            errors.append(f"{name}: config packet mismatch")
        if packet.get("soft_src_lines") != spec["soft"] or packet.get("hard_total_lines") != 10000:
            errors.append(f"{name}: line budget mismatch")
        if crate.get("path") != spec["path"] or crate.get("wave") != 1:
            errors.append(f"{name}: crate registry mismatch")
        if function.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: function registry write scope mismatch")

        if ticket.get("status") != "DRAFT_ONLY_NOT_ISSUED" or ticket.get("claimable") is not False:
            errors.append(f"{name}: ticket draft became claimable")
        if ticket.get("authorizes_implementation") is not False or ticket.get("creates_lease") is not False:
            errors.append(f"{name}: ticket draft creates authority")
        if ticket.get("stage") != "W1" or ticket.get("wave") != 1 or ticket.get("phase") != spec["phase"]:
            errors.append(f"{name}: ticket stage mismatch")
        if ticket.get("repository_fence", {}).get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: ticket write scope mismatch")
        if ticket.get("dependencies", {}).get("required_handoff_packages") != spec["deps"]:
            errors.append(f"{name}: ticket dependency handoffs mismatch")
        unresolved = ticket.get("unresolved_identity", {})
        if unresolved.get("base_commit") != "UNSELECTED" or unresolved.get("writer") != "UNASSIGNED" or unresolved.get("reviewer") != "UNASSIGNED":
            errors.append(f"{name}: ticket identity prematurely resolved")
        if ticket.get("stage_prerequisites", {}).get("status") != "UNAVAILABLE":
            errors.append(f"{name}: stage prerequisites prematurely accepted")

        if context.get("status") != "UNMATERIALIZED_DRAFT" or context.get("claimable") is not False:
            errors.append(f"{name}: context draft became claimable")
        if context.get("authorizes_implementation") is not False:
            errors.append(f"{name}: context draft authorizes implementation")
        if context.get("stage") != "W1" or context.get("wave") != 1 or context.get("phase") != spec["phase"]:
            errors.append(f"{name}: context stage mismatch")
        content = context.get("content", {})
        sources = content.get("source_files", [])
        selectors = content.get("registry_fragments", [])
        slots = content.get("accepted_handoff_slots", [])
        if context.get("source_file_count") != len(sources) or len(sources) > 16:
            errors.append(f"{name}: source count/ceiling mismatch")
        if context.get("registry_fragment_count") != len(selectors) or len(selectors) > 4:
            errors.append(f"{name}: selector count/ceiling mismatch")
        if context.get("accepted_handoff_slot_count") != len(slots) or len(slots) != len(spec["deps"]):
            errors.append(f"{name}: handoff slot count mismatch")
        expected_slots = [f"{dep}::accepted_package_and_api_handoff" for dep in spec["deps"]]
        if slots != expected_slots:
            errors.append(f"{name}: handoff slots mismatch")
        for source in sources:
            if not isinstance(source, str) or not (ROOT / source).is_file():
                errors.append(f"{name}: missing context source {source}")
            if source.startswith("docs/architecture/"):
                errors.append(f"{name}: architecture master in context")
            if "/src/" in source or source.endswith("/src"):
                errors.append(f"{name}: implementation source in context")
        if selectors != [
            f"swarm/crates.toml::package[name={name}]",
            f"swarm/function-packets.toml::package[name={name}]",
            "swarm/stages.toml::stage[id=W1]",
        ]:
            errors.append(f"{name}: selector set mismatch")

    if ticket_manifest.get("draft_count") != 7 or ticket_manifest.get("issued_ticket_count") != 0:
        errors.append("W1 ticket manifest counts invalid")
    if context_manifest.get("draft_count") != 7 or context_manifest.get("materialized_context_count") != 0:
        errors.append("W1 context manifest counts invalid")
    if cases.get("case_count") != 20 or len(cases.get("case", [])) != 20:
        errors.append("qualification case inventory mismatch")

    workflow = ROOT / ".github/workflows/w1-agent-drafts.yml"
    if not workflow.is_file():
        errors.append("missing manual workflow")
    else:
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        for forbidden in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:"):
            if forbidden in text:
                errors.append(f"automatic workflow trigger: {forbidden.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(EXPECTED),
        "ticket_drafts": len(ticket_rows),
        "context_drafts": len(context_rows),
        "qualification_cases": len(cases.get("case", [])),
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1

if __name__ == "__main__":
    raise SystemExit(main())
