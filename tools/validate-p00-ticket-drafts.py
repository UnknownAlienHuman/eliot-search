#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ("search-contracts", "search-domain", "search-ports")
CONTROL_ROOTS = (
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/gate-receipts",
    "swarm/wave-receipts",
)


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def rows(document: dict[str, Any], key: str, name_key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(name_key), str):
            raise ValueError(f"invalid {key} row")
        name = row[name_key]
        if name in result:
            raise ValueError(f"duplicate {key} row {name}")
        result[name] = row
    return result


def machine_files(path: str) -> list[str]:
    root = ROOT / path
    if not root.exists():
        return []
    ignored = {"README.md", ".gitkeep", ".gitignore"}
    return sorted(
        file.relative_to(ROOT).as_posix()
        for file in root.rglob("*")
        if file.is_file() and file.name not in ignored
    )


def fail(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.parse_args()

    errors: list[str] = []
    try:
        ticket_manifest = load("swarm/ticket-drafts/manifest.toml")
        context_manifest = load("swarm/context-drafts/manifest.toml")
        contract_manifest = load("docs/contracts/p00/manifest.toml")
        launch = load("swarm/launch-state.toml")
        orchestration = load("swarm/orchestration.toml")
        ticket_rows = rows(ticket_manifest, "draft", "package")
        context_rows = rows(context_manifest, "draft", "package")
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError, ValueError) as exc:
        print(json.dumps({"status": "FAIL", "errors": [str(exc)]}, indent=2))
        return 1

    fail(errors, ticket_manifest.get("schema_version") == 2, "ticket manifest schema must be v2")
    fail(errors, ticket_manifest.get("ticket_draft_schema_version") == 2, "ticket draft schema must be v2")
    fail(errors, ticket_manifest.get("context_draft_manifest_schema_version") == 2, "context draft manifest schema must be v2")
    fail(errors, ticket_manifest.get("status") == "DRAFT_ONLY_NOT_ISSUED", "ticket manifest status changed")
    fail(errors, ticket_manifest.get("draft_count") == 3, "ticket manifest draft count must be 3")
    fail(errors, set(ticket_rows) == set(PACKAGES), "ticket package set mismatch")

    for key in (
        "issued_ticket_count",
        "active_lease_count",
        "submission_count",
        "accepted_review_count",
        "package_handoff_count",
        "wave_receipt_count",
    ):
        fail(errors, ticket_manifest.get(key) == 0, f"{key} must remain zero")

    ticket_invariants = ticket_manifest.get("invariants", {})
    for key in (
        "draft_is_orchestration_state",
        "draft_may_authorize",
        "draft_may_create_lease",
        "draft_may_contain_lease_identity",
        "draft_may_be_writer_acknowledged",
    ):
        fail(errors, ticket_invariants.get(key) is False, f"unsafe ticket invariant {key}")
    for key in (
        "draft_uses_distinct_signed_payload_and_exact_file_digest_slots",
        "issued_ticket_requires_new_record",
        "issued_ticket_requires_exact_base_commit",
        "issued_ticket_requires_materialized_context",
        "issued_ticket_requires_writer_and_reviewer",
        "conditional_ticket_requires_accepted_dependency_handoffs",
    ):
        fail(errors, ticket_invariants.get(key) is True, f"required ticket invariant disabled: {key}")

    fail(errors, context_manifest.get("schema_version") == 2, "context manifest schema must be v2")
    fail(errors, context_manifest.get("context_draft_schema_version") == 2, "context draft schema must be v2")
    fail(errors, context_manifest.get("status") == "NON_CLAIMABLE_CONTEXT_DRAFTS", "context manifest status changed")
    fail(errors, context_manifest.get("draft_count") == 3, "context manifest draft count must be 3")
    fail(errors, context_manifest.get("materialized_context_count") == 0, "materialized context count must remain zero")
    fail(errors, context_manifest.get("writer_visible_artifact_count_per_context") == 1, "each context must be one artifact")
    fail(errors, set(context_rows) == set(PACKAGES), "context package set mismatch")

    ordinary_ceiling = context_manifest.get("ordinary_static_source_file_ceiling")
    exact_pack_ceiling = context_manifest.get("p00_exact_contract_pack_source_file_ceiling")
    fragment_ceiling = context_manifest.get("max_registry_fragments_per_context")
    handoff_ceiling = context_manifest.get("max_accepted_handoff_slots_per_context")
    fail(errors, ordinary_ceiling == 16, "ordinary context ceiling must remain 16")
    fail(errors, exact_pack_ceiling == 24, "P00 exact-pack ceiling must remain 24")
    fail(errors, context_manifest.get("p00_exact_contract_pack_exception_packages") == ["search-contracts"], "P00 exception package mismatch")
    fail(errors, fragment_ceiling == 6, "registry fragment ceiling must remain 6")
    fail(errors, handoff_ceiling == 1, "accepted handoff ceiling must remain 1")

    context_invariants = context_manifest.get("invariants", {})
    for key in (
        "architecture_master_allowed",
        "dependency_implementation_source_allowed",
        "materialized_context_may_be_amended",
        "p00_exception_may_add_ad_hoc_sources",
    ):
        fail(errors, context_invariants.get(key) is False, f"unsafe context invariant {key}")
    for key in (
        "base_commit_required_at_materialization",
        "per_source_sha256_required",
        "registry_selector_must_match_exactly_one_record",
        "accepted_handoff_digests_required_when_declared",
        "canonical_order_required",
        "manifest_and_artifact_identities_are_distinct",
        "p00_exception_requires_manifest_closed_exact_pack",
    ):
        fail(errors, context_invariants.get(key) is True, f"required context invariant disabled: {key}")

    required_names = contract_manifest.get("required_files")
    if not isinstance(required_names, list) or not all(isinstance(item, str) for item in required_names):
        errors.append("P00 required_files must be a string array")
        required_names = []
    fail(errors, len(required_names) == 13, "P00 required_files must contain 13 files")
    fail(errors, len(required_names) == len(set(required_names)), "P00 required_files contains duplicates")
    fail(errors, required_names[:1] == ["README.md"], "README.md must remain first in P00 required_files")
    fail(errors, "TYPE_COMPLETIONS.md" in required_names, "TYPE_COMPLETIONS.md missing from P00 manifest")
    for name in required_names:
        fail(errors, (ROOT / "docs/contracts/p00" / name).is_file(), f"missing P00 contract file {name}")

    fixed_prefix = [
        "AGENTS.md",
        "crates/search-contracts/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        "swarm/assignments/search-contracts.md",
        "docs/handoff/P00_BOOTSTRAP.md",
    ]
    contract_sources = [
        *fixed_prefix,
        "docs/contracts/p00/README.md",
        "docs/contracts/p00/manifest.toml",
        *[f"docs/contracts/p00/{name}" for name in required_names[1:]],
    ]
    domain_sources = [
        "AGENTS.md",
        "crates/search-domain/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        "swarm/assignments/search-domain.md",
        "docs/handoff/P00_BOOTSTRAP.md",
        "docs/contracts/p00/manifest.toml",
        "docs/contracts/p00/CANONICAL_TYPES.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/CONTRACT_CHALLENGES.md",
        "docs/contracts/p00/SOURCE_GRAPH.md",
        "docs/contracts/p00/QUERY_AND_RESULTS.md",
        "docs/contracts/p00/RECIPE_RESULTS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/REASON_CODES.md",
    ]
    port_sources = [
        "AGENTS.md",
        "crates/search-ports/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        "swarm/assignments/search-ports.md",
        "docs/handoff/P00_BOOTSTRAP.md",
        "docs/contracts/p00/manifest.toml",
        "docs/contracts/p00/CANONICAL_TYPES.md",
        "docs/contracts/p00/TYPE_REGISTRY.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/CONTRACT_CHALLENGES.md",
        "docs/contracts/p00/PORT_OPERATIONS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/REASON_CODES.md",
    ]

    expected = {
        "search-contracts": {
            "launch": "AUTHORIZED",
            "precondition": "CURRENTLY_PRESENT",
            "scope": "crates/search-contracts/**",
            "handoffs": [],
            "sources": contract_sources,
            "fragments": [
                "swarm/crates.toml::package[name=search-contracts]",
                "swarm/function-packets.toml::foundation[package=search-contracts]",
                "swarm/stages.toml::stage[id=W0]",
                "swarm/launch-state.toml::authorized_packages[search-contracts]",
            ],
            "ceiling": exact_pack_ceiling,
        },
        "search-domain": {
            "launch": "CONDITIONAL",
            "precondition": "ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED",
            "scope": "crates/search-domain/**",
            "handoffs": ["search-contracts::accepted_package_and_api_handoff"],
            "sources": domain_sources,
            "fragments": [
                "swarm/crates.toml::package[name=search-domain]",
                "swarm/function-packets.toml::foundation[package=search-domain]",
                "swarm/stages.toml::stage[id=W0]",
                "swarm/launch-state.toml::conditional_packages[search-domain]",
                "swarm/launch-state.toml::conditional_activation.search-domain",
            ],
            "ceiling": ordinary_ceiling,
        },
        "search-ports": {
            "launch": "CONDITIONAL",
            "precondition": "ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED",
            "scope": "crates/search-ports/**",
            "handoffs": ["search-contracts::accepted_package_and_api_handoff"],
            "sources": port_sources,
            "fragments": [
                "swarm/crates.toml::package[name=search-ports]",
                "swarm/function-packets.toml::foundation[package=search-ports]",
                "swarm/stages.toml::stage[id=W0]",
                "swarm/launch-state.toml::conditional_packages[search-ports]",
                "swarm/launch-state.toml::conditional_activation.search-ports",
            ],
            "ceiling": ordinary_ceiling,
        },
    }

    for package, spec in expected.items():
        ticket_path = f"swarm/ticket-drafts/p00/{package}.toml"
        context_path = f"swarm/context-drafts/p00/{package}.toml"
        try:
            ticket = load(ticket_path)
            context = load(context_path)
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            errors.append(f"{package}: {exc}")
            continue

        fail(errors, ticket.get("schema_version") == 2, f"{package}: ticket schema must be v2")
        fail(errors, ticket.get("record_kind") == "assignment_ticket_draft", f"{package}: ticket kind mismatch")
        fail(errors, ticket.get("status") == "DRAFT_ONLY_NOT_ISSUED", f"{package}: ticket status changed")
        for key in ("claimable", "authorizes_implementation", "creates_lease", "may_be_writer_acknowledged"):
            fail(errors, ticket.get(key) is False, f"{package}: ticket authority flag {key}")
        fail(errors, ticket.get("stage") == "W0" and ticket.get("phase") == "P00" and ticket.get("wave") == 0, f"{package}: ticket stage mismatch")
        fail(errors, ticket.get("launch_class") == spec["launch"], f"{package}: launch class mismatch")
        fail(errors, ticket.get("launch_precondition") == spec["precondition"], f"{package}: launch precondition mismatch")
        fail(errors, ticket.get("repository_fence", {}).get("write_scope") == spec["scope"], f"{package}: ticket write scope mismatch")
        unresolved = ticket.get("unresolved_identity", {})
        fail(errors, unresolved.get("ticket_id") == "UNASSIGNED", f"{package}: ticket ID prematurely assigned")
        fail(errors, unresolved.get("writer") == "UNASSIGNED" and unresolved.get("reviewer") == "UNASSIGNED", f"{package}: actors prematurely assigned")
        fail(errors, unresolved.get("base_commit") == "UNSELECTED", f"{package}: base commit prematurely selected")
        fail(errors, unresolved.get("ticket_signed_payload_sha256") == "UNAVAILABLE", f"{package}: signed payload digest prematurely selected")
        fail(errors, unresolved.get("ticket_exact_record_file_sha256") == "UNAVAILABLE", f"{package}: exact file digest prematurely selected")
        fail(errors, ticket.get("context", {}).get("context_draft") == context_path, f"{package}: context draft link mismatch")
        fail(errors, ticket.get("dependencies", {}).get("accepted_handoff_refs") == [], f"{package}: accepted handoff refs must remain empty")

        fail(errors, context.get("schema_version") == 2, f"{package}: context schema must be v2")
        fail(errors, context.get("record_kind") == "writer_context_draft", f"{package}: context kind mismatch")
        fail(errors, context.get("status") == "UNMATERIALIZED_DRAFT", f"{package}: context status changed")
        fail(errors, context.get("claimable") is False and context.get("authorizes_implementation") is False, f"{package}: context creates authority")
        fail(errors, context.get("base_commit") == "UNSELECTED", f"{package}: context base prematurely selected")
        fail(errors, context.get("writer_visible_artifact_count") == 1, f"{package}: context must be one artifact")
        sources = context.get("content", {}).get("source_files", [])
        fragments = context.get("content", {}).get("registry_fragments", [])
        handoffs = context.get("content", {}).get("accepted_handoff_slots", [])
        fail(errors, sources == spec["sources"], f"{package}: source list differs from exact bounded context")
        fail(errors, fragments == spec["fragments"], f"{package}: registry fragments differ")
        fail(errors, handoffs == spec["handoffs"], f"{package}: handoff slots differ")
        fail(errors, context.get("source_file_count") == len(sources), f"{package}: source_file_count mismatch")
        fail(errors, len(sources) <= spec["ceiling"], f"{package}: source ceiling exceeded")
        fail(errors, context.get("registry_fragment_count") == len(fragments) <= fragment_ceiling, f"{package}: fragment count/ceiling mismatch")
        fail(errors, context.get("accepted_handoff_slot_count") == len(handoffs) <= handoff_ceiling, f"{package}: handoff count/ceiling mismatch")
        fail(errors, len(sources) == len(set(sources)), f"{package}: duplicate context source")
        for source in sources:
            fail(errors, (ROOT / source).is_file(), f"{package}: missing context source {source}")
            fail(errors, not source.startswith("docs/architecture/"), f"{package}: architecture master in context")
            if "/src/" in source:
                fail(errors, source.startswith(f"crates/{package}/src/"), f"{package}: dependency implementation source in context")

    fail(errors, launch.get("active_stage") == "P00" and launch.get("active_wave") == 0, "launch state must remain P00/W0")
    fail(errors, launch.get("authorized_packages") == ["search-contracts"], "only search-contracts may be authorized")
    fail(errors, set(launch.get("conditional_packages", [])) == {"search-domain", "search-ports"}, "conditional package set mismatch")
    fail(errors, orchestration.get("workflow_policy") == "manual_only", "orchestration workflow policy must remain manual_only")

    for path in CONTROL_ROOTS:
        files = machine_files(path)
        if files:
            errors.append(f"premature control records under {path}: {files}")

    completion_text = (ROOT / "docs/contracts/p00/TYPE_COMPLETIONS.md").read_text(encoding="utf-8")
    for token in ("RecipeIdV1", "RecipeBodyV1", "ComparisonAxis", "ProtocolRange", "PackageOpaque"):
        fail(errors, token in completion_text, f"named type completion missing {token}")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "ticket_drafts": len(ticket_rows),
        "context_drafts": len(context_rows),
        "p00_required_files": len(required_names),
        "search_contracts_sources": len(contract_sources),
        "search_domain_sources": len(domain_sources),
        "search_ports_sources": len(port_sources),
        "active_stage": launch.get("active_stage"),
        "active_wave": launch.get("active_wave"),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
