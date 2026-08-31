#!/usr/bin/env python3
from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "swarm/accepted-evidence-digest-v1.toml",
    "swarm/type-rule-profiles-v1.toml",
    "docs/handoff/ACCEPTED_EVIDENCE_DIGEST_V1.md",
    "tools/accepted_evidence_digest_v1.py",
    "tools/compute-accepted-evidence-digest.py",
    "qualification/accepted-evidence/test_accepted_evidence_digest_v1.py",
]


def main() -> int:
    errors: list[str] = []
    for rel in REQUIRED:
        if not (ROOT / rel).is_file():
            errors.append(f"missing: {rel}")
    try:
        profile = tomllib.loads((ROOT / REQUIRED[0]).read_text(encoding="utf-8"))
        mapping = tomllib.loads((ROOT / REQUIRED[1]).read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"TOML parse: {exc}")
        profile = {}
        mapping = {}
    if profile.get("profile") != "accepted_evidence_digest_v1":
        errors.append("profile identity mismatch")
    if profile.get("self_referential_digest_allowed") is not False:
        errors.append("self-referential digest must remain false")
    rows = mapping.get("binding", [])
    exact = [row for row in rows if isinstance(row, dict) and row.get("type") == "OrderedAcceptedPackageHandoff" and row.get("field") == "evidence_digest"]
    if len(exact) != 1 or exact[0].get("profile") != "accepted_evidence_digest_v1":
        errors.append("type-rule profile binding missing or duplicate")
    workflow = ROOT / ".github/workflows/accepted-evidence-digest.yml"
    if workflow.is_file():
        text = workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:", "contents: read", "persist-credentials: false"):
            if token not in text:
                errors.append(f"workflow missing {token}")
        for forbidden in ("\n  push:", "\n  pull_request:", "\n  schedule:"):
            if forbidden in text:
                errors.append(f"automatic workflow trigger: {forbidden.strip()}")
    result = {"status": "PASS" if not errors else "FAIL", "required_files": len(REQUIRED), "bindings": len(rows), "errors": errors}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
