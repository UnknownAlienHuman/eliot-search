#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / "tools/coverage_graph_v2.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def main() -> int:
    text = PATH.read_text(encoding="utf-8")
    text = replace_once(
        text,
        '''                candidate = re.match(r"`([a-z][a-z0-9_]*)\\b", heading["raw"])
                if candidate and candidate.group(1) == name:
                    continue
                if heading["level"] <= 3:
                    context = heading["title"]
''',
        '''                candidate = re.match(r"`([a-z][a-z0-9_]*)\\b", heading["raw"])
                if candidate:
                    continue
                if heading["level"] <= 3:
                    context = heading["title"]
''',
        "operation heading context",
    )
    text = replace_once(
        text,
        '''    if best_score >= 20:
        return best, "semantic", best_score
    if best_score >= 5:
        return best, "semantic_low", best_score
    return public_entry, "public_facade", 0
''',
        '''    if best_score >= 5:
        return best, "semantic", best_score
    if best_score > 0:
        return best, "semantic_low", best_score
    return public_entry, "public_facade", 0
''',
        "semantic thresholds",
    )
    rules = '''
PACKAGE_RULES.update({
    "eliot-search-doc-worker": [
        (r"^emit_terminal$", "protocol"),
        (r"^(cancel_request|cleanup_request_workspace)$", "lifecycle"),
        (r"^verify_inherited_sandbox$", "sandbox"),
    ],
    "eliot-search-model-worker": [
        (r"^emit_terminal$", "protocol"),
        (r"^verify_inherited_containment$", "sandbox"),
        (r"^(begin_drain|shutdown_and_remove|cancel_request)$", "lifecycle"),
    ],
    "search-access": [
        (r"^(classify_active_request_contamination|recheck_live_access)$", "invalidation"),
        (r"^(begin_security_mutation|classify_security_change|install_live_restriction|commit_and_publish_restriction)$", "barrier"),
        (r"^compile_base_eligibility$", "snapshot"),
    ],
    "search-candidate-validator": [
        (r"^(material_coverage_change|precheck|validate|validate_lifecycle_fence)$", "decision"),
        (r"^build_readback_request$", "request"),
    ],
    "search-code-enricher": [
        (r"^build_enrichment_manifest$", "facts"),
        (r"^classify_evidence_role$", "assurance"),
        (r"^validate_fact_anchor$", "anchors"),
        (r"^extract_configuration_predicate$", "predicates"),
        (r"^extract_structural_relations$", "references"),
    ],
    "search-continuation": [
        (r"^apply_live_limits$", "config"),
        (r"^resolve$", "resolve"),
        (r"^(create_durable_replan_checkpoint|create_ephemeral)$", "issue"),
        (r"^(expand_durable|expand_ephemeral)$", "resume"),
        (r"^revalidate$", "reauthorize"),
        (r"^(invalidate|invalidate_lifecycle_scope)$", "cleanup"),
    ],
    "search-epoch-pins": [
        (r"^evaluate_old_route_reclaimability$", "watermark"),
        (r"^fence_old_route_for_new_pins$", "acquire"),
        (r"^invalidate_scale_owner_epoch$", "identity"),
        (r"^snapshot_route_drain$", "release"),
    ],
    "search-eval": [
        (r"^(decide_acceptance|compare_optional_candidate|validate_acceptance_policy)$", "adjudication"),
        (r"^(evaluate_candidate_slos|score_case|score_incremental_cost)$", "metrics"),
        (r"^(audit_optional_noninterference|validate_fault_matrix|validate_optional_fault_matrix|validate_protocol_stress)$", "safety"),
        (r"^validate_removal_and_p15_regression$", "review"),
        (r"^aggregate_product_pulse$", "campaign"),
        (r"^(ingest_external_probe|plan_case_block)$", "evidence"),
    ],
    "search-exact": [
        (r"^classify_completeness$", "report"),
        (r"^compile_exact_scan$", "plan"),
        (r"^resume_execution$", "recovery"),
    ],
    "search-handles": [
        (r"^apply_live_limits$", "config"),
        (r"^(mint_durable_source|mint_ephemeral)$", "issue"),
        (r"^token_digest$", "identity"),
    ],
    "search-index-reclaimer": [
        (r"^complete$", "delete"),
        (r"^(complete_old_route_reclaim|execute_old_route_batch|resume_old_route_reclaim)$", "delete"),
        (r"^(validate_retired_manifest|validate_retired_route_manifest)$", "candidate"),
    ],
    "search-lexical": [
        (r"^normalize_input$", "tokenize"),
    ],
    "search-model-provider": [
        (r"^instrumentation_summary$", "capability"),
        (r"^(prepare_removal|validate_removal_receipt)$", "lifecycle"),
    ],
    "search-overlay": [
        (r"^merge_overlay_and_base$", "direct"),
        (r"^retrieve_overlay$", "direct"),
    ],
    "search-point-identity": [
        (r"^encode_canonical_key$", "canonical"),
        (r"^(derive_point_identity|derive_qdrant_uuid|full_digest|compare_existing_identity)$", "identity"),
    ],
    "search-projection-planner": [
        (r"^diff_manifests$", "diff"),
    ],
    "search-provider-protocol": [
        (r"^(disconnect|close_connection)$", "cleanup"),
        (r"^project_capability_descriptor$", "negotiation"),
        (r"^route_expand_handle$", "request"),
    ],
    "search-publication": [
        (r"^(apply_ordered_catch_up|record_base_at_r0)$", "stage"),
        (r"^(enter_final_barrier|validate_candidate_at_r1)$", "commit"),
        (r"^emit_old_route_manifest$", "reclaim"),
    ],
    "search-qdrant-bridge": [
        (r"^connect$", "capability"),
        (r"^validate_scale_profile$", "schema"),
    ],
    "search-result-projector": [
        (r"^bind_result$", "request"),
        (r"^(project_candidate_set|select_handle_subjects)$", "card"),
        (r"^(project_recipe_result|project_coverage)$", "coverage"),
        (r"^enforce_result_budget$", "truncation"),
    ],
    "search-retention": [
        (r"^(build_lifecycle_invalidation_set|verify_invalidation_completion)$", "purge"),
    ],
    "search-retrieval-executor": [
        (r"^cancel$", "cleanup"),
        (r"^admit$", "request"),
    ],
    "search-revision-store": [
        (r"^(apply_exact_object_deletion|install_purge_tombstone)$", "deletion"),
    ],
    "search-runtime-owner": [
        (r"^inspect_existing_owner$", "observation"),
    ],
    "search-source-reconcile": [
        (r"^(ingest_watch_hint|observe_cursor_transition)$", "gap"),
        (r"^handle_live_head_mismatch$", "currentness"),
    ],
    "search-subject-resolver": [
        (r"^assemble_resolution$", "ladder"),
        (r"^revalidate_resolution$", "drift"),
    ],
    "search-unitizer": [
        (r"^derive_unit_id$", "identity"),
        (r"^diff_unit_manifests$", "diff"),
        (r"^unitize$", "api"),
    ],
})
'''
    text = replace_once(text, "}\n\nCONFIG_MODULES = {", "}\n" + rules + "\nCONFIG_MODULES = {", "package rule extension")
    text = replace_once(
        text,
        '''            role = "public_entry" if module == public[package] else "error_boundary" if module == "error" else "implementation"
            specific = sum(value for key, value in counts.items() if key not in {"documentation_nodes"})
            weak = role == "implementation" and specific == 0
''',
        '''            if module == public[package]:
                role = "public_entry"
            elif module in STRUCTURAL_MODULES:
                role = "structural_boundary"
            else:
                role = "implementation"
            specific = sum(value for key, value in counts.items() if key not in {"documentation_nodes"})
            weak = role == "implementation" and specific == 0
''',
        "module roles",
    )
    PATH.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
