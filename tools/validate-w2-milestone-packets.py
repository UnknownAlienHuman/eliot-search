#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
EXPECTED: dict[str, tuple[str, list[str], list[str]]] = {
    "search-source-admission": (
        "crates/search-source/search-source-admission",
        ["search-contracts", "search-domain", "search-ports", "search-config"],
        ["A0", "A1", "A2", "A3"],
    ),
    "search-source-identity": (
        "crates/search-source/search-source-identity",
        ["search-contracts", "search-domain"],
        ["I0", "I1", "I2", "I3"],
    ),
    "search-safe-reader": (
        "crates/search-source/search-safe-reader",
        ["search-contracts", "search-domain", "search-ports", "search-config"],
        ["SR0", "SR1", "SR2", "SR3"],
    ),
    "search-revision-store": (
        "crates/search-source/search-revision-store",
        ["search-contracts", "search-domain", "search-ports", "search-config"],
        ["V0", "V1", "V2", "V3"],
    ),
    "search-materializer": (
        "crates/search-prep/search-materializer",
        ["search-contracts", "search-domain", "search-ports"],
        ["M0", "M1", "M2", "M3"],
    ),
    "search-unitizer": (
        "crates/search-prep/search-unitizer",
        ["search-contracts", "search-domain", "search-ports"],
        ["U0", "U1", "U2", "U3"],
    ),
    "search-source-registry": (
        "crates/search-source/search-source-registry",
        [
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-source-identity",
            "search-source-admission",
        ],
        ["RG0", "RG1", "RG2", "RG3"],
    ),
    "eliot-searchd": (
        "bins/eliot-searchd",
        [
            "eliot-searchd",
            "search-source-admission",
            "search-source-registry",
            "search-source-identity",
            "search-safe-reader",
            "search-revision-store",
            "search-materializer",
            "search-unitizer",
        ],
        ["D20", "D21", "D22", "D23"],
    ),
}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def rows(document: dict[str, Any], key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get("name"), str):
            raise ValueError(f"invalid {key} row")
        name = row["name"]
        if name in result:
            raise ValueError(f"duplicate {key} row: {name}")
        result[name] = row
    return result


