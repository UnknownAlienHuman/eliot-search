from __future__ import annotations

import hashlib
import json
import re
import subprocess
import tomllib
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
RESERVED = {"if", "for", "while", "match", "loop", "return"}
STRUCTURAL_MODULES = {"lib", "api", "main", "error"}
SUPPLEMENT_NAMES = {
    "W7_HARDENING.md",
    "P18_SCALE.md",
    "W8_INTEGRATION.md",
    "W10_INTEGRATION.md",
    "W8_CLIENT.md",
    "W10_OPTIONAL_EVALUATION.md",
}

MODULE_ALIASES: dict[str, set[str]] = {
    "canonical": {"canonical", "canon", "encode", "encoding", "normalize", "serialization", "codec"},
    "bounds": {"bound", "bounded", "limit", "budget", "quota", "size", "depth"},
    "ids": {"id", "identity", "identifier", "reference", "ref", "digest"},
    "source": {"source", "path", "repository", "workspace", "root"},
    "query": {"query", "request", "search"},
    "recipes": {"recipe"},
    "results": {"result", "candidate", "output", "coverage"},
    "protocol": {"protocol", "wire", "frame", "session", "ipc"},
    "lifecycle": {"lifecycle", "start", "stop", "shutdown", "drain", "expire", "health"},
    "reasons": {"reason", "failure", "error", "degradation"},
    "schema": {"schema", "type", "registry", "contract", "shape"},
    "ordering": {"order", "ordering", "sort", "deterministic"},
    "coverage": {"coverage", "gap", "partial", "complete", "denominator"},
    "assurance": {"assurance", "confidence", "exactness", "loss"},
    "currentness": {"current", "currentness", "fresh", "freshness", "stale"},
    "eligibility": {"eligible", "eligibility", "filter", "scope"},
    "visibility": {"visibility", "disclosure", "redaction", "public"},
    "transitions": {"transition", "state", "change"},
    "outcomes": {"outcome", "result", "failure", "partial"},
    "decisions": {"decision", "decide", "verdict", "classify"},
    "context": {"context", "operation", "deadline", "cancel", "budget"},
    "stream": {"stream", "page", "progress"},
    "runtime": {"runtime", "owner", "process"},
    "control": {"control", "journal", "transaction", "snapshot"},
    "preparation": {"prepare", "materialize", "unitize", "representation"},
    "index": {"index", "qdrant", "point", "projection", "publication", "epoch"},
    "access": {"access", "grant", "security", "authorize"},
    "handles": {"handle", "continuation", "token", "expand"},
    "optional": {"optional", "model", "document", "provider", "scale"},
    "conformance": {"conformance", "contract", "fake", "test", "qualification"},
    "document": {"document", "parse", "toml", "input"},
    "registry": {"registry", "register", "section", "descriptor"},
    "layering": {"layer", "precedence", "merge", "override"},
    "snapshot": {"snapshot", "view", "publish"},
    "diff": {"diff", "change", "reconfiguration"},
    "redaction": {"redact", "redaction", "disclosure", "diagnostic"},
    "config": {"config", "configuration", "section", "setting", "default", "reload"},
    "identity": {"identity", "id", "key", "digest", "fingerprint", "binding"},
    "observation": {"observe", "observation", "inspect", "probe"},
    "acquire": {"acquire", "acquisition", "claim", "lock", "owner"},
    "recovery": {"recover", "recovery", "resume", "retry", "unknown", "rebuild"},
    "release": {"release", "drain", "permit", "unlock"},
    "health": {"health", "diagnostic", "doctor", "readiness"},
    "binding": {"binding", "bind", "incarnation", "purpose"},
    "store": {"store", "secret", "persist", "delete", "create"},
    "lease": {"lease", "borrow", "guard"},
    "rotation": {"rotate", "rotation"},
    "audit": {"audit", "leak", "surface", "diagnostic"},
    "open": {"open", "create", "inspect"},
    "migration": {"migration", "migrate", "upgrade", "route", "candidate"},
    "transaction": {"transaction", "transact", "commit", "mutation", "read"},
    "idempotency": {"idempotent", "idempotency", "operation", "prune"},
    "quarantine": {"quarantine", "corrupt", "deny"},
    "frame": {"frame", "encode", "decode", "wire"},
    "negotiation": {"negotiate", "hello", "version", "capability"},
    "pairing": {"pair", "pairing", "challenge", "proof"},
    "session": {"session", "connection", "disconnect"},
    "request": {"request", "admit", "dispatch", "validate"},
    "progress": {"progress", "stream"},
    "terminal": {"terminal", "complete", "emit", "result"},
    "cancel": {"cancel", "disconnect"},
    "cleanup": {"cleanup", "release", "expire", "invalidate", "remove"},
    "profile": {"profile", "descriptor", "qualification", "mode"},
    "adapters": {"adapter", "bind", "port", "dependency"},
    "startup": {"startup", "start", "initialize", "acquire", "endpoint"},
    "readiness": {"readiness", "health", "ready", "dependency"},
    "capability": {"capability", "available", "publish"},
    "connection": {"connection", "connect", "authenticate", "session", "pair"},
    "shutdown": {"shutdown", "drain", "release", "stop"},
    "evaluation": {"evaluation", "evaluate", "pulse", "acceptance", "metric"},
    "args": {"arg", "args", "command", "cli", "parse"},
    "render": {"render", "display", "json", "human"},
    "doctor": {"doctor", "diagnostic", "health"},
    "exit": {"exit", "status", "code"},
    "policy": {"policy", "rule", "normalize", "fingerprint"},
    "decision": {"decision", "decide", "evaluate", "receipt", "classify"},
    "receipt": {"receipt", "issue", "verify"},
    "batch": {"batch", "many", "bulk"},
    "root": {"root", "admitted", "registration"},
    "membership": {"membership", "member", "bind"},
    "portfolio": {"portfolio", "corpus"},
    "view": {"view", "snapshot", "resolve"},
    "cutover": {"cutover", "owner", "fence", "activate"},
    "stable_read": {"stable", "read", "before", "after", "containment"},
    "git_object": {"git", "object", "blob", "tree"},
    "encoding": {"encoding", "unicode", "utf", "newline"},
    "residency": {"residency", "resident", "domain"},
    "address": {"address", "key", "object"},
    "object": {"object", "cas", "blob", "store"},
    "revision": {"revision", "version", "content", "retained"},
    "anchor": {"anchor", "coordinate", "span", "offset"},
    "deletion": {"delete", "deletion", "reclaim"},
    "restore": {"restore", "backup", "quarantine"},
    "decode": {"decode", "encoding", "text", "bytes"},
    "normalize": {"normalize", "canonical", "newline", "unicode"},
    "maps": {"map", "coordinate", "loss", "span"},
    "product": {"product", "materialization", "receipt", "output"},
    "provider": {"provider", "external", "optional", "artifact"},
    "boundaries": {"boundary", "scan", "split", "range"},
    "occurrence": {"occurrence", "anchor", "sequence"},
    "manifest": {"manifest", "canonical", "digest", "verify"},
    "facts": {"fact", "item", "structure"},
    "references": {"reference", "relation", "call"},
    "predicates": {"predicate", "cfg", "condition"},
    "anchors": {"anchor", "coordinate", "span"},
    "tokenize": {"token", "tokenize", "term", "identifier"},
    "sparse": {"sparse", "weight", "vector", "encode"},
    "statistics": {"statistic", "idf", "collision", "count", "measure"},
    "fixture": {"fixture", "corpus", "conformance", "test"},
    "artifact": {"artifact", "binary", "checksum", "license", "qualify"},
    "process": {"process", "start", "restart", "exit", "pid"},
    "admin": {"admin", "collection", "schema", "create"},
    "mutation": {"mutation", "upsert", "delete", "close", "write"},
    "readback": {"readback", "read", "verify", "count"},
    "key": {"key", "canonical", "identity"},
    "digest": {"digest", "hash", "fingerprint"},
    "uuid": {"uuid", "point", "id"},
    "collision": {"collision", "compare", "mismatch"},
    "input": {"input", "request", "validate", "admit"},
    "point_set": {"point", "set", "spec", "payload"},
    "candidate": {"candidate", "retired", "eligible"},
    "plan": {"plan", "compile", "prepare"},
    "delete": {"delete", "batch", "reclaim"},
    "command": {"command", "submit", "request"},
    "actor": {"actor", "worker", "owner"},
    "intent": {"intent", "persist", "operation"},
    "stage": {"stage", "upsert", "prepare"},
    "commit": {"commit", "visible", "publish", "snapshot"},
    "reclaim": {"reclaim", "retired", "close", "manifest"},
    "grant": {"grant", "validate", "authorize"},
    "scope": {"scope", "eligibility", "intersect"},
    "legs": {"leg", "route", "compile", "retrieve"},
    "checkpoint": {"checkpoint", "fence", "revision"},
    "barrier": {"barrier", "restriction", "security", "commit"},
    "invalidation": {"invalidate", "revocation", "contaminate"},
    "scheduler": {"schedule", "scheduler", "admit", "lane", "queue"},
    "pins": {"pin", "epoch", "route", "acquire"},
    "fusion": {"fusion", "fuse", "score", "normalize"},
    "contamination": {"contaminate", "discard", "deny", "completion"},
    "card": {"card", "project", "result"},
    "evidence": {"evidence", "candidate", "source", "bind"},
    "truncation": {"truncate", "budget", "omit"},
    "ladder": {"ladder", "resolve", "step", "priority"},
    "ambiguity": {"ambiguity", "ambiguous", "candidate"},
    "drift": {"drift", "stale", "revalidate"},
    "lineage": {"lineage", "fork", "mirror", "independent"},
    "alignment": {"align", "alignment", "compare"},
    "behavior": {"behavior", "signature", "compare"},
    "conflicts": {"conflict", "variant", "difference"},
    "predicate": {"predicate", "regex", "structural", "compile"},
    "denominator": {"denominator", "inventory", "complete", "scope"},
    "execute": {"execute", "scan", "run", "checkpoint"},
    "report": {"report", "proof", "result", "complete"},
    "token": {"token", "digest", "opaque"},
    "issue": {"issue", "mint", "create"},
    "resolve": {"resolve", "lookup", "record"},
    "expand": {"expand", "read", "authorize"},
    "invalidate": {"invalidate", "revoke", "expire"},
    "resume": {"resume", "expand", "checkpoint"},
    "reauthorize": {"reauthorize", "revalidate", "access"},
    "roots": {"root", "mark", "retention"},
    "mark": {"mark", "manifest", "root"},
    "sweep": {"sweep", "delete", "reclaim"},
    "purge": {"purge", "erase", "delete", "fence"},
    "tombstone": {"tombstone", "deny", "restore"},
    "campaign": {"campaign", "run", "experiment", "pulse"},
    "corpus": {"corpus", "fixture", "case"},
    "oracle": {"oracle", "label", "truth"},
    "runner": {"run", "execute", "driver"},
    "trial": {"trial", "case", "observation"},
    "adjudication": {"adjudicate", "score", "decision"},
    "metrics": {"metric", "latency", "resource", "aggregate", "score"},
    "safety": {"safety", "leak", "hard", "blocker", "audit"},
    "review": {"review", "receipt", "acceptance", "verdict"},
    "model": {"model", "provider", "runtime", "load"},
    "encode": {"encode", "embedding", "vector"},
    "rerank": {"rerank", "rank", "subset"},
    "validation": {"validate", "validation", "output", "request"},
    "sandbox": {"sandbox", "containment", "inherited", "isolation"},
    "materialize": {"materialize", "convert", "render", "output"},
    "resource": {"resource", "pressure", "memory", "cpu", "quota"},
    "content": {"content", "native", "bytes", "reopen"},
    "export": {"export", "bundle", "publish"},
}

