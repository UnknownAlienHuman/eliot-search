#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GRAPH = ROOT / "tools/coverage_graph_v2.py"
VALIDATOR = ROOT / "tools/validate-coverage-graph-v2.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def patch_graph() -> None:
    text = GRAPH.read_text(encoding="utf-8")

    reviewed_routes = r'''
for _package, _rules in {
    "eliot-search": [
        (r"^resolve_local_endpoint$", "connection"),
    ],
    "eliot-searchd": [
        (r"^build_candidate_generation$", "optional"),
        (r"^commit_baseline_restore$", "optional"),
        (r"^mint_standalone_grant$", "capability"),
        (r"^drain_generic_edge$", "shutdown"),
    ],
    "search-candidate-validator": [
        (r"^recheck_before_emission$", "security"),
    ],
    "search-comparator": [
        (r"^order_recommended_reading$", "result"),
        (r"^revalidate_comparison$", "lineage"),
    ],
    "search-continuation": [
        (r"^resolve$", "resume"),
    ],
    "search-eliot-adapter": [
        (r"^(map_capability_pulse|map_search_revocation_event)$", "result"),
    ],
    "search-epoch-pins": [
        (r"^renew_continuation_pin$", "acquire"),
        (r"^snapshot$", "registry"),
    ],
    "search-eval": [
        (r"^compare_abc$", "metrics"),
        (r"^(freeze_candidate_comparison|ingest_candidate_operation_receipt)$", "evidence"),
    ],
    "search-handles": [
        (r"^revalidate$", "resolve"),
    ],
    "search-index-reclaimer": [
        (r"^checkpoint$", "recovery"),
    ],
    "search-model-provider": [
        (r"^prepare_model_input$", "validation"),
    ],
    "search-projection-planner": [
        (r"^plan_projection$", "point_set"),
    ],
    "search-provider-protocol": [
        (r"^cancel_request$", "cancel"),
    ],
    "search-query-planner": [
        (r"^(capture_query_snapshot|fingerprint_snapshot)$", "request"),
    ],
    "search-result-projector": [
        (r"^project_candidate_set$", "ordering"),
    ],
    "search-source-reconcile": [
        (r"^verify_change_set$", "inventory"),
    ],
    "search-subject-resolver": [
        (r"^issue_resolution_receipt$", "redaction"),
    ],
    "search-materializer": [
        (r"^prepare_admission$", "request"),
    ],
}.items():
    PACKAGE_RULES.setdefault(_package, []).extend(_rules)

STRUCTURAL_MODULE_ROLES: dict[str, str] = {
    "eliot-search:doctor": "bounded diagnostic command and rendering boundary",
    "eliot-search-doc-worker:args": "private worker argument bootstrap boundary",
    "eliot-search-model-worker:args": "private worker argument bootstrap boundary",
    "search-candidate-validator:batch": "bounded validation batch coordination without new semantics",
    "search-code-enricher:batch": "bounded enrichment batch coordination without new semantics",
    "search-eliot-adapter:binding": "ELIOT binding translation support under adapter authority",
    "search-point-identity:batch": "bounded pure identity batch coordination",
    "search-ports:stream": "vendor-neutral bounded page and stream support types",
    "search-projection-planner:profile": "projection profile identity and compatibility support",
    "search-result-projector:ordering": "deterministic result ordering policy",
}
'''
    text = replace_once(
        text,
        "\nCONFIG_MODULES = {",
        "\n" + reviewed_routes + "\nCONFIG_MODULES = {",
        "reviewed routes and structural roles",
    )

    old_role = '''            if module == public[package]:
                role = "public_entry"
            elif module in STRUCTURAL_MODULES:
                role = "structural_boundary"
            else:
                role = "implementation"
            specific = sum(value for key, value in counts.items() if key not in {"documentation_nodes"})
            weak = role == "implementation" and specific == 0
            if weak:
                weak_modules.append(ref)
'''
    new_role = '''            structural_rationale = STRUCTURAL_MODULE_ROLES.get(ref, "")
            if module == public[package]:
                role = "public_entry"
                structural_rationale = structural_rationale or "single package public entry boundary"
            elif module in STRUCTURAL_MODULES:
                role = "structural_boundary"
                structural_rationale = structural_rationale or "package structural boundary"
            elif structural_rationale:
                role = "structural_support"
            else:
                role = "implementation"
            specific = sum(value for key, value in counts.items() if key not in {"documentation_nodes"})
            weak = role == "implementation" and specific == 0
            if weak:
                weak_modules.append(ref)
'''
    text = replace_once(text, old_role, new_role, "module role classification")

    old_row = '''                    "role": role,
                    "operation_count": counts.get("operations", 0),
'''
    new_row = '''                    "role": role,
                    "structural_rationale": structural_rationale,
                    "operation_count": counts.get("operations", 0),
'''
    text = replace_once(text, old_row, new_row, "module row rationale")

    old_render = '''                f"role = {q(row['role'])}",
                f"operation_count = {row['operation_count']}",
'''
    new_render = '''                f"role = {q(row['role'])}",
                f"structural_rationale = {q(row['structural_rationale'])}",
                f"operation_count = {row['operation_count']}",
'''
    text = replace_once(text, old_render, new_render, "module registry rationale")

    GRAPH.write_text(text, encoding="utf-8", newline="\n")


def patch_validator() -> None:
    text = VALIDATOR.read_text(encoding="utf-8")
    old_fields = '''        "package", "module", "role", "operation_count", "documentation_node_count",
'''
    new_fields = '''        "package", "module", "role", "structural_rationale", "operation_count", "documentation_node_count",
'''
    text = replace_once(text, old_fields, new_fields, "module validator fields")

    old_quality = '''    if public_facades:
        warnings.append(f"{public_facades} operations are explicit public-entry facade operations")
    if semantic_low:
        warnings.append(f"{semantic_low} operations use low-score but exact semantic routes")
'''
    new_quality = '''    fail(errors, public_facades == 0, f"unreviewed public-entry operation routes remain: {public_facades}")
    fail(errors, semantic_low == 0, f"low-confidence operation routes remain: {semantic_low}")
'''
    text = replace_once(text, old_quality, new_quality, "route quality enforcement")

    old_weak = '''    fail(errors, module_doc.get("weak_module_count") == len(graph["weak_modules"]), "module weak count mismatch")
    fail(errors, not graph["weak_modules"], f"implementation modules without specific operation/document/architecture relation: {graph['weak_modules']}")
'''
    new_weak = '''    fail(errors, module_doc.get("weak_module_count") == len(graph["weak_modules"]), "module weak count mismatch")
    for identity, row in actual_modules.items():
        role = row.get("role")
        rationale = row.get("structural_rationale")
        if role in {"public_entry", "structural_boundary", "structural_support"}:
            fail(errors, isinstance(rationale, str) and bool(rationale.strip()), f"{identity}: structural module rationale missing")
    fail(errors, not graph["weak_modules"], f"implementation modules without specific operation/document/architecture relation: {graph['weak_modules']}")
'''
    text = replace_once(text, old_weak, new_weak, "structural module validation")

    VALIDATOR.write_text(text, encoding="utf-8", newline="\n")


def main() -> int:
    patch_graph()
    patch_validator()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