def main() -> int:
    errors: list[str] = []
    try:
        document = load("swarm/w2-milestone-packets.toml")
        packet_rows = rows(document, "package")
        agent_rows = rows(load("swarm/w2-agent-packets.toml"), "package")
        launch = load("swarm/launch-state.toml")
        cases = load("qualification/w2-milestones/cases-v1.toml")
        daemon_context = load("swarm/context-drafts/w2/eliot-searchd.toml")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    if set(packet_rows) != set(EXPECTED):
        errors.append(f"package set mismatch: {sorted(set(packet_rows) ^ set(EXPECTED))}")
    if document.get("status") != "BLOCKED_ON_G0_AND_W1":
        errors.append("W2 milestone registry is not blocked")
    if document.get("requires_accepted_gates") != ["G0"]:
        errors.append("W2 milestone gate prerequisite mismatch")
    if document.get("requires_accepted_receipts") != ["W1"]:
        errors.append("W2 milestone receipt prerequisite mismatch")
    if document.get("package_count") != 8 or document.get("milestone_count") != 32:
        errors.append("W2 package/milestone counts mismatch")
    if document.get("one_writer_one_package") is not True:
        errors.append("one-writer-one-package invariant disabled")
    if document.get("sequential_milestones_per_package") is not True:
        errors.append("sequential milestone invariant disabled")
    if document.get("parallel_milestones_within_package") is not False:
        errors.append("parallel milestones inside package must remain false")
    if document.get("implementation_authorized_by_this_registry") is not False:
        errors.append("W2 milestone registry authorizes implementation")

    if launch.get("active_stage") != "P00" or launch.get("active_wave") != 0:
        errors.append("launch authority moved from P00/W0")
    if launch.get("authorized_packages") != ["search-contracts"]:
        errors.append("current authorized package set changed")

    for name, (path, dependencies, milestone_ids) in EXPECTED.items():
        row = packet_rows.get(name, {})
        agent = agent_rows.get(name, {})
        if row.get("path") != path or row.get("write_scope") != path + "/**":
            errors.append(f"{name}: path/write scope mismatch")
        if row.get("required_handoff_packages") != dependencies:
            errors.append(f"{name}: milestone dependency handoffs mismatch")
        if agent.get("required_handoff_packages") != dependencies:
            errors.append(f"{name}: agent dependency handoffs mismatch")
        if row.get("milestone_ids") != milestone_ids:
            errors.append(f"{name}: milestone IDs/order mismatch")
        if row.get("one_active_milestone") is not True or row.get("claimable") is not False:
            errors.append(f"{name}: active/claimability invariant failed")
        packet = row.get("packet")
        if not isinstance(packet, str) or not (ROOT / packet).is_file():
            errors.append(f"{name}: missing package checkpoint packet")
            continue
        text = (ROOT / packet).read_text(encoding="utf-8")
        for milestone_id in milestone_ids:
            if f"## {milestone_id} —" not in text:
                errors.append(f"{name}: missing checkpoint {milestone_id}")
        if "docs/architecture/" in text or "/src/" in text:
            errors.append(f"{name}: forbidden architecture/implementation read in packet")
        if "docs/handoff/W1_IMPLEMENTATION_PACKET.md" in text:
            errors.append(f"{name}: prior W1 packet replayed")
        if name != "eliot-searchd" and "Qdrant" in text:
            errors.append(f"{name}: indexed dependency mentioned in W2 packet")

    daemon_sources = daemon_context.get("content", {}).get("source_files", [])
    if "docs/handoff/W2_DAEMON_REENTRY.md" not in daemon_sources:
        errors.append("daemon context lacks W2 re-entry boundary")
    if "docs/handoff/W1_IMPLEMENTATION_PACKET.md" in daemon_sources:
        errors.append("daemon W2 context replays W1 implementation packet")
    daemon_text = (ROOT / "docs/handoff/w2-packages/eliot-searchd.md").read_text(encoding="utf-8")
    for token in (
        "accepted prior W1 daemon",
        "Do not replay the W1 implementation packet",
        "DIRECT",
        "D20",
        "D23",
    ):
        if token not in daemon_text:
            errors.append(f"daemon packet missing re-entry token: {token}")

    current_state = document.get("current_state", {})
    expected_zero = {
        "accepted_G0": False,
        "accepted_W1": False,
        "materialized_contexts": 0,
        "issued_tickets": 0,
        "active_leases": 0,
        "accepted_package_handoffs": 0,
        "W2_G1_receipt": "ABSENT",
    }
    if current_state != expected_zero:
        errors.append("W2 milestone zero-state mismatch")

    case_rows = cases.get("case")
    if cases.get("case_count") != 18 or not isinstance(case_rows, list) or len(case_rows) != 18:
        errors.append("qualification case inventory mismatch")
    elif any(
        not isinstance(case, dict)
        or case.get("mandatory") is not True
        or case.get("result") != "UNAVAILABLE"
        for case in case_rows
    ):
        errors.append("qualification cases are not mandatory UNAVAILABLE")

    workflow = ROOT / ".github/workflows/w2-milestone-packets.yml"
    if not workflow.is_file():
        errors.append("missing W2 milestone workflow")
    else:
        workflow_text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in workflow_text:
                errors.append(f"workflow missing {token}")
        for trigger in (
            "\n  push:",
            "\n  pull_request:",
            "\n  schedule:",
            "\n  workflow_run:",
        ):
            if trigger in workflow_text:
                errors.append(f"automatic workflow trigger: {trigger.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(packet_rows),
        "milestones": sum(len(value[2]) for value in EXPECTED.values()),
        "cases": len(case_rows) if isinstance(case_rows, list) else 0,
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