PACKAGE_RULES: dict[str, list[tuple[str, str]]] = {
    "eliot-searchd": [
        (r"^(validate_composition_profile|composition_digest|classify_profile_change)$", "profile"),
        (r"^(capture_config_inputs|build_candidate_config|plan_config_activation|execute_config_activation|recover_config_activation)$", "config"),
        (r"^(construct_platform_adapters|bind_ports|verify_dependency_graph)$", "adapters"),
        (r"^(acquire_root|build_wave1_shell|initialize_secrets_and_control|publish_control_snapshot|start_provider_endpoint)$", "startup"),
        (r"^(collect_dependency_health|derive_readiness)$", "readiness"),
        (r"^publish_capability_snapshot$", "capability"),
        (r"^(admit_connection|handle_disconnect)$", "connection"),
        (r"^(admit_request|dispatch_request|cancel_request)$", "request"),
        (r"^(recover_operations|recover_startup)$", "recovery"),
        (r"^(begin_drain|drain_requests|shutdown_services|release_owner)$", "shutdown"),
        (r"^compose_optional_candidate$", "optional"),
        (r"^compose_evaluation_mode$", "evaluation"),
        (r"^compose_", "capability"),
    ],
    "eliot-search": [
        (r"^parse_command$", "args"),
        (r"^(connect_and_authenticate|close_session)$", "connection"),
        (r"^request_standalone_grant$", "pairing"),
        (r"^(build_recipe_request|run_request|fetch_capabilities|expand_handle)$", "request"),
        (r"^render_result$", "render"),
        (r"^cancel_request$", "cancel"),
        (r"^map_exit_status$", "exit"),
    ],
    "search-config": [
        (r"^parse_document$", "document"),
        (r"^(register_sections|project_section)$", "registry"),
        (r"^(merge_layers|validate_environment_key)$", "layering"),
        (r"^(assemble_effective|fingerprint)$", "snapshot"),
        (r"^(diff|plan_reconfiguration)$", "diff"),
        (r"^redacted_view$", "redaction"),
    ],
}

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

