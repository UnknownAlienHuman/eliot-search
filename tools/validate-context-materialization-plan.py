#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "swarm/context-materialization-planner-v1.toml",
    "swarm/context-materialization-plan-schema-v1.toml",
    "swarm/context-materialization-plan-digest-v1.toml",
    "swarm/context-manifest-instance-v1.toml",
    "swarm/context-manifest-renderer-v1.toml",
    "swarm/accepted-evidence-digest-v1.toml",
    "tools/plan-context-materialization.py",
    "tools/context_materialization_planner_v1/core.py",
    "tools/context_materialization_planner_v1/manifest.py",
    "tools/context_materialization_planner_v1/plan.py",
    "qualification/context-materialization/cases-v1.toml",
    "qualification/context-materialization/test_context_materialization_plan_v1.py",
    "docs/handoff/CONTEXT_MATERIALIZATION_PLAN_V1.md",
]


def read_toml(rel: str, errors: list[str]) -> dict:
    try:
        return tomllib.loads((ROOT / rel).read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"{rel}: {exc}")
        return {}


def main() -> int:
    errors: list[str] = []
    for rel in REQUIRED:
        if not (ROOT / rel).is_file():
            errors.append(f"missing: {rel}")
    registry = read_toml(REQUIRED[0], errors)
    schema = read_toml(REQUIRED[1], errors)
    digest = read_toml(REQUIRED[2], errors)
    instance = read_toml(REQUIRED[3], errors)
    renderer = read_toml(REQUIRED[4], errors)
    cases = read_toml(REQUIRED[10], errors)

    if registry.get("component") != "context_materialization_planner_v1":
        errors.append("planner component mismatch")
    if registry.get("operation_kind") != "materialize_context_v1":
        errors.append("operation kind mismatch")
    authority = registry.get("authority")
    if not isinstance(authority, dict) or not authority or any(authority.values()):
        errors.append("planner authority ceiling must be all false")
    if registry.get("signature_refs_are_operation_id_inputs") is not False:
        errors.append("signature refs must be excluded from operation ID")
    if registry.get("signature_refs_bind_post_operation_signed_payload") is not True:
        errors.append("post-operation signature binding missing")
    if schema.get("record_kind") != "context_materialization_plan_v1":
        errors.append("plan schema kind mismatch")
    invariants = schema.get("invariants")
    if not isinstance(invariants, dict) or invariants.get("control_record_mutations_must_be_empty") is not True or invariants.get("all_authority_fields_must_be_false") is not True:
        errors.append("plan schema authority invariants missing")
    if digest.get("self_referential_digest_allowed") is not False:
        errors.append("plan digest must remain non-self-referential")
    if instance.get("instance_status") != "MATERIALIZED":
        errors.append("context manifest instance status mismatch")
    if renderer.get("signature_table_excluded_from_signed_payload") is not True:
        errors.append("renderer signature boundary mismatch")
    renderer_invariants = renderer.get("invariants")
    if not isinstance(renderer_invariants, dict) or renderer_invariants.get("self_referential_complete_file_digest_allowed") is not False:
        errors.append("renderer allows complete-file self hash")
    rows = cases.get("case", [])
    if cases.get("case_count") != 12 or len(rows) != 12:
        errors.append("materialization corpus must contain exactly twelve cases")
    ids = [row.get("id") for row in rows if isinstance(row, dict)]
    if len(ids) != len(set(ids)):
        errors.append("materialization case IDs are not unique")

    workflow = ROOT / ".github/workflows/context-materialization-plan.yml"
    if not workflow.is_file():
        errors.append("missing manual workflow")
    else:
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        if re.search(r"^\s{2}(push|pull_request|schedule|workflow_run):", text, re.MULTILINE):
            errors.append("workflow has automatic trigger")

    plan_source = (ROOT / "tools/context_materialization_planner_v1/plan.py").read_text(encoding="utf-8") if (ROOT / "tools/context_materialization_planner_v1/plan.py").is_file() else ""
    core_source = (ROOT / "tools/context_materialization_planner_v1/core.py").read_text(encoding="utf-8") if (ROOT / "tools/context_materialization_planner_v1/core.py").is_file() else ""
    if '"control_record_mutations": []' not in plan_source:
        errors.append("plan implementation lacks empty control mutation field")
    if "AUTHORITY_FIELDS" not in core_source:
        errors.append("authority field registry missing")

    result = {
        "status": "PASS" if not errors else "FAIL",
        "required_files": len(REQUIRED),
        "cases": len(rows),
        "decisions": len(schema.get("closed_decisions", [])),
        "errors": errors,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
