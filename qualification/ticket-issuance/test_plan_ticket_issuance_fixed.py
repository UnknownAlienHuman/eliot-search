from __future__ import annotations

# This module is the canonical unittest entrypoint. The substantive cases live in
# test_plan_ticket_issuance.py; the runner preloads the hyphenated planner module into sys.modules before
# importing that suite. Keeping this shim tiny makes the intended invocation explicit for IDEs and test
# discovery without duplicating the 18-case corpus.

from run_planner_tests import main


if __name__ == "__main__":
    raise SystemExit(main())