CONFIG_MODULES = {
    "instance": "config",
    "secrets": "config",
    "control": "config",
    "protocol": "config",
    "source_admission": "config",
    "source_reader": "config",
    "reconcile": "config",
    "revision_store": "config",
    "lexical": "config",
    "qdrant_process": "config",
    "qdrant_data": "config",
    "index_reclaim": "config",
    "query": "config",
    "scheduler": "config",
    "overlay": "config",
    "handles": "config",
    "continuations": "config",
    "retention": "config",
    "observability": "config",
    "optional_profiles": "optional",
}

DOC_FILE_ROUTES: dict[str, list[str]] = {
    "docs/contracts/p00/README.md": ["search-contracts:schema", "search-domain:decisions", "search-ports:conformance"],
    "docs/contracts/p00/CANONICAL_TYPES.md": ["search-contracts:canonical", "search-contracts:bounds", "search-contracts:ids"],
    "docs/contracts/p00/TYPE_REGISTRY.md": ["search-contracts:schema", "search-contracts:bounds", "search-contracts:ids", "search-domain:decisions", "search-ports:context"],
    "docs/contracts/p00/TYPE_COMPLETIONS.md": ["search-contracts:schema", "search-contracts:recipes", "search-contracts:protocol"],
    "docs/contracts/p00/SUPPORT_SCHEMAS.md": ["search-contracts:schema", "search-domain:coverage", "search-domain:assurance", "search-domain:currentness", "search-domain:decisions"],
    "docs/contracts/p00/CONTRACT_CHALLENGES.md": ["search-contracts:schema", "search-domain:decisions"],
    "docs/contracts/p00/SOURCE_GRAPH.md": ["search-contracts:source", "search-domain:transitions", "search-domain:currentness", "search-domain:visibility"],
    "docs/contracts/p00/RECIPES.md": ["search-contracts:recipes", "search-query-planner:recipe"],
    "docs/contracts/p00/QUERY_AND_RESULTS.md": ["search-contracts:query", "search-contracts:results", "search-domain:coverage", "search-domain:outcomes"],
    "docs/contracts/p00/RECIPE_RESULTS.md": ["search-contracts:results", "search-domain:decisions"],
    "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md": ["search-contracts:protocol", "search-contracts:lifecycle", "search-provider-protocol:frame", "search-handles:token", "search-continuation:token"],
    "docs/contracts/p00/REASON_CODES.md": ["search-contracts:reasons", "search-domain:outcomes"],
    "docs/contracts/p00/PORT_OPERATIONS.md": ["search-ports:runtime", "search-ports:control", "search-ports:source", "search-ports:preparation", "search-ports:index", "search-ports:query", "search-ports:access", "search-ports:handles", "search-ports:optional"],
    "docs/current/W5_CURRENT_WORKSPACE_CONTRACTS_1.0.md": ["search-source-reconcile:gap", "search-source-reconcile:inventory", "search-source-reconcile:commit", "search-source-reconcile:currentness", "search-overlay:snapshot", "search-overlay:shadow", "search-overlay:save", "search-code-enricher:parse", "search-code-enricher:facts", "search-code-enricher:predicates", "search-code-enricher:assurance"],
    "docs/client/W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md": ["search-provider-protocol:frame", "search-provider-protocol:pairing", "search-provider-protocol:binding", "search-provider-protocol:session", "search-provider-protocol:request", "eliot-searchd:connection", "eliot-searchd:capability", "eliot-search:connection", "eliot-search:request", "search-eliot-adapter:request", "search-research-export-adapter:export"],
    "docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md": ["search-eval:campaign", "search-eval:corpus", "search-eval:oracle", "search-eval:runner", "search-eval:metrics", "search-eval:safety", "search-eval:evidence", "search-eval:review", "eliot-searchd:evaluation"],
    "docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md": ["search-model-provider:profile", "search-model-provider:encode", "search-model-provider:rerank", "eliot-search-model-worker:model", "eliot-search-model-worker:encode", "eliot-search-model-worker:rerank", "eliot-search-doc-worker:provider", "eliot-search-doc-worker:materialize", "eliot-searchd:optional", "search-qdrant-bridge:admin", "search-publication:commit", "search-epoch-pins:registry", "search-index-reclaimer:plan"],
    "docs/config/CONFIGURATION_1.0.md": ["search-config:document", "search-config:registry", "search-config:layering", "search-config:snapshot", "search-config:diff", "search-config:redaction"],
    "docs/config/RECONFIGURATION_1.1.md": ["search-config:diff", "search-config:snapshot", "eliot-searchd:config"],
    "docs/config/W5_CURRENT_SETTINGS_1.0.md": ["search-source-reconcile:config", "search-overlay:config", "search-code-enricher:profile"],
    "docs/config/W7_LIFECYCLE_SETTINGS_1.0.md": ["search-retention:config", "search-access:barrier", "search-handles:invalidate", "search-continuation:cleanup"],
    "docs/config/W8_CLIENT_EDGE_SETTINGS_1.0.md": ["search-provider-protocol:config", "eliot-searchd:config", "eliot-search:config"],
    "docs/config/W9_PRODUCT_PULSE_SETTINGS_1.0.md": ["search-eval:config", "eliot-searchd:evaluation"],
    "docs/config/W10_OPTIONAL_DEPTH_SETTINGS_1.0.md": ["eliot-searchd:optional", "search-model-provider:profile", "eliot-search-model-worker:config", "eliot-search-doc-worker:config"],
}

