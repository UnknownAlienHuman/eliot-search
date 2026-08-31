#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from accepted_evidence_digest_v1 import EvidenceDigestError, result_record


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Compute accepted_evidence_digest_v1 from package_handoff evidence[].")
    parser.add_argument("record", help="package_handoff_v1 TOML file or JSON evidence array")
    parser.add_argument("--json-array", action="store_true", help="input is a JSON evidence array")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    path = Path(args.record)
    try:
        raw = path.read_bytes()
        if args.json_array:
            evidence = json.loads(raw.decode("utf-8", "strict"))
        else:
            record = tomllib.loads(raw.decode("utf-8", "strict"))
            if record.get("record_kind") != "package_handoff_v1":
                raise EvidenceDigestError("record_kind must be package_handoff_v1")
            evidence = record.get("evidence")
        print(json.dumps(result_record(evidence), sort_keys=True, separators=(",", ":")))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, tomllib.TOMLDecodeError, EvidenceDigestError) as exc:
        print(f"ACCEPTED_EVIDENCE_DIGEST_INVALID: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
