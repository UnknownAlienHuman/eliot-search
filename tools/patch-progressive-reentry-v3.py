#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GRAPH = ROOT / "tools/coverage_graph_v2.py"
GRAPH_VALIDATOR = ROOT / "tools/validate-coverage-graph-v2.py"
MAPS = ROOT / "tools/package_maps_v2.py"
MAP_VALIDATOR = ROOT / "tools/validate-package-maps-v2.py"


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


def patch_graph() -> None:
    old = '''            relationship = "shared_contract" if producer == "search-contracts" else "domain_rules" if producer == "search-domain" else "vendor_neutral_port" if producer == "search-ports" else "accepted_public_handoff"
            dependency_rows.append(
                {
                    "id": f"{consumer}->{producer}",
                    "consumer": consumer,
                    "consumer_module": target,
                    "producer": producer,
                    "producer_module": public[producer],
                    "relationship": relationship,
                    "contract_source": source_map[consumer][0],
                    "cargo_manifest": package_rows[consumer]["path"] + "/Cargo.toml",
                    "route_kind": route_kind,
                    "exact_accepted_handoff_required": producer not in {"search-contracts", "search-domain", "search-ports", "search-config"},
                }
            )
'''
    new = '''            relationship = "shared_contract" if producer == "search-contracts" else "domain_rules" if producer == "search-domain" else "vendor_neutral_port" if producer == "search-ports" else "accepted_public_handoff"
            consumer_wave = int(package_rows[consumer].get("wave", 0))
            producer_wave = int(package_rows[producer].get("wave", 0))
            requires_stage_reentry = producer_wave > consumer_wave
            reentry_stage = f"W{producer_wave}" if requires_stage_reentry else "NONE"
            if requires_stage_reentry:
                relationship = "progressive_reentry_handoff"
            dependency_rows.append(
                {
                    "id": f"{consumer}->{producer}",
                    "consumer": consumer,
                    "consumer_module": target,
                    "consumer_earliest_wave": consumer_wave,
                    "producer": producer,
                    "producer_module": public[producer],
                    "producer_earliest_wave": producer_wave,
                    "relationship": relationship,
                    "contract_source": source_map[consumer][0],
                    "cargo_manifest": package_rows[consumer]["path"] + "/Cargo.toml",
                    "route_kind": route_kind,
                    "requires_stage_reentry": requires_stage_reentry,
                    "reentry_stage": reentry_stage,
                    "exact_accepted_handoff_required": producer not in {"search-contracts", "search-domain", "search-ports", "search-config"},
                }
            )
'''
    replace_once(GRAPH, old, new, "dependency reentry derivation")

    old = '''        "producer_public_entry_only = true",
        "consumer_module_must_exist = true",
        "implementation_authorized_by_this_registry = false",
'''
    new = '''        "producer_public_entry_only = true",
        "consumer_module_must_exist = true",
        "later_wave_dependency_requires_exact_stage_reentry = true",
        "implementation_authorized_by_this_registry = false",
'''
    replace_once(GRAPH, old, new, "dependency registry invariant")

    old = '''                f"consumer_module = {q(row['consumer_module'])}",
                f"producer = {q(row['producer'])}",
                f"producer_module = {q(row['producer_module'])}",
                f"relationship = {q(row['relationship'])}",
'''
    new = '''                f"consumer_module = {q(row['consumer_module'])}",
                f"consumer_earliest_wave = {row['consumer_earliest_wave']}",
                f"producer = {q(row['producer'])}",
                f"producer_module = {q(row['producer_module'])}",
                f"producer_earliest_wave = {row['producer_earliest_wave']}",
                f"relationship = {q(row['relationship'])}",
'''
    replace_once(GRAPH, old, new, "dependency registry wave fields")

    old = '''                f"route_kind = {q(row['route_kind'])}",
                f"exact_accepted_handoff_required = {'true' if row['exact_accepted_handoff_required'] else 'false'}",
'''
    new = '''                f"route_kind = {q(row['route_kind'])}",
                f"requires_stage_reentry = {'true' if row['requires_stage_reentry'] else 'false'}",
                f"reentry_stage = {q(row['reentry_stage'])}",
                f"exact_accepted_handoff_required = {'true' if row['exact_accepted_handoff_required'] else 'false'}",
'''
    replace_once(GRAPH, old, new, "dependency registry reentry fields")


