#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

EXPECTED: dict[str, dict[str, Any]] = {
    "search-source-admission": {
        "path": "crates/search-source/search-source-admission",
        "phase": "P03",
        "base_wave": 2,
        "soft": 4500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/source_admission.md"],
        "group": "A",
    },
    "search-source-identity": {
        "path": "crates/search-source/search-source-identity",
        "phase": "P03",
        "base_wave": 2,
        "soft": 6500,
        "deps": ["search-contracts", "search-domain"],
        "config": [],
        "group": "A",
    },
    "search-safe-reader": {
        "path": "crates/search-source/search-safe-reader",
        "phase": "P03",
        "base_wave": 2,
        "soft": 6500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/source_reader.md"],
        "group": "A",
    },
    "search-revision-store": {
        "path": "crates/search-source/search-revision-store",
        "phase": "P04",
        "base_wave": 2,
        "soft": 7500,
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/revision_store.md"],
        "group": "A",
    },
    "search-materializer": {
        "path": "crates/search-prep/search-materializer",
        "phase": "P04",
        "base_wave": 2,
        "soft": 7000,
        "deps": ["search-contracts", "search-domain", "search-ports"],
        "config": [],
        "group": "A",
    },
    "search-unitizer": {
        "path": "crates/search-prep/search-unitizer",
        "phase": "P04",
        "base_wave": 2,
        "soft": 6500,
        "deps": ["search-contracts", "search-domain", "search-ports"],
        "config": [],
        "group": "A",
    },
    "search-source-registry": {
        "path": "crates/search-source/search-source-registry",
        "phase": "P03",
        "base_wave": 2,
        "soft": 6500,
        "deps": [
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-source-identity",
            "search-source-admission",
        ],
        "config": [],
        "group": "B",
    },
    "eliot-searchd": {
        "path": "bins/eliot-searchd",
        "phase": "P04",
        "base_wave": 1,
        "soft": 6500,
        "deps": [
            "eliot-searchd",
            "search-source-admission",
            "search-source-registry",
            "search-source-identity",
            "search-safe-reader",
            "search-revision-store",
            "search-materializer",
            "search-unitizer",
        ],
        "config": [],
        "group": "C",
    },
}

CENTRAL_W2_PACKAGES = [
    "search-source-admission",
    "search-source-registry",
    "search-source-identity",
    "search-safe-reader",
    "search-revision-store",
    "search-materializer",
    "search-unitizer",
    "eliot-searchd",
]


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def indexed_rows(document: dict[str, Any], key: str, identity: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} is not an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(identity), str):
            raise ValueError(f"invalid {key} row")
        name = row[identity]
        if name in result:
            raise ValueError(f"duplicate {key} identity: {name}")
        result[name] = row
    return result


def require_regular_file(errors: list[str], owner: str, value: Any) -> None:
    if not isinstance(value, str) or not (ROOT / value).is_file():
        errors.append(f"{owner}: missing referenced file {value}")


