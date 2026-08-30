from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLANNER_PATH = ROOT / "tools/plan-ticket-issuance.py"
TEST_PATH = ROOT / "qualification/ticket-issuance/test_plan_ticket_issuance.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load module spec: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def main() -> int:
    # Preload the planner under the exact name used by the test module. This makes dataclass/type
    # resolution deterministic across CPython versions when the module is loaded from a hyphenated path.
    load_module("ticket_issuance_planner", PLANNER_PATH)
    test_module = load_module("ticket_issuance_planner_tests", TEST_PATH)
    suite = unittest.defaultTestLoader.loadTestsFromModule(test_module)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
