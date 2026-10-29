# Working on the legacy evaluation package

Follow [root instructions](../../AGENTS.md) and [test cutover](../../docs/implementation/TEST_CUTOVER.md).
This package still exists on main; removal work in PR #610 is separate. Trace current callers before deletion,
keep its included README until its source is removed, and do not build new receipts, gates or evaluation services.
