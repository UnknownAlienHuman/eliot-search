#!/usr/bin/env python3
"""Build one deterministic non-authoritative P00 writer-context artifact candidate."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from context_artifact_builder_v1.build import build_candidate, write_candidate  # noqa: E402
from context_artifact_builder_v1.core import ARTIFACT_ROOT, CandidateFailure  # noqa: E402


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--package", required=True, help="exact P00 foundation package")
    parser.add_argument(
        "--base-commit",
        required=True,
        help="full algorithm-tagged immutable Git commit",
    )
    parser.add_argument(
        "--accepted-handoff",
        action="append",
        default=[],
        metavar="REPOSITORY_RELATIVE_PATH",
        help="committed accepted package_handoff_v1 path; repeat as needed",
    )
    parser.add_argument(
        "--output-root",
        default=ARTIFACT_ROOT,
        help=f"{ARTIFACT_ROOT} or a descendant",
    )
    parser.add_argument(
        "--print-result",
        action="store_true",
        help="print the compact candidate metadata JSON after writing",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    try:
        build = build_candidate(
            args.root,
            args.package,
            args.base_commit,
            args.accepted_handoff,
            args.output_root,
        )
        bundle_path, candidate_path = write_candidate(args.root, build)
    except CandidateFailure as exc:
        print(f"{exc.reason_code}: {exc.message}", file=sys.stderr)
        return 2
    result = {
        "candidate_id": build.candidate["candidate_id"],
        "bundle": bundle_path.relative_to(Path(args.root).resolve()).as_posix(),
        "candidate": candidate_path.relative_to(Path(args.root).resolve()).as_posix(),
        "artifact_sha256": build.candidate["artifact_candidate"]["sha256"],
        "candidate_sha256": build.candidate["candidate_sha256"],
        "status": build.candidate["status"],
    }
    if args.print_result:
        print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    else:
        print(
            f"READY: {result['candidate_id']} -> {result['bundle']} and {result['candidate']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