QUALIFICATION_ROUTES: dict[str, list[str]] = {
    "qualification/qdrant/": ["search-qdrant-supervisor:artifact", "search-qdrant-supervisor:process", "search-qdrant-bridge:schema", "search-lexical:fixture", "search-publication:readback", "search-epoch-pins:registry", "search-index-reclaimer:readback"],
    "qualification/query/": ["search-access:legs", "search-query-planner:plan", "search-retrieval-executor:legs", "search-candidate-validator:decision", "search-result-projector:evidence", "search-handles:expand", "search-continuation:resume"],
    "qualification/current/": ["search-source-reconcile:currentness", "search-overlay:shadow", "search-code-enricher:assurance"],
    "qualification/rust-syntax/": ["search-code-enricher:profile", "search-code-enricher:parse", "search-code-enricher:assurance"],
    "qualification/proof/": ["search-subject-resolver:ambiguity", "search-comparator:coverage", "search-exact:denominator", "search-exact:report"],
    "qualification/lifecycle/": ["search-retention:purge", "search-retention:restore", "search-access:invalidation", "search-handles:invalidate", "search-continuation:cleanup", "search-index-reclaimer:recovery"],
    "qualification/client-edge/": ["search-provider-protocol:binding", "search-provider-protocol:request", "eliot-searchd:connection", "eliot-search:request", "search-eliot-adapter:request", "search-research-export-adapter:export"],
    "qualification/product-pulse/": ["search-eval:campaign", "search-eval:corpus", "search-eval:metrics", "search-eval:safety", "search-eval:review"],
    "qualification/optional-depth/": ["search-model-provider:profile", "eliot-search-model-worker:model", "eliot-search-doc-worker:provider", "eliot-searchd:optional", "search-qdrant-bridge:admin", "search-publication:commit"],
}

GOVERNANCE_PREFIXES = (
    "swarm/",
    "docs/handoff/",
    "qualification/ticket-issuance/",
    "qualification/context-",
    "qualification/p00-",
    "qualification/w1-",
    "qualification/w2-",
    "qualification/w3-",
    "qualification/w4-",
    "qualification/architecture-",
    "tools/",
)


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def rows(document: dict[str, Any], key: str, name_key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key, [])
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(name_key), str):
            raise ValueError(f"invalid {key} row")
        identity = row[name_key]
        if identity in result:
            raise ValueError(f"duplicate {key}: {identity}")
        result[identity] = row
    return result


def git_files() -> list[str]:
    output = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return [line.strip() for line in output.splitlines() if line.strip()]


def words(value: str) -> set[str]:
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", value)
    tokens = re.findall(r"[a-z0-9]+", value.lower().replace("_", " ").replace("-", " "))
    result: set[str] = set()
    for token in tokens:
        result.add(token)
        if token.endswith("ies") and len(token) > 4:
            result.add(token[:-3] + "y")
        elif token.endswith("s") and len(token) > 3:
            result.add(token[:-1])
        if token.endswith("ing") and len(token) > 5:
            result.add(token[:-3])
        if token.endswith("ed") and len(token) > 4:
            result.add(token[:-2])
    return result


def heading_rows(text: str) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for line_no, line in enumerate(text.splitlines(), start=1):
        match = re.match(r"^(#{1,4})\s+(.+?)\s*$", line)
        if not match:
            continue
        raw = match.group(2)
        title = re.sub(r"[`*_]", "", raw).strip()
        result.append({"line": line_no, "level": len(match.group(1)), "raw": raw, "title": title})
    return result


