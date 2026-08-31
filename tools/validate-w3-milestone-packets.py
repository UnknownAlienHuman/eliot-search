#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    "search-lexical": ("crates/search-lexical", ["search-contracts", "search-domain", "search-ports", "search-config"], ["LX0", "LX1", "LX2", "LX3"]),
    "search-point-identity": ("crates/search-index-qdrant/search-point-identity", ["search-contracts", "search-domain"], ["PI0", "PI1", "PI2", "PI3"]),
    "search-qdrant-supervisor": ("crates/search-index-qdrant/search-qdrant-supervisor", ["search-contracts", "search-domain", "search-ports", "search-config"], ["QS0", "QS1", "QS2", "QS3"]),
    "search-qdrant-bridge": ("crates/search-index-qdrant/search-qdrant-bridge", ["search-contracts", "search-domain", "search-ports", "search-config"], ["QB0", "QB1", "QB2", "QB3"]),
    "search-epoch-pins": ("crates/search-index-qdrant/search-epoch-pins", ["search-contracts", "search-domain", "search-ports"], ["EP0", "EP1", "EP2", "EP3"]),
    "search-projection-planner": ("crates/search-index-qdrant/search-projection-planner", ["search-contracts", "search-domain", "search-ports", "search-point-identity"], ["PP0", "PP1", "PP2", "PP3"]),
    "search-index-reclaimer": ("crates/search-index-qdrant/search-index-reclaimer", ["search-contracts", "search-domain", "search-ports", "search-config", "search-epoch-pins"], ["IR0", "IR1", "IR2", "IR3"]),
    "search-publication": ("crates/search-index-qdrant/search-publication", ["search-contracts", "search-domain", "search-ports", "search-projection-planner", "search-point-identity"], ["PUB0", "PUB1", "PUB2", "PUB3"]),
    "eliot-searchd": ("bins/eliot-searchd", ["eliot-searchd", "search-lexical", "search-projection-planner", "search-point-identity", "search-qdrant-supervisor", "search-qdrant-bridge", "search-publication", "search-epoch-pins", "search-index-reclaimer"], ["IDX0", "IDX1", "IDX2", "IDX3"]),
}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def rows(document: dict[str, Any], key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get("name"), str):
            raise ValueError(f"invalid {key} row")
        name = row["name"]
        if name in result:
            raise ValueError(f"duplicate {name}")
        result[name] = row
    return result


