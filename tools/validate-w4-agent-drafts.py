#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

EXPECTED = {
    "search-access": {
        "path": "crates/search-query/search-access", "phase": "P08", "soft": 7500, "group": "A",
        "deps": ["search-contracts", "search-domain", "search-ports"], "config": [],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-handles": {
        "path": "crates/search-query/search-handles", "phase": "P08", "soft": 6500, "group": "A",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config"],
        "config": ["config/sections/handles.md"],
        "qualification_sources": ["qualification/query/probes.toml"],
    },
    "search-eval": {
        "path": "crates/search-eval", "phase": "P08", "soft": 7500, "group": "A",
        "deps": ["search-contracts", "search-domain", "search-config"],
        "config": ["config/sections/observability.md"],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-query-planner": {
        "path": "crates/search-query/search-query-planner", "phase": "P08", "soft": 7500, "group": "B",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config", "search-access"],
        "config": ["config/sections/query.md"],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-candidate-validator": {
        "path": "crates/search-query/search-candidate-validator", "phase": "P08", "soft": 7500, "group": "B",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-access"], "config": [],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-retrieval-executor": {
        "path": "crates/search-query/search-retrieval-executor", "phase": "P08", "soft": 7500, "group": "C",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config", "search-query-planner", "search-lexical", "search-epoch-pins", "search-access"],
        "config": ["config/sections/scheduler.md"],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-result-projector": {
        "path": "crates/search-query/search-result-projector", "phase": "P08", "soft": 7000, "group": "C",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-candidate-validator", "search-handles"],
        "config": [],
        "qualification_sources": ["qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
    "search-continuation": {
        "path": "crates/search-query/search-continuation", "phase": "P08", "soft": 6000, "group": "C",
        "deps": ["search-contracts", "search-domain", "search-ports", "search-config", "search-query-planner", "search-access", "search-epoch-pins"],
        "config": ["config/sections/continuations.md"],
        "qualification_sources": ["qualification/query/probes.toml"],
    },
    "eliot-searchd": {
        "path": "bins/eliot-searchd", "phase": "P08", "soft": 6500, "group": "D",
        "deps": ["eliot-searchd", "search-access", "search-query-planner", "search-retrieval-executor", "search-candidate-validator", "search-handles", "search-result-projector", "search-continuation", "search-eval"],
        "config": [],
        "qualification_sources": ["docs/handoff/W4_DAEMON_REENTRY.md", "qualification/query/baseline.toml", "qualification/query/probes.toml"],
    },
}

CENTRAL_W4_PACKAGES = [
    "search-access", "search-query-planner", "search-retrieval-executor",
    "search-candidate-validator", "search-handles", "search-result-projector",
    "search-continuation", "search-eval", "eliot-searchd",
]


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


def machine_files(path: str) -> list[str]:
    root = ROOT / path
    if not root.exists():
        return []
    ignored = {"README.md", ".gitkeep", ".gitignore"}
    return sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name not in ignored)


def main() -> int:
    errors: list[str] = []
    try:
        packet_doc = load("swarm/w4-agent-packets.toml")
        ticket_manifest = load("swarm/ticket-drafts/w4/manifest.toml")
        context_manifest = load("swarm/context-drafts/w4/manifest.toml")
        crates = rows(load("swarm/crates.toml"), "package", "name")
        functions = rows(load("swarm/function-packets.toml"), "package", "name")
        stages = rows(load("swarm/stages.toml"), "stage", "id")
        readsets = rows(load("swarm/stage-readsets.toml"), "override", "id")
        launch = load("swarm/launch-state.toml")
        baseline = load("qualification/query/baseline.toml")
        probes = load("qualification/query/probes.toml")
        cases = load("qualification/w4-agent-drafts/cases-v1.toml")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    packet_rows = rows(packet_doc, "package", "name")
    ticket_rows = rows(ticket_manifest, "draft", "package")
    context_rows = rows(context_manifest, "draft", "package")
    expected_names = set(EXPECTED)
    for label, actual in (("packet", set(packet_rows)), ("ticket", set(ticket_rows)), ("context", set(context_rows))):
        if actual != expected_names:
            errors.append(f"{label} package set mismatch: {sorted(actual ^ expected_names)}")

    if packet_doc.get("status") != "BLOCKED_ON_G1_W3_AND_QUERY_QUALIFICATION":
        errors.append("W4 packet registry is not fail-closed")
    if packet_doc.get("requires_accepted_gates") != ["G1"] or packet_doc.get("requires_accepted_receipts") != ["W3"]:
        errors.append("W4 prerequisite mismatch")
    if packet_doc.get("one_writer_one_package") is not True or packet_doc.get("implementation_authorized_by_this_registry") is not False:
        errors.append("W4 ownership or authority ceiling invalid")
    if packet_doc.get("query_product_enabled") is not False:
        errors.append("W4 registry enables query product")

    w4 = stages.get("W4", {})
    if w4.get("status") != "BLOCKED" or w4.get("packages") != CENTRAL_W4_PACKAGES:
        errors.append("central W4 stage mismatch")
    if w4.get("requires_accepted_gates") != ["G1"] or w4.get("requires_accepted_receipts") != ["W3"]:
        errors.append("central W4 prerequisites mismatch")
    if launch.get("active_stage") != "P00" or launch.get("active_wave") != 0 or launch.get("authorized_packages") != ["search-contracts"]:
        errors.append("launch authority moved from P00/W0")

    execution = packet_doc.get("execution", {})
    if execution.get("group_order") != ["A", "B", "C", "D"]:
        errors.append("execution group order mismatch")
    for group in ("A", "B", "C", "D"):
        actual = set(execution.get(f"group_{group}_packages", []))
        expected = {name for name, spec in EXPECTED.items() if spec["group"] == group}
        if actual != expected:
            errors.append(f"group {group} mismatch")
    if execution.get("query_planner_requires") != ["search-access"] or execution.get("candidate_validator_requires") != ["search-access"]:
        errors.append("group B predecessor mismatch")
    if execution.get("retrieval_executor_requires") != ["search-query-planner", "search-access", "search-lexical", "search-epoch-pins"]:
        errors.append("retrieval executor predecessor mismatch")
    if execution.get("result_projector_requires") != ["search-candidate-validator", "search-handles"]:
        errors.append("result projector predecessor mismatch")
    if execution.get("continuation_requires") != ["search-query-planner", "search-access", "search-epoch-pins"]:
        errors.append("continuation predecessor mismatch")

    override = readsets.get("W4.eliot-searchd", {})
    if override.get("replace_previous_stage_context") is not True or override.get("accepted_prior_stage_handoff_only") is not True:
        errors.append("W4 daemon replacement semantics missing")
    if override.get("required_prior_handoffs") != ["accepted_eliot-searchd_W3_API", "accepted_W3_receipt"]:
        errors.append("W4 daemon prior handoffs mismatch")
    if override.get("write_scope") != "bins/eliot-searchd/**" or override.get("dependency_implementation_reads_allowed") is not False:
        errors.append("W4 daemon override scope/read boundary mismatch")

    if baseline.get("status") != "DESIGNED_NOT_EXECUTED" or baseline.get("implementation_authorized") is not False or baseline.get("runtime_evidence_available") is not False:
        errors.append("query baseline has premature implementation/evidence state")
    if baseline.get("w4_status") != "BLOCKED" or baseline.get("query_product_status") != "UNAVAILABLE":
        errors.append("query baseline verdict state is not blocked/unavailable")
    probe_rows = probes.get("probe", [])
    if probes.get("status") != "NOT_EXECUTED" or not isinstance(probe_rows, list) or any(row.get("mandatory") is not True or row.get("result") != "UNAVAILABLE" for row in probe_rows):
        errors.append("query probe registry has premature evidence")

    for name, spec in EXPECTED.items():
        packet = packet_rows.get(name, {})
        crate = crates.get(name, {})
        function = functions.get(name, {})
        ticket = load(f"swarm/ticket-drafts/w4/{name}.toml")
        context = load(f"swarm/context-drafts/w4/{name}.toml")

        for path in (packet.get("assignment"), packet.get("functions"), f"{spec['path']}/AGENTS.md", f"{spec['path']}/Cargo.toml", f"{spec['path']}/README.md", *spec["config"], *spec["qualification_sources"]):
            if not isinstance(path, str) or not (ROOT / path).is_file():
                errors.append(f"{name}: missing referenced file {path}")

        if packet.get("path") != spec["path"] or packet.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: packet path/write scope mismatch")
        if packet.get("phase") != spec["phase"] or packet.get("execution_group") != spec["group"]:
            errors.append(f"{name}: phase/group mismatch")
        if packet.get("required_handoff_packages") != spec["deps"] or packet.get("config_packets") != spec["config"]:
            errors.append(f"{name}: dependency/config packet mismatch")
        if packet.get("soft_src_lines") != spec["soft"] or packet.get("hard_total_lines") != 10000 or packet.get("claimable") is not False:
            errors.append(f"{name}: line budget or claimability mismatch")
        expected_wave = 1 if name == "eliot-searchd" else 4
        if crate.get("path") != spec["path"] or crate.get("wave") != expected_wave:
            errors.append(f"{name}: crate registry mismatch")
        if function.get("write_scope") != spec["path"] + "/**":
            errors.append(f"{name}: function registry write scope mismatch")

        if ticket.get("status") != "DRAFT_ONLY_NOT_ISSUED" or ticket.get("claimable") is not False or ticket.get("authorizes_implementation") is not False or ticket.get("creates_lease") is not False:
            errors.append(f"{name}: ticket draft creates authority")
        if ticket.get("stage") != "W4" or ticket.get("wave") != 4 or ticket.get("phase") != spec["phase"]:
            errors.append(f"{name}: ticket stage mismatch")
        if ticket.get("repository_fence", {}).get("write_scope") != spec["path"] + "/**" or ticket.get("dependencies", {}).get("required_handoff_packages") != spec["deps"]:
            errors.append(f"{name}: ticket scope/dependency mismatch")
        unresolved = ticket.get("unresolved_identity", {})
        if unresolved.get("base_commit") != "UNSELECTED" or unresolved.get("writer") != "UNASSIGNED" or unresolved.get("reviewer") != "UNASSIGNED":
            errors.append(f"{name}: ticket identity prematurely resolved")
        if ticket.get("stage_prerequisites", {}).get("status") != "UNAVAILABLE":
            errors.append(f"{name}: stage prerequisites prematurely accepted")
        qualification = ticket.get("qualification", {})
        if qualification.get("status") != "DESIGNED_NOT_EXECUTED" or qualification.get("mandatory_probe_evidence") != "UNAVAILABLE" or qualification.get("independent_reviewer_receipt") != "ABSENT" or qualification.get("query_product_enabled") is not False:
            errors.append(f"{name}: qualification state is successful or incomplete")

        if context.get("status") != "UNMATERIALIZED_DRAFT" or context.get("claimable") is not False or context.get("authorizes_implementation") is not False:
            errors.append(f"{name}: context draft creates authority")
        if context.get("stage") != "W4" or context.get("wave") != 4 or context.get("phase") != spec["phase"]:
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
        expected_selectors = [f"swarm/crates.toml::package[name={name}]", f"swarm/function-packets.toml::package[name={name}]", "swarm/stages.toml::stage[id=W4]"]
        if selectors != expected_selectors:
            errors.append(f"{name}: selector set mismatch")
        for source in sources:
            if not isinstance(source, str) or not (ROOT / source).is_file():
                errors.append(f"{name}: missing context source {source}")
                continue
            if source.startswith("docs/architecture/") or "/src/" in source:
                errors.append(f"{name}: forbidden architecture/implementation source {source}")
            if source in {"docs/handoff/W1_IMPLEMENTATION_PACKET.md", "docs/handoff/W2_IMPLEMENTATION_PACKET.md", "docs/handoff/W3_IMPLEMENTATION_PACKET.md"}:
                errors.append(f"{name}: prior stage packet replayed")
        if "qualification/query/W4_QUALIFICATION.md" not in sources:
            errors.append(f"{name}: W4 qualification contract absent from context")
        for required in spec["qualification_sources"]:
            if required not in sources:
                errors.append(f"{name}: qualification source absent {required}")
        if name == "eliot-searchd" and "docs/handoff/W4_DAEMON_REENTRY.md" not in sources:
            errors.append("eliot-searchd: re-entry packet absent")

    if ticket_manifest.get("draft_count") != 9 or ticket_manifest.get("issued_ticket_count") != 0 or ticket_manifest.get("active_lease_count") != 0:
        errors.append("W4 ticket manifest counts invalid")
    if context_manifest.get("draft_count") != 9 or context_manifest.get("materialized_context_count") != 0:
        errors.append("W4 context manifest counts invalid")
    if packet_doc.get("current_state", {}).get("accepted_W4_package_handoffs") != 0 or packet_doc.get("current_state", {}).get("W4_G2_receipt") != "ABSENT":
        errors.append("W4 packet current state is non-zero")

    for protected in ("swarm/tickets", "swarm/leases", "swarm/submissions", "swarm/reviews", "swarm/handoffs", "swarm/supersessions"):
        if machine_files(protected):
            errors.append(f"issued control records exist under {protected}")

    case_rows = cases.get("case", [])
    if cases.get("case_count") != 24 or not isinstance(case_rows, list) or len(case_rows) != 24 or any(row.get("mandatory") is not True or row.get("result") != "UNAVAILABLE" for row in case_rows):
        errors.append("qualification case inventory mismatch")

    workflow = ROOT / ".github/workflows/w4-agent-drafts.yml"
    if not workflow.is_file():
        errors.append("missing manual workflow")
    else:
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        for forbidden in ("\n  push:", "\n  pull_request:", "\n  schedule:", "\n  workflow_run:", "\n  repository_dispatch:"):
            if forbidden in text:
                errors.append(f"automatic workflow trigger: {forbidden.strip()}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "packages": len(EXPECTED),
        "ticket_drafts": len(ticket_rows),
        "context_drafts": len(context_rows),
        "qualification_cases": len(case_rows) if isinstance(case_rows, list) else 0,
        "query_probes": len(probe_rows) if isinstance(probe_rows, list) else 0,
        "launch_stage": launch.get("active_stage"),
        "launch_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