def operation_occurrences(text: str) -> dict[str, dict[str, Any]]:
    headings = heading_rows(text)
    heading_positions: list[tuple[int, dict[str, Any]]] = []
    cursor = 0
    for line in text.splitlines(keepends=True):
        match = re.match(r"^(#{1,4})\s+(.+?)\s*$", line.rstrip("\r\n"))
        if match:
            raw = match.group(2)
            heading_positions.append((cursor, {"level": len(match.group(1)), "raw": raw, "title": re.sub(r"[`*_]", "", raw).strip()}))
        cursor += len(line)

    found: dict[str, dict[str, Any]] = {}
    patterns = [
        re.compile(r"^#{2,3}\s+`([a-z][a-z0-9_]*)\b", re.MULTILINE),
        re.compile(r"`([a-z][a-z0-9_]*)\([^`]*\)`"),
    ]
    for pattern in patterns:
        for match in pattern.finditer(text):
            name = match.group(1)
            if name in RESERVED:
                continue
            context = ""
            for position, heading in heading_positions:
                if position > match.start():
                    break
                candidate = re.match(r"`([a-z][a-z0-9_]*)\b", heading["raw"])
                if candidate:
                    continue
                if heading["level"] <= 3:
                    context = heading["title"]
            found.setdefault(name, {"context": context, "position": match.start()})
    for block_match in re.finditer(r"```[^\n]*\n(.*?)```", text, flags=re.DOTALL):
        block = block_match.group(1)
        for match in re.finditer(r"^(?:pub\s+)?(?:async\s+)?(?:fn\s+)?([a-z][a-z0-9_]*)\s*\(", block, flags=re.MULTILINE):
            name = match.group(1)
            if name in RESERVED:
                continue
            absolute = block_match.start(1) + match.start()
            context = ""
            for position, heading in heading_positions:
                if position > absolute:
                    break
                if heading["level"] <= 3:
                    context = heading["title"]
            found.setdefault(name, {"context": context, "position": absolute})
    return found


def package_modules() -> tuple[dict[str, list[str]], dict[str, str], dict[str, dict[str, Any]]]:
    manifest = load("swarm/coverage/manifest.toml")
    package_rows = rows(load(manifest["package_registry"]), "package", "name")
    module_registry = load(manifest["module_registry"])
    modules: dict[str, list[str]] = {}
    entries: dict[str, dict[str, Any]] = {}
    public: dict[str, str] = {}
    for packet in module_registry.get("packet", []):
        document = load(packet["path"])
        for package, row in rows(document, "package", "name").items():
            modules[package] = list(row["modules"])
            public[package] = row["public_entry_module"]
            entries[package] = row
    return modules, public, package_rows


def operation_source_map(package_rows: dict[str, dict[str, Any]]) -> dict[str, list[str]]:
    manifest = load("swarm/coverage/manifest.toml")
    function_doc = load(manifest["function_registry"])
    foundation = rows(function_doc, "foundation", "package")
    normal = rows(function_doc, "package", "name")
    files = git_files()
    result: dict[str, list[str]] = {}
    for package in package_rows:
        sources: list[str] = []
        if package in foundation:
            sources.append(foundation[package]["primary_contract"])
            if package == "search-contracts":
                sources.extend(
                    path for path in files
                    if path.startswith("docs/contracts/p00/") and path.endswith(".md") and Path(path).name != "README.md"
                )
        else:
            sources.append(normal[package]["functions"])
        root = package_rows[package]["path"] + "/"
        for path in files:
            if not path.startswith(root) or not path.endswith(".md"):
                continue
            name = Path(path).name
            if name in SUPPLEMENT_NAMES or re.fullmatch(r"W\d+_[A-Z0-9_]+\.md", name) or re.fullmatch(r"P\d+_[A-Z0-9_]+\.md", name):
                sources.append(path)
        result[package] = sorted(dict.fromkeys(sources))
    return result


def aliases(module: str) -> set[str]:
    result = words(module)
    result.update(MODULE_ALIASES.get(module, set()))
    return result


def choose_module(package: str, operation: str, context: str, modules: list[str], public_entry: str) -> tuple[str, str, int]:
    for pattern, module in PACKAGE_RULES.get(package, []):
        if re.search(pattern, operation) and module in modules:
            return module, "package_rule", 100

    op_words = words(operation)
    context_words = words(context)
    scores: dict[str, int] = {}
    for index, module in enumerate(modules):
        if module in STRUCTURAL_MODULES:
            scores[module] = -20 - index
            continue
        module_words = words(module)
        module_aliases = aliases(module)
        score = 0
        if module in operation:
            score += 45
        if module_words and module_words <= op_words:
            score += 30
        score += 14 * len(module_aliases & op_words)
        score += 6 * len(module_aliases & context_words)
        if module in context.lower().replace(" ", "_"):
            score += 20
        scores[module] = score - index

    special: list[tuple[str, tuple[str, ...]]] = [
        ("config", ("section_", "compiled_defaults", "plan_section_change", "apply_live_change", "config")),
        ("recovery", ("recover", "rebuild", "resume_unknown")),
        ("health", ("health", "doctor")),
        ("readiness", ("readiness", "derive_readiness")),
        ("profile", ("profile", "qualification")),
        ("token", ("token",)),
        ("identity", ("identity", "fingerprint")),
        ("digest", ("digest", "hash")),
        ("cleanup", ("cleanup", "expire", "remove")),
        ("lifecycle", ("begin_drain", "shutdown", "restart", "start", "stop")),
        ("shutdown", ("begin_drain", "shutdown", "release_owner")),
        ("cancel", ("cancel",)),
        ("error", ("error", "failure")),
    ]
    for module, prefixes in special:
        if module not in scores:
            continue
        if any(operation.startswith(prefix) or prefix in operation for prefix in prefixes):
            scores[module] += 28

    best = max(scores, key=scores.get) if scores else public_entry
    best_score = scores.get(best, 0)
    if best_score >= 5:
        return best, "semantic", best_score
    if best_score > 0:
        return best, "semantic_low", best_score
    return public_entry, "public_facade", 0


def module_refs_to_packages(refs: Iterable[str]) -> list[str]:
    return sorted({ref.split(":", 1)[0] for ref in refs})


def slug(value: str) -> str:
    normalized = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return normalized[:96] or "node"