def main() -> int:
    errors: list[str] = []
    try:
        doc = load("swarm/w3-milestone-packets.toml")
        agents = rows(load("swarm/w3-agent-packets.toml"), "package")
        launch = load("swarm/launch-state.toml")
        artifact = load("qualification/qdrant/artifact.toml")
        collection = load("qualification/qdrant/collection-schema.toml")
        probes = load("qualification/qdrant/probes.toml")
        cases = load("qualification/w3-milestones/cases-v1.toml")
        packet_rows = rows(doc, "package")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    if set(packet_rows) != set(EXPECTED):
        errors.append("package set mismatch")
    if doc.get("package_count") != 9 or doc.get("milestone_count") != 36:
        errors.append("package/milestone count mismatch")
    if doc.get("status") != "BLOCKED_ON_G1_W2_G1_AND_QDRANT_QUALIFICATION":
        errors.append("registry is not qualification-blocked")
    if doc.get("requires_accepted_gates") != ["G1"] or doc.get("requires_accepted_receipts") != ["W2_G1"]:
        errors.append("stage prerequisite mismatch")
    if doc.get("one_writer_one_package") is not True or doc.get("sequential_milestones_per_package") is not True:
        errors.append("ownership/order invariant disabled")
    if doc.get("parallel_milestones_within_package") is not False or doc.get("implementation_authorized_by_this_registry") is not False or doc.get("indexed_mode_enabled") is not False:
        errors.append("authority/indexed-mode ceiling failed")
    if launch.get("active_stage") != "P00" or launch.get("active_wave") != 0 or launch.get("authorized_packages") != ["search-contracts"]:
        errors.append("launch authority moved")

    if artifact.get("status") != "UNQUALIFIED" or artifact.get("server", {}).get("version") or artifact.get("client", {}).get("version"):
        errors.append("Qdrant artifact/client selected or qualified")
    if artifact.get("server", {}).get("automatic_download") is not False or artifact.get("server", {}).get("automatic_upgrade") is not False:
        errors.append("automatic Qdrant acquisition enabled")
    if collection.get("status") != "DESIGNED_NOT_EXECUTED" or any(row.get("profile_status") != "UNQUALIFIED" for row in collection.get("sparse_vector", [])):
        errors.append("collection/profile qualification state changed")
    probe_rows = probes.get("probe", [])
    if probes.get("status") != "NOT_EXECUTED" or not isinstance(probe_rows, list) or any(row.get("mandatory") is not True or row.get("result") != "UNAVAILABLE" for row in probe_rows):
        errors.append("mandatory probe state changed")

    all_ids: list[str] = []
    for name, (path, deps, milestone_ids) in EXPECTED.items():
        row = packet_rows.get(name, {})
        agent = agents.get(name, {})
        if row.get("path") != path or row.get("write_scope") != path + "/**":
            errors.append(f"{name}: path/write scope mismatch")
        if row.get("required_handoff_packages") != deps or agent.get("required_handoff_packages") != deps:
            errors.append(f"{name}: dependency handoff mismatch")
        if row.get("milestone_ids") != milestone_ids or row.get("one_active_milestone") is not True or row.get("claimable") is not False:
            errors.append(f"{name}: milestone or claimability mismatch")
        if row.get("phase") != agent.get("phase"):
            errors.append(f"{name}: phase mismatch with agent registry")
        if agent.get("write_scope") != path + "/**":
            errors.append(f"{name}: agent write scope mismatch")
        if agent.get("ticket_draft") != f"swarm/ticket-drafts/w3/{name}.toml" or agent.get("context_draft") != f"swarm/context-drafts/w3/{name}.toml":
            errors.append(f"{name}: agent draft linkage mismatch")
        packet = row.get("packet")
        if not isinstance(packet, str) or not (ROOT / packet).is_file():
            errors.append(f"{name}: packet missing")
        else:
            text = (ROOT / packet).read_text(encoding="utf-8")
            for milestone_id in milestone_ids:
                if f"## {milestone_id} —" not in text:
                    errors.append(f"{name}: missing checkpoint {milestone_id}")
            if "docs/architecture/" in text or "/src/" in text:
                errors.append(f"{name}: forbidden read path in packet")
            if "W1_IMPLEMENTATION_PACKET.md" in text or "W2_IMPLEMENTATION_PACKET.md" in text:
                errors.append(f"{name}: prior-stage packet replay")
            if "submission candidate" not in text:
                errors.append(f"{name}: exit claim ceiling missing")
        all_ids.extend(milestone_ids)

    if len(all_ids) != 36 or len(set(all_ids)) != 36:
        errors.append("milestone IDs are not exactly 36 unique values")

    transition = doc.get("transition", {})
    for key in ("require_raw_command_outcomes", "require_no_blocking_contract_challenge", "require_package_only_diff", "require_dependency_handoff_digests", "require_qualification_receipts_where_applicable", "require_line_budget"):
        if transition.get(key) is not True:
            errors.append(f"transition invariant disabled: {key}")
    if transition.get("advance_launch_state") is not False or transition.get("publish_package_handoff") is not False or transition.get("enable_indexed_mode") is not False:
        errors.append("transition creates authority")

    current = doc.get("current_state", {})
    if any(current.get(key) not in {False, 0, "ABSENT"} for key in current):
        errors.append("current state contains success/authority")

    case_rows = cases.get("case", [])
    if cases.get("case_count") != 20 or not isinstance(case_rows, list) or len(case_rows) != 20 or any(row.get("mandatory") is not True or row.get("result") != "UNAVAILABLE" for row in case_rows):
        errors.append("case inventory mismatch")

    workflow = ROOT / ".github/workflows/w3-milestone-packets.yml"
    if not workflow.is_file():
        errors.append("workflow missing")
    else:
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        for token in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:", "\n  repository_dispatch:"):
            if token in text:
                errors.append(f"automatic trigger {token.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(packet_rows),
        "milestones": len(all_ids),
        "cases": len(case_rows) if isinstance(case_rows, list) else 0,
        "qdrant_probes": len(probe_rows) if isinstance(probe_rows, list) else 0,
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
