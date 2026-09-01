#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from package_maps_v2 import MAP_ROOT, ROOT, build_outputs, patch_manifest, write_outputs


def check_outputs(outputs: dict[str, str]) -> list[str]:
    stale: list[str] = []
    for relative, expected in sorted(outputs.items()):
        path = ROOT / relative
        if not path.is_file() or path.read_text(encoding="utf-8") != expected:
            stale.append(relative)
    expected_package_files = {
        relative for relative in outputs if relative.startswith(MAP_ROOT + "/")
    }
    actual_package_files = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / MAP_ROOT).rglob("*.toml")
    } if (ROOT / MAP_ROOT).exists() else set()
    stale.extend(sorted(expected_package_files ^ actual_package_files))
    return sorted(set(stale))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    outputs, stats = build_outputs()
    if args.check:
        stale = check_outputs(outputs)
        status = "PASS" if not stale else "FAIL"
        result = {"status": status, **stats, "stale": stale}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if status == "PASS" else 1

    write_outputs(outputs, stats)
    result = {"status": "GENERATED", **stats, "output_count": len(outputs)}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
