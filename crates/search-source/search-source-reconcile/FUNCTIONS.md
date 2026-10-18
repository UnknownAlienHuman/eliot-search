# search-source-reconcile — functions and integration

The [implementation guide](../../../docs/implementation/README.md) owns the application path.
This map describes inspected source and the incremental update work still needed.

## Existing code

| Function or owner | Actual behavior |
|---|---|
| [`reconcile_source`](src/lib.rs) | Compares one `RegisteredSource` and `ReconcileObservation`. Produces no-change, content update, path rebind, identity binding, confirmed retirement or replacement nomination. Performs no I/O. |
| `ReconcileBatch::validate` | Validates a supplied finite batch. It does not enumerate a root. |
| `ReconcileLedger::record` | Searches a vector of operation IDs/input digests and retains outcomes without pruning; default capacity is 2,000,000 operations. This is old bookkeeping to remove, not durable inventory. |
| [`SourceRootCatalog::refresh`](../../../bins/eliot-searchd/src/source_roots/kernel/catalog.rs) | Probes root availability and calls `SourceRootCurrentness::reconcile`; it does not discover changed files. Watcher hint methods have no connected native producer. |
| [`SourceRootCurrentness`](../search-source-records/src/root_currentness.rs) | Tracks root states, root-level hints, overflow and generations. `workspace_current` also requires the last synced generation; root availability alone does not prove unchanged content. |
| [`run_source_sync_preflight`](../../../bins/eliot-searchd/src/owner_composition/kernel/source_sync.rs) | Even successful preflight ends in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. Replace that refusal with actual ingestion. |

The old `ingest_watch_hint`, `execute_inventory_slice`, `commit_reconcile` and recovery/receipt APIs
listed here were not implemented functions. Do not create them just to satisfy the old document.

## One incremental scheduler

Daemon composition owns one bounded work queue across selected roots. Repeated hints collapse to dirty
paths; an uncertain or overflowing path set becomes an affected-root scan. Retain current source/head
records and scan progress needed by real work, not a lifetime event ledger or a supervisor per root.

Read confirmed changes through the existing source reader and writer. Compare content identity before
repeating preparation and Qdrant work. A rename can change location without preparing identical content
again. Metadata and watcher silence are hints, not proof that bytes are unchanged.

An inventory error or cancellation leaves its scope incomplete. Do not retire unseen sources or clear
a gap from that partial inventory. Changes observed during a scan remain pending after its completion.
An explicitly read missing path can be retired without waiting for an unrelated full-portfolio scan.
Startup, resume, periodic reconciliation and explicit sync reuse this same path.

## Walk and watch donors

Use `ignore::WalkBuilder` from ripgrep for filesystem discovery, with explicit hidden-file, parent/global
ignore, `.ignore` and `.gitignore` settings. Nested rules and negations belong to the donor, not another
glob grammar. An ignore-file change rescans its affected subtree. Git metadata changes re-resolve the
selected Git view. Exclude application storage and avoid scanning an independently registered nested
root twice.

`WalkBuilder::add` supports multiple roots; serial iteration or `build_parallel().run(...)` emits entries
and errors directly. Bound downstream read/preparation work and choose a shared thread budget. Parallel
order is not deterministic; do not collect and sort an entire portfolio to manufacture a receipt.
`max_filesize` filters metadata only: the actual reader still enforces the byte limit. `walkdir` is the
lower-level donor already used by `ignore`, not a second Eliot walker. Its `max_open` bounds descriptors,
not memory, and sorting buffers directory entries.

One `notify::RecommendedWatcher` can watch multiple roots and feed hints to this queue. It cannot prove
continuity. The inspected Windows backend does not expose every lost-event condition as a rescan event;
keep periodic/startup recovery. `PollWatcher` already performs repeated complete walks, so do not layer
it over the same periodic scanner by default. No USN service is required for the first connected path.

Pinned source: [ignore walk](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs),
[walkdir](https://github.com/BurntSushi/walkdir/blob/4f26be4d450910916ea11533b2efc52b9a6483bc/src/lib.rs),
[notify Windows](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs),
[notify poll](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/poll.rs).

## Git deltas use the same ingestion path

Working-tree bytes, staged index blobs and committed tree blobs are distinct source views. Resolve the
selected commit/tree once; a later `HEAD` change cannot relabel previously read bytes. Capture index
entries with their stage when that view is selected; unresolved merge stages are not ordinary stage-0
content. Keep path/source occurrence separate from blob content identity.

Use the controlled local Git plumbing path in [safe-reader](../search-safe-reader/FUNCTIONS.md):
initial `ls-tree`, subsequent raw `diff-tree` between exact tree IDs, then batch-read only new/changed
blobs. Disable similarity/rename detection for this task. Missing local objects remain a visible gap;
submodules are separate selected roots, not implicit recursive fetches.

If packaging later requires operation without installed Git, the native alternative is
`gix_diff::tree` with `gix_object::TreeRefIter`, a reusable `State` and a `Visit` callback over the local
ODB. Its [tree comparison](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-diff/src/tree/function.rs)
skips equal-OID subtrees. The high-level `Tree::changes` path constructs a blob/attribute resource cache
and supports similarity detection that ingestion does not need. Do not maintain both backend paths now.