def specific_doc_route(path: str, heading: dict[str, Any], package: str | None, modules: dict[str, list[str]], public: dict[str, str], operation_routes: dict[str, dict[str, Any]]) -> tuple[str, list[str], str, str]:
    title = heading["title"]
    lowered = title.lower()
    principle = bool(re.search(r"\b(principle|principles|global rules|core invariants|non-negotiable|invariants)\b", lowered))

    if package is not None:
        if heading["level"] == 1:
            return "implementation_contract_root", [f"{package}:{name}" for name in modules[package]], "package_contract_root", "all declared package modules"
        op_match = re.match(r"`?([a-z][a-z0-9_]*)\s*\(", heading["raw"])
        if op_match:
            identity = f"{package}::{op_match.group(1)}"
            route = operation_routes.get(identity)
            if route:
                return "implementation_operation", [f"{package}:{route['module']}"], "operation_route", identity
        if "typed failures" in lowered or lowered in {"failures", "error model"}:
            target = "error" if "error" in modules[package] else public[package]
            return "implementation_error_contract", [f"{package}:{target}"], "heading_semantic", "typed failure surface"
        if principle:
            candidates = [name for name in modules[package] if name not in STRUCTURAL_MODULES]
            target, _, _ = choose_module(package, title.replace(" ", "_"), title, candidates or modules[package], public[package])
            refs = [f"{package}:{public[package]}"]
            if target != public[package]:
                refs.append(f"{package}:{target}")
            return "principle_or_invariant", refs, "package_principle", "package-wide rule entering through public boundary"
        target, route_kind, _ = choose_module(package, title.replace(" ", "_"), title, modules[package], public[package])
        return "implementation_contract", [f"{package}:{target}"], route_kind, "package-local documentation"

    if path == "docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md":
        return "architecture_root", ["search-contracts:schema", "eliot-searchd:profile"], "architecture_root", "architecture source of truth"

    if path in DOC_FILE_ROUTES:
        kind = "principle_or_invariant" if principle else "implementation_contract"
        return kind, DOC_FILE_ROUTES[path], "explicit_file_route", "bounded cross-package contract"

    for prefix, refs in QUALIFICATION_ROUTES.items():
        if path.startswith(prefix):
            return "qualification_contract", refs, "qualification_area_route", prefix.rstrip("/")

    if path.startswith("config/sections/"):
        config = rows(load("config/sections.toml"), "section", "name")
        name = Path(path).stem
        row = config.get(name)
        if row:
            return "configuration_contract", [f"{row['owner']}:{row['owner_module']}"], "config_owner", name

    if path.startswith(GOVERNANCE_PREFIXES):
        if re.match(r"docs/handoff/W\d+_IMPLEMENTATION_PACKET\.md", path):
            text = read(path)
            package_names = [name for name in sorted(modules) if re.search(rf"(?<![A-Za-z0-9_-]){re.escape(name)}(?![A-Za-z0-9_-])", text)]
            if package_names and len(package_names) <= 20:
                refs = [f"{name}:{public[name]}" for name in package_names]
                return "implementation_handoff", refs, "mentioned_package_entries", "stage implementation packet"
        return "governance", [], "non_crate_owner", "integration-owner tooling/control plane"

    if Path(path).name == "README.md":
        return "navigation", [], "non_crate_owner", "navigation/explanatory document"

    return "governance", [], "non_crate_owner", "repository governance or explanatory material"


