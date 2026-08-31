#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from context_materialization_planner_v1.core import DECISION_COMMIT, MaterializationPlanError
from context_materialization_planner_v1.plan import build_plan, write_plan


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build a non-authoritative materialize_context_v1 plan.")
    parser.add_argument("--root", default=".")
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--bundle")
    parser.add_argument("--selection")
    parser.add_argument("--output-root", default="artifacts/context-materialization-plans")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--require-ready", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        build = build_plan(args.root, args.candidate, args.bundle, args.selection, args.output_root)
        if args.write:
            write_plan(args.root, build)
        print(json.dumps(build.plan, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    except MaterializationPlanError as exc:
        print(f"{exc.reason_code}: {exc.message}", file=sys.stderr)
        return 2
    if args.require_ready and build.plan["decision"] != DECISION_COMMIT:
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