def main() -> int:
    errors: list[str] = []
    try:
        packet_doc = load("swarm/w2-agent-packets.toml")
        ticket_manifest = load("swarm/ticket-drafts/w2/manifest.toml")
        context_manifest = load("swarm/context-drafts/w2/manifest.toml")
        crates = indexed_rows(load("swarm/crates.toml"), "package", "name")
        functions = indexed_rows(load("swarm/function-packets.toml"), "package", "name")
        stages = indexed_rows(load("swarm/stages.toml"), "stage", "id")
        overrides = indexed_rows(load("swarm/stage-readsets.toml"), "override", "id")
        launch = load("swarm/launch-state.toml")
        cases = load("qualification/w2-agent-drafts/cases-v1.toml")
        packet_rows = indexed_rows(packet_doc, "package", "name")
        ticket_rows = indexed_rows(ticket_manifest, "draft", "package")
        context_rows = indexed_rows(context_manifest, "draft", "package")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    expected_names = set(EXPECTED)
    for label, actual in (
        ("packet", set(packet_rows)),
        ("ticket", set(ticket_rows)),
        ("context", set(context_rows)),
    ):
        if actual != expected_names:
            errors.append(f"{label} package set mismatch: {sorted(actual ^ expected_names)}")

    if packet_doc.get("status") != "BLOCKED_ON_G0_AND_W1":
        errors.append("W2 packet registry is not blocked on G0/W1")
    if packet_doc.get("requires_accepted_gates") != ["G0"]:
        errors.append("W2 gate prerequisite mismatch")
    if packet_doc.get("requires_accepted_receipts") != ["W1"]:
        errors.append("W2 receipt prerequisite mismatch")
    if packet_doc.get("one_writer_one_package") is not True:
        errors.append("one-writer-one-package invariant disabled")
    if packet_doc.get("implementation_authorized_by_this_registry") is not False:
        errors.append("W2 packet registry authorizes implementation")

    w2 = stages.get("W2", {})
    if w2.get("status") != "BLOCKED":
        errors.append("central W2 stage is not BLOCKED")
    if w2.get("packages") != CENTRAL_W2_PACKAGES:
        errors.append("central W2 package order/set mismatch")
    if w2.get("requires_accepted_gates") != ["G0"]:
        errors.append("central W2 gate prerequisite mismatch")
    if w2.get("requires_accepted_receipts") != ["W1"]:
        errors.append("central W2 receipt prerequisite mismatch")

    if launch.get("active_stage") != "P00" or launch.get("active_wave") != 0:
        errors.append("launch authority moved from P00/W0")
    if launch.get("authorized_packages") != ["search-contracts"]:
        errors.append("current authorized package set changed")

    execution = packet_doc.get("execution")
    if not isinstance(execution, dict):
        errors.append("missing W2 execution table")
    else:
        if execution.get("group_order") != ["A", "B", "C"]:
            errors.append("W2 group order mismatch")
        if execution.get("group_A_packages") != [
            "search-source-admission",
            "search-source-identity",
            "search-safe-reader",
            "search-revision-store",
            "search-materializer",
            "search-unitizer",
        ]:
            errors.append("W2 group A mismatch")
        if execution.get("group_B_packages") != ["search-source-registry"]:
            errors.append("W2 group B mismatch")
        if execution.get("group_C_packages") != ["eliot-searchd"]:
            errors.append("W2 group C mismatch")
        if execution.get("group_B_requires_accepted_handoffs") != [
            "search-source-admission",
            "search-source-identity",
        ]:
            errors.append("source registry predecessor set mismatch")
        if execution.get("group_C_requires_all_W2_library_handoffs") is not True:
            errors.append("daemon does not require all W2 library handoffs")
        if execution.get("eliot_searchd_requires_prior_stage_handoffs") != ["eliot-searchd", "W1"]:
            errors.append("daemon prior-stage requirements mismatch")

    daemon_override = overrides.get("W2.eliot-searchd", {})
    if daemon_override.get("replace_previous_stage_context") is not True:
        errors.append("W2 daemon override does not replace previous context")
    if daemon_override.get("accepted_prior_stage_handoff_only") is not True:
        errors.append("W2 daemon override permits prior implementation context")
    if daemon_override.get("required_prior_handoffs") != [
        "accepted_eliot-searchd_W1_API",
        "accepted_W1_receipt",
    ]:
        errors.append("W2 daemon prior handoff list mismatch")
    if daemon_override.get("forbidden_prior_stage_packets") != [
        "docs/handoff/W1_IMPLEMENTATION_PACKET.md"
    ]:
        errors.append("W2 daemon does not forbid W1 packet replay")

    for name, spec in EXPECTED.items():
        packet = packet_rows.get(name, {})
        crate = crates.get(name, {})
        function = functions.get(name, {})
        try:
            ticket = load(f"swarm/ticket-drafts/w2/{name}.toml")
            context = load(f"swarm/context-drafts/w2/{name}.toml")
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            errors.append(f"{name}: unable to load drafts: {exc}")
            continue

        for path in (
            packet.get("assignment"),
            packet.get("functions"),
            f"{spec['path']}/AGENTS.md",
            f"{spec['path']}/Cargo.toml",
            f"{spec['path']}/README.md",
            *spec["config"],
        ):
            require_regular_file(errors, name, path)

        if packet.get("path") != spec["path"]:
            errors.append(f"{name}: packet package path mismatch")
        if packet.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: packet write scope mismatch")
        if packet.get("phase") != spec["phase"] or packet.get("execution_group") != spec["group"]:
            errors.append(f"{name}: phase/execution group mismatch")
        if packet.get("required_handoff_packages") != spec["deps"]:
            errors.append(f"{name}: packet dependency handoffs mismatch")
        if packet.get("config_packets") != spec["config"]:
            errors.append(f"{name}: configuration packet mismatch")
        if packet.get("soft_src_lines") != spec["soft"]:
            errors.append(f"{name}: soft line target mismatch")
        if packet.get("split_review_total_lines") != 8500 or packet.get("hard_total_lines") != 10000:
            errors.append(f"{name}: split/hard line limits mismatch")
        if packet.get("one_active_writer") is not True or packet.get("claimable") is not False:
            errors.append(f"{name}: packet writer/claimability invariant failed")

        if crate.get("path") != spec["path"] or crate.get("wave") != spec["base_wave"]:
            errors.append(f"{name}: crate registry path/base wave mismatch")
        if function.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: function registry write scope mismatch")
        if function.get("assignment") != packet.get("assignment"):
            errors.append(f"{name}: assignment registry mismatch")
        if function.get("functions") != packet.get("functions"):
            errors.append(f"{name}: function packet registry mismatch")

        if ticket.get("status") != "DRAFT_ONLY_NOT_ISSUED" or ticket.get("claimable") is not False:
            errors.append(f"{name}: ticket draft became claimable")
        if ticket.get("authorizes_implementation") is not False or ticket.get("creates_lease") is not False:
            errors.append(f"{name}: ticket draft creates authority")
        if ticket.get("stage") != "W2" or ticket.get("wave") != 2 or ticket.get("phase") != spec["phase"]:
            errors.append(f"{name}: ticket stage/phase mismatch")
        if ticket.get("repository_fence", {}).get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: ticket write scope mismatch")
        if ticket.get("dependencies", {}).get("required_handoff_packages") != spec["deps"]:
            errors.append(f"{name}: ticket dependency handoffs mismatch")
        unresolved = ticket.get("unresolved_identity", {})
        if (
            unresolved.get("base_commit") != "UNSELECTED"
            or unresolved.get("writer") != "UNASSIGNED"
            or unresolved.get("reviewer") != "UNASSIGNED"
        ):
            errors.append(f"{name}: ticket identity prematurely resolved")
        stage_prerequisites = ticket.get("stage_prerequisites", {})
        if (
            stage_prerequisites.get("required_gates") != ["G0"]
            or stage_prerequisites.get("required_receipts") != ["W1"]
            or stage_prerequisites.get("status") != "UNAVAILABLE"
        ):
            errors.append(f"{name}: ticket stage prerequisites mismatch")

        if context.get("status") != "UNMATERIALIZED_DRAFT" or context.get("claimable") is not False:
            errors.append(f"{name}: context draft became claimable")
        if context.get("authorizes_implementation") is not False:
            errors.append(f"{name}: context draft authorizes implementation")
        if context.get("stage") != "W2" or context.get("wave") != 2 or context.get("phase") != spec["phase"]:
            errors.append(f"{name}: context stage/phase mismatch")
        context_prerequisites = context.get("stage_prerequisites", {})
        if (
            context_prerequisites.get("required_gates") != ["G0"]
            or context_prerequisites.get("required_receipts") != ["W1"]
            or context_prerequisites.get("status") != "UNAVAILABLE"
        ):
            errors.append(f"{name}: context stage prerequisites mismatch")

        content = context.get("content", {})
        sources = content.get("source_files", [])
        selectors = content.get("registry_fragments", [])
        slots = content.get("accepted_handoff_slots", [])
        if context.get("source_file_count") != len(sources) or len(sources) > 16:
            errors.append(f"{name}: source count/ceiling mismatch")
        if context.get("registry_fragment_count") != len(selectors) or len(selectors) > 4:
            errors.append(f"{name}: registry fragment count/ceiling mismatch")
        if context.get("accepted_handoff_slot_count") != len(slots) or len(slots) != len(spec["deps"]):
            errors.append(f"{name}: accepted handoff slot count mismatch")
        expected_slots = [f"{dependency}::accepted_package_and_api_handoff" for dependency in spec["deps"]]
        if slots != expected_slots:
            errors.append(f"{name}: accepted handoff slot set/order mismatch")
        expected_selectors = [
            f"swarm/crates.toml::package[name={name}]",
            f"swarm/function-packets.toml::package[name={name}]",
            "swarm/stages.toml::stage[id=W2]",
        ]
        if selectors != expected_selectors:
            errors.append(f"{name}: registry selector set/order mismatch")

        for source in sources:
            if not isinstance(source, str) or not (ROOT / source).is_file():
                errors.append(f"{name}: missing context source {source}")
                continue
            if source.startswith("docs/architecture/"):
                errors.append(f"{name}: architecture master in context")
            if "/src/" in source or source.endswith("/src"):
                errors.append(f"{name}: implementation source in context")
            if source == "docs/handoff/W1_IMPLEMENTATION_PACKET.md":
                errors.append(f"{name}: W1 packet replayed in W2 context")
            lowered = source.lower()
            if "qdrant" in lowered or "w3_implementation_packet" in lowered:
                errors.append(f"{name}: indexed/Qdrant packet entered W2 context")

        forbidden = content.get("forbidden_paths", [])
        for required_forbidden in (
            "docs/architecture/**",
            "docs/handoff/W1_IMPLEMENTATION_PACKET.md",
        ):
            if required_forbidden not in forbidden:
                errors.append(f"{name}: missing forbidden path {required_forbidden}")

        if name == "eliot-searchd":
            if "docs/handoff/W2_DAEMON_REENTRY.md" not in sources:
                errors.append("eliot-searchd: missing W2 re-entry boundary")
            if spec["deps"][0] != "eliot-searchd":
                errors.append("eliot-searchd: prior W1 daemon handoff not first")

    if ticket_manifest.get("draft_count") != 8 or ticket_manifest.get("issued_ticket_count") != 0:
        errors.append("W2 ticket manifest counts invalid")
    if context_manifest.get("draft_count") != 8 or context_manifest.get("materialized_context_count") != 0:
        errors.append("W2 context manifest counts invalid")
    if ticket_manifest.get("requires_accepted_gates") != ["G0"] or ticket_manifest.get("requires_accepted_receipts") != ["W1"]:
        errors.append("W2 ticket manifest prerequisites mismatch")
    if context_manifest.get("requires_accepted_gates") != ["G0"] or context_manifest.get("requires_accepted_receipts") != ["W1"]:
        errors.append("W2 context manifest prerequisites mismatch")

    case_rows = cases.get("case")
    if cases.get("case_count") != 20 or not isinstance(case_rows, list) or len(case_rows) != 20:
        errors.append("W2 qualification case inventory mismatch")
    elif any(
        not isinstance(case, dict)
        or case.get("mandatory") is not True
        or case.get("result") != "UNAVAILABLE"
        for case in case_rows
    ):
        errors.append("W2 qualification cases are not mandatory UNAVAILABLE")

    workflow = ROOT / ".github/workflows/w2-agent-drafts.yml"
    if not workflow.is_file():
        errors.append("missing W2 manual workflow")
    else:
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        for forbidden_trigger in (
            "\n  push:",
            "\n  pull_request:",
            "\n  schedule:",
            "\n  workflow_run:",
        ):
            if forbidden_trigger in text:
                errors.append(f"automatic workflow trigger: {forbidden_trigger.strip()}")

    current_state = packet_doc.get("current_state", {})
    expected_zero_state = {
        "accepted_G0": False,
        "accepted_W1": False,
        "materialized_W2_contexts": 0,
        "issued_W2_tickets": 0,
        "active_W2_leases": 0,
        "accepted_W2_package_handoffs": 0,
        "W2_G1_receipt": "ABSENT",
    }
    if current_state != expected_zero_state:
        errors.append("W2 current-state zero disposition mismatch")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(packet_rows),
        "ticket_drafts": len(ticket_rows),
        "context_drafts": len(context_rows),
        "qualification_cases": len(case_rows) if isinstance(case_rows, list) else 0,
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