def patch_graph_validator() -> None:
    old = '''        "consumer", "consumer_module", "producer", "producer_module", "relationship", "contract_source",
        "cargo_manifest", "route_kind", "exact_accepted_handoff_required",
'''
    new = '''        "consumer", "consumer_module", "consumer_earliest_wave", "producer", "producer_module",
        "producer_earliest_wave", "relationship", "contract_source", "cargo_manifest", "route_kind",
        "requires_stage_reentry", "reentry_stage", "exact_accepted_handoff_required",
'''
    replace_once(GRAPH_VALIDATOR, old, new, "dependency validator fields")

    old = '''    for identity, row in actual_dependencies.items():
        consumer_ref = f"{row.get('consumer')}:{row.get('consumer_module')}"
        producer_ref = f"{row.get('producer')}:{row.get('producer_module')}"
'''
    new = '''    stage_readsets = load("swarm/stage-readsets.toml", errors)
    reentry_overrides = {
        row.get("id"): row
        for row in stage_readsets.get("override", [])
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    for identity, row in actual_dependencies.items():
        consumer_ref = f"{row.get('consumer')}:{row.get('consumer_module')}"
        producer_ref = f"{row.get('producer')}:{row.get('producer_module')}"
'''
    replace_once(GRAPH_VALIDATOR, old, new, "reentry override loading")

    old = '''        fail(errors, row.get("producer_module") == public_entries.get(row.get("producer")), f"{identity}: dependency must enter producer public boundary")
        fail(errors, (ROOT / str(row.get("contract_source"))).is_file(), f"{identity}: missing contract source")
        fail(errors, (ROOT / str(row.get("cargo_manifest"))).is_file(), f"{identity}: missing Cargo manifest")
'''
    new = '''        fail(errors, row.get("producer_module") == public_entries.get(row.get("producer")), f"{identity}: dependency must enter producer public boundary")
        requires_reentry = row.get("requires_stage_reentry") is True
        if requires_reentry:
            expected_stage = f"W{row.get('producer_earliest_wave')}"
            expected_override = f"{expected_stage}.{row.get('consumer')}"
            override = reentry_overrides.get(expected_override, {})
            fail(errors, row.get("reentry_stage") == expected_stage, f"{identity}: reentry stage mismatch")
            fail(errors, row.get("relationship") == "progressive_reentry_handoff", f"{identity}: later-wave edge must be progressive reentry")
            fail(errors, override.get("package") == row.get("consumer"), f"{identity}: exact reentry override missing")
            fail(errors, override.get("wave") == row.get("producer_earliest_wave"), f"{identity}: reentry override wave mismatch")
            fail(errors, override.get("replace_previous_stage_context") is True, f"{identity}: reentry must replace prior context")
            fail(errors, override.get("accepted_prior_stage_handoff_only") is True, f"{identity}: reentry must use accepted prior handoff")
            fail(errors, override.get("dependency_implementation_reads_allowed") is False, f"{identity}: reentry may not read dependency implementation")
        else:
            fail(errors, row.get("reentry_stage") == "NONE", f"{identity}: same/earlier-wave edge has spurious reentry")
            fail(errors, int(row.get("producer_earliest_wave", -1)) <= int(row.get("consumer_earliest_wave", -1)), f"{identity}: unmodelled later-wave dependency")
        fail(errors, (ROOT / str(row.get("contract_source"))).is_file(), f"{identity}: missing contract source")
        fail(errors, (ROOT / str(row.get("cargo_manifest"))).is_file(), f"{identity}: missing Cargo manifest")
'''
    replace_once(GRAPH_VALIDATOR, old, new, "dependency reentry validation")


def patch_maps() -> None:
    old = '''                    f"consumer_module = {q(row['consumer_module'])}",
                    f"producer = {q(row['producer'])}",
                    f"producer_module = {q(row['producer_module'])}",
                    f"relationship = {q(row['relationship'])}",
                    f"contract_source = {q(row['contract_source'])}",
                    f"exact_accepted_handoff_required = {bool_text(bool(row['exact_accepted_handoff_required']))}",
'''
    new = '''                    f"consumer_module = {q(row['consumer_module'])}",
                    f"consumer_earliest_wave = {row['consumer_earliest_wave']}",
                    f"producer = {q(row['producer'])}",
                    f"producer_module = {q(row['producer_module'])}",
                    f"producer_earliest_wave = {row['producer_earliest_wave']}",
                    f"relationship = {q(row['relationship'])}",
                    f"contract_source = {q(row['contract_source'])}",
                    f"requires_stage_reentry = {bool_text(bool(row['requires_stage_reentry']))}",
                    f"reentry_stage = {q(row['reentry_stage'])}",
                    f"exact_accepted_handoff_required = {bool_text(bool(row['exact_accepted_handoff_required']))}",
'''
    replace_once(MAPS, old, new, "package relation reentry fields")


def patch_map_validator() -> None:
    old = '''        consumer_wave = int(row.get("wave", 0))
        for producer in declared:
            producer_wave = int(graph["package_rows"][producer].get("wave", 0))
            fail(errors, producer_wave <= consumer_wave, f"{package}: depends on later-wave package {producer} ({producer_wave}>{consumer_wave})")
    cycle = dependency_cycle(graph["package_rows"])
'''
    new = '''        consumer_wave = int(row.get("wave", 0))
        stage_readsets = read_toml("swarm/stage-readsets.toml", errors)
        reentry_overrides = {
            item.get("id"): item
            for item in stage_readsets.get("override", [])
            if isinstance(item, dict) and isinstance(item.get("id"), str)
        }
        for producer in declared:
            producer_wave = int(graph["package_rows"][producer].get("wave", 0))
            if producer_wave > consumer_wave:
                override_id = f"W{producer_wave}.{package}"
                override = reentry_overrides.get(override_id, {})
                fail(errors, override.get("package") == package, f"{package}: later-wave dependency {producer} lacks exact {override_id} reentry")
                fail(errors, override.get("wave") == producer_wave, f"{package}: {override_id} wave mismatch")
                fail(errors, override.get("replace_previous_stage_context") is True, f"{package}: {override_id} must replace prior context")
                fail(errors, override.get("accepted_prior_stage_handoff_only") is True, f"{package}: {override_id} must consume accepted handoff only")
                fail(errors, override.get("dependency_implementation_reads_allowed") is False, f"{package}: {override_id} may not read dependency implementation")
    cycle = dependency_cycle(graph["package_rows"])
'''
    replace_once(MAP_VALIDATOR, old, new, "package dependency reentry check")


def main() -> int:
    patch_graph()
    patch_graph_validator()
    patch_maps()
    patch_map_validator()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
