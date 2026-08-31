#!/usr/bin/env python3
"""Read-only schema-v2 P00 ticket-issuance advisory planner.

Repository inputs are read from one immutable Git commit. Output is stdout or an ordinary local JSON
artifact below artifacts/ticket-issuance-plans/. No control-record mutation path exists.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Sequence

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from ticket_issuance_planner_v2.core import *  # noqa: E402,F401,F403
from ticket_issuance_planner_v2.plan import build_plan, write_plan  # noqa: E402


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--package", required=True, help="exact P00 package")
    parser.add_argument(
        "--base-commit",
        help="full algorithm-tagged immutable Git commit",
    )
    parser.add_argument("--writer", help="closed ActorIdentity")
    parser.add_argument(
        "--reviewer", help="closed ActorIdentity distinct from writer"
    )
    parser.add_argument(
        "--accepted-handoff",
        action="append",
        default=[],
        metavar="REPOSITORY_RELATIVE_PATH",
        help="committed accepted package_handoff_v1 path; repeat as needed",
    )
    parser.add_argument(
        "--output",
        default="-",
        help=f"stdout or JSON below {PLAN_ARTIFACT_ROOT}/",
    )
    parser.add_argument(
        "--require-ready",
        action="store_true",
        help="return exit 3 unless decision is preview-ready",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    try:
        plan, target = build_plan(args)
        write_plan(plan, target)
    except PlannerFailure as exc:
        print(f"{exc.reason_code}: {exc.message}", file=sys.stderr)
        return 2
    if args.require_ready and plan["decision"] != DECISION_READY:
        return 3
    return 2 if plan["decision"] == DECISION_INVALID else 0


if __name__ == "__main__":
    raise SystemExit(main())
