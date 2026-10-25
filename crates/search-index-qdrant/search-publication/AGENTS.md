# Working on publication

Read the [implementation spine](../../../docs/implementation/README.md), local
[FUNCTIONS.md](FUNCTIONS.md), the [bridge](../search-qdrant-bridge/FUNCTIONS.md) and
[control](../../search-control-redb/FUNCTIONS.md).

Implement one serialized publisher and one pending artifact with actual bounded Qdrant inputs.

Baseline invariants:

- new points are immutable and use insert-only creation;
- retry only the same fixed point value and verify exact readback;
- closure writes one fixed `valid_until` value;
- never physically delete individual points;
- compaction/rebuild creates a fresh never-reused generation;
- cleanup deletes whole retired generations after all view/window guards are gone;
- one redb transaction installs evidence/inventory/current and removes pending;
- the matching redb snapshot is installed before reporting visibility;
- local object collection is stopped-root maintenance.

Do not implement isolated-route succession, possible-writer receipts, per-point cleanup journals, full manifest
copies or another publication state machine. Change bridge/control/view/query consumers together. After code run
minimal Rust 1.98 compiler/Clippy for actual changed owners, then focused retry/closure/view/restart/generation
cases.