def build_graph() -> dict[str, Any]:
    manifest = load("swarm/coverage/manifest.toml")
    modules, public, package_rows = package_modules()
    package_roots = sorted(((row["path"] + "/", name) for name, row in package_rows.items()), key=lambda item: len(item[0]), reverse=True)
    source_map = operation_source_map(package_rows)

    operation_acc: dict[str, dict[str, Any]] = {}
    for package, source_paths in source_map.items():
        for source in source_paths:
            if not (ROOT / source).is_file():
                continue
            for operation, occurrence in operation_occurrences(read(source)).items():
                identity = f"{package}::{operation}"
                row = operation_acc.setdefault(identity, {"id": identity, "package": package, "operation": operation, "sources": [], "contexts": []})
                row["sources"].append(source)
                if occurrence["context"]:
                    row["contexts"].append(occurrence["context"])

    operation_rows: list[dict[str, Any]] = []
    for identity in sorted(operation_acc):
        row = operation_acc[identity]
        context = row["contexts"][0] if row["contexts"] else ""
        module, route_kind, score = choose_module(row["package"], row["operation"], context, modules[row["package"]], public[row["package"]])
        operation_rows.append(
            {
                "id": identity,
                "package": row["package"],
                "operation": row["operation"],
                "module": module,
                "public_entry_module": public[row["package"]],
                "sources": sorted(set(row["sources"])),
                "source_contexts": sorted(set(row["contexts"])),
                "route_kind": route_kind,
                "score": score,
            }
        )
    operation_routes = {row["id"]: row for row in operation_rows}

    section_rows = rows(load(manifest["architecture_section_registry"]), "section", "id")
    invariant_rows = rows(load(manifest["invariant_registry"]), "invariant", "id")
    delivery_rows = rows(load(manifest["delivery_registry"]), "slice", "id")

    doc_rows: list[dict[str, Any]] = []
    selected_markdown = [path for path in git_files() if path.endswith(".md") and not path.startswith("artifacts/") and not path.startswith("docs/generated/")]
    for path in selected_markdown:
        text = read(path)
        headings = heading_rows(text)
        if not headings:
            headings = [{"line": 1, "level": 0, "raw": "document root", "title": "document root"}]
        package: str | None = None
        for root, candidate in package_roots:
            if path.startswith(root):
                package = candidate
                break
        if package is None and path.startswith("swarm/assignments/"):
            candidate = Path(path).stem
            if candidate in modules:
                package = candidate

        current_arch_refs: list[str] = ["search-contracts:schema", "eliot-searchd:profile"]
        seen_slugs: dict[str, int] = defaultdict(int)
        for heading in headings:
            title = heading["title"]
            if path == "docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md":
                section_match = re.match(r"S(\d+)\.\s+", title)
                invariant_match = re.match(r"(INV-\d{2})[:.\s]", title)
                delivery_match = re.match(r"(P\d{2})\s+[—-]", title)
                if section_match:
                    section_id = f"S{int(section_match.group(1))}"
                    current_arch_refs = list(section_rows.get(section_id, {}).get("modules", current_arch_refs))
                    kind, refs, route_kind, rationale = "architecture_section", current_arch_refs, "architecture_section_registry", section_id
                elif invariant_match and invariant_match.group(1) in invariant_rows:
                    invariant_id = invariant_match.group(1)
                    refs = list(invariant_rows[invariant_id].get("modules", current_arch_refs))
                    kind, route_kind, rationale = "principle_or_invariant", "invariant_registry", invariant_id
                elif delivery_match and delivery_match.group(1) in delivery_rows:
                    delivery_id = delivery_match.group(1)
                    refs = list(delivery_rows[delivery_id].get("modules", current_arch_refs))
                    kind, route_kind, rationale = "delivery_contract", "delivery_registry", delivery_id
                else:
                    refs = current_arch_refs
                    kind = "principle_or_invariant" if re.search(r"\b(principle|invariant|non-negotiable)\b", title, flags=re.IGNORECASE) else "architecture_node"
                    route_kind, rationale = "inherit_architecture_section", "nearest S-section ownership"
            else:
                kind, refs, route_kind, rationale = specific_doc_route(path, heading, package, modules, public, operation_routes)

            node_slug = slug(title)
            seen_slugs[node_slug] += 1
            suffix = f"@{seen_slugs[node_slug]}" if seen_slugs[node_slug] > 1 else ""
            node_id = f"{path}#{node_slug}{suffix}"
            doc_rows.append(
                {
                    "id": node_id,
                    "path": path,
                    "line": heading["line"],
                    "level": heading["level"],
                    "heading": title,
                    "kind": kind,
                    "packages": module_refs_to_packages(refs),
                    "modules": refs,
                    "route_kind": route_kind,
                    "rationale": rationale,
                }
            )

    config_rows = rows(load("config/sections.toml"), "section", "name")
    recipe_rows = rows(load(manifest["recipe_registry"]), "recipe", "id")

    dependency_rows: list[dict[str, Any]] = []
    for consumer, package in sorted(package_rows.items()):
        for producer in package.get("deps", []):
            target, route_kind, _ = choose_module(consumer, producer.replace("search-", ""), producer, modules[consumer], public[consumer])
            if consumer == "eliot-searchd" and "adapters" in modules[consumer]:
                target = "adapters"
                route_kind = "composition_adapter"
            relationship = "shared_contract" if producer == "search-contracts" else "domain_rules" if producer == "search-domain" else "vendor_neutral_port" if producer == "search-ports" else "accepted_public_handoff"
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

    relation_counts: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    for row in operation_rows:
        relation_counts[f"{row['package']}:{row['module']}"]["operations"] += 1
    for row in doc_rows:
        for ref in row["modules"]:
            relation_counts[ref]["documentation_nodes"] += 1
            if row["route_kind"] != "package_contract_root":
                relation_counts[ref]["specific_documentation_nodes"] += 1
    for registry_path, key, id_key in [
        (manifest["architecture_section_registry"], "section", "id"),
        (manifest["capability_registry"], "cell", "id"),
        (manifest["invariant_registry"], "invariant", "id"),
        (manifest["delivery_registry"], "slice", "id"),
    ]:
        for row in rows(load(registry_path), key, id_key).values():
            for ref in row.get("modules", []):
                relation_counts[ref]["architecture_relations"] += 1
    for row in rows(load(manifest["port_registry"]), "port", "name").values():
        package = row["implementation_package"]
        relation_counts[f"{package}:{row['implementation_module']}"]["port_relations"] += 1
        for method_module in row.get("method_modules", []):
            relation_counts[f"{package}:{method_module}"]["port_method_relations"] += 1
    for packet in load(manifest["schema_registry"]).get("packet", []):
        for group in load(packet["path"]).get("group", []):
            for prefix in ("shape_owner", "meaning_owner", "state_owner"):
                package = group.get(f"{prefix}_package")
                module = group.get(f"{prefix}_module")
                if package != "NONE" and module != "NONE":
                    relation_counts[f"{package}:{module}"]["schema_relations"] += 1
    for row in config_rows.values():
        relation_counts[f"{row['owner']}:{row['owner_module']}"]["configuration_relations"] += 1
    for row in recipe_rows.values():
        for ref in row.get("execution_modules", []):
            relation_counts[ref]["recipe_relations"] += 1
    for row in dependency_rows:
        relation_counts[f"{row['consumer']}:{row['consumer_module']}"]["dependency_relations"] += 1
        relation_counts[f"{row['producer']}:{row['producer_module']}"]["dependency_relations"] += 1

    module_rows: list[dict[str, Any]] = []
    weak_modules: list[str] = []
    for package in sorted(modules):
        for module in modules[package]:
            ref = f"{package}:{module}"
            counts = relation_counts[ref]
            structural_rationale = STRUCTURAL_MODULE_ROLES.get(ref, "")
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
            module_rows.append(
                {
                    "id": ref,
                    "package": package,
                    "module": module,
                    "role": role,
                    "structural_rationale": structural_rationale,
                    "operation_count": counts.get("operations", 0),
                    "documentation_node_count": counts.get("documentation_nodes", 0),
                    "specific_documentation_node_count": counts.get("specific_documentation_nodes", 0),
                    "architecture_relation_count": counts.get("architecture_relations", 0),
                    "port_relation_count": counts.get("port_relations", 0),
                    "port_method_relation_count": counts.get("port_method_relations", 0),
                    "schema_relation_count": counts.get("schema_relations", 0),
                    "configuration_relation_count": counts.get("configuration_relations", 0),
                    "recipe_relation_count": counts.get("recipe_relations", 0),
                    "dependency_relation_count": counts.get("dependency_relations", 0),
                    "weakly_covered": weak,
                }
            )

    return {
        "modules": modules,
        "public_entries": public,
        "package_rows": package_rows,
        "operation_rows": operation_rows,
        "documentation_rows": doc_rows,
        "dependency_rows": dependency_rows,
        "module_rows": module_rows,
        "weak_modules": weak_modules,
        "selected_markdown": selected_markdown,
    }


