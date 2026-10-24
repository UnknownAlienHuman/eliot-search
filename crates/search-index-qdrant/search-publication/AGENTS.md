# Working on publication

Read the [manifesto](../../../docs/architecture/README.md), [implementation](../../../docs/implementation/README.md)
and [FUNCTIONS.md](FUNCTIONS.md). Change the real planner, metadata, bridge, view and query consumers together.

Use compact evidence rows in redb and pin the actual read snapshot. Same-generation evidence changes and the
current-view update commit atomically after Qdrant readback; large rebuilds fill separate staging-generation
tables. Do not implement a second manifest B-tree, per-view full copy or generic temporal-record framework.

Bound the incremental metadata delta before external work. Partial expected mutation progress is retried per
item, not reported as corruption. Keep only unfinished pending/cleanup state. No database or I/O work under
the view-manager lock. Local object collection runs only with the daemon stopped and its root lock acquired;
do not restore the old ObjectGcGate protocol or lease platform.

Preserve useful existing code. Implement the selected slice and use minimal changed-owner check/Clippy;
runtime fault/scale evidence follows implementation, not a receipt or review gate.