def q(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def arr(values: Iterable[str]) -> str:
    return "[" + ", ".join(q(value) for value in values) + "]"


def render_operation_registry(graph: dict[str, Any]) -> str:
    rows_ = graph["operation_rows"]
    lines = [
        "schema_version = 2",
        'project = "eliot-search"',
        'status = "EXACT_OPERATION_TO_LOGICAL_MODULE_COVERAGE_CLOSED_NOT_IMPLEMENTED"',
        'source_registry = "swarm/function-packets.toml"',
        'module_registry = "swarm/module-packets.toml"',
        f"operation_count = {len(rows_)}",
        "one_module_per_operation = true",
        "operation_module_must_share_package = true",
        "implementation_authorized_by_this_registry = false",
        "",
    ]
    for row in rows_:
        lines.extend(
            [
                "[[operation]]",
                f"id = {q(row['id'])}",
                f"package = {q(row['package'])}",
                f"operation = {q(row['operation'])}",
                f"module = {q(row['module'])}",
                f"public_entry_module = {q(row['public_entry_module'])}",
                f"sources = {arr(row['sources'])}",
                f"source_contexts = {arr(row['source_contexts'])}",
                f"route_kind = {q(row['route_kind'])}",
                f"score = {row['score']}",
                "",
            ]
        )
    return "\n".join(lines)


def render_documentation_registry(graph: dict[str, Any]) -> str:
    rows_ = graph["documentation_rows"]
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "EXACT_DOCUMENTATION_NODE_OWNERSHIP_CLOSED_NOT_IMPLEMENTED"',
        f"source_file_count = {len(graph['selected_markdown'])}",
        f"node_count = {len(rows_)}",
        "heading_levels = [0, 1, 2, 3, 4]",
        "implementation_nodes_require_modules = true",
        "governance_and_navigation_nodes_require_non_crate_rationale = true",
        "implementation_authorized_by_this_registry = false",
        "",
    ]
    for row in rows_:
        lines.extend(
            [
                "[[node]]",
                f"id = {q(row['id'])}",
                f"path = {q(row['path'])}",
                f"line = {row['line']}",
                f"level = {row['level']}",
                f"heading = {q(row['heading'])}",
                f"kind = {q(row['kind'])}",
                f"packages = {arr(row['packages'])}",
                f"modules = {arr(row['modules'])}",
                f"route_kind = {q(row['route_kind'])}",
                f"rationale = {q(row['rationale'])}",
                "",
            ]
        )
    return "\n".join(lines)


def render_dependency_registry(graph: dict[str, Any]) -> str:
    rows_ = graph["dependency_rows"]
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "EXACT_CARGO_DEPENDENCY_SEMANTIC_EDGES_CLOSED_NOT_IMPLEMENTED"',
        f"edge_count = {len(rows_)}",
        "one_edge_per_registry_dependency = true",
        "producer_public_entry_only = true",
        "consumer_module_must_exist = true",
        "later_wave_dependency_requires_exact_stage_reentry = true",
        "implementation_authorized_by_this_registry = false",
        "",
    ]
    for row in rows_:
        lines.extend(
            [
                "[[edge]]",
                f"id = {q(row['id'])}",
                f"consumer = {q(row['consumer'])}",
                f"consumer_module = {q(row['consumer_module'])}",
                f"consumer_earliest_wave = {row['consumer_earliest_wave']}",
                f"producer = {q(row['producer'])}",
                f"producer_module = {q(row['producer_module'])}",
                f"producer_earliest_wave = {row['producer_earliest_wave']}",
                f"relationship = {q(row['relationship'])}",
                f"contract_source = {q(row['contract_source'])}",
                f"cargo_manifest = {q(row['cargo_manifest'])}",
                f"route_kind = {q(row['route_kind'])}",
                f"requires_stage_reentry = {'true' if row['requires_stage_reentry'] else 'false'}",
                f"reentry_stage = {q(row['reentry_stage'])}",
                f"exact_accepted_handoff_required = {'true' if row['exact_accepted_handoff_required'] else 'false'}",
                "",
            ]
        )
    return "\n".join(lines)


def render_module_registry(graph: dict[str, Any]) -> str:
    rows_ = graph["module_rows"]
    lines = [
        "schema_version = 1",
        'project = "eliot-search"',
        'status = "LOGICAL_MODULE_RELATION_COVERAGE_CLOSED_NOT_IMPLEMENTED"',
        f"module_count = {len(rows_)}",
        f"weak_module_count = {len(graph['weak_modules'])}",
        "public_entry_and_error_modules_are_structural_boundaries = true",
        "implementation_modules_require_specific_relation = true",
        "implementation_authorized_by_this_registry = false",
        "",
    ]
    for row in rows_:
        lines.extend(
            [
                "[[module]]",
                f"id = {q(row['id'])}",
                f"package = {q(row['package'])}",
                f"module = {q(row['module'])}",
                f"role = {q(row['role'])}",
                f"structural_rationale = {q(row['structural_rationale'])}",
                f"operation_count = {row['operation_count']}",
                f"documentation_node_count = {row['documentation_node_count']}",
                f"specific_documentation_node_count = {row['specific_documentation_node_count']}",
                f"architecture_relation_count = {row['architecture_relation_count']}",
                f"port_relation_count = {row['port_relation_count']}",
                f"port_method_relation_count = {row['port_method_relation_count']}",
                f"schema_relation_count = {row['schema_relation_count']}",
                f"configuration_relation_count = {row['configuration_relation_count']}",
                f"recipe_relation_count = {row['recipe_relation_count']}",
                f"dependency_relation_count = {row['dependency_relation_count']}",
                f"weakly_covered = {'true' if row['weakly_covered'] else 'false'}",
                "",
            ]
        )
    return "\n".join(lines)


def digest_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()
