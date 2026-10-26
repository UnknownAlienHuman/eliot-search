# View lifetime — pin exact physical generation and evidence snapshot

At `c8e38b6`, `PinRegistry` exposes arbitrary route/epoch acquisition, renewal, sampled watermarks and a second
continuation catalogue. Remove the public registry with callers.

The surviving manager is private to publication/query:

```text
current: Arc<PublishedView>
retired: weak references to views still held by requests/windows
```

`PublishedView` contains the exact physical Qdrant collection, retrieval profile and real redb read snapshot of
published source/evidence tables. Stage 2/3A has no intra-generation sequence; Stage 3B may add one only if required
by failed release budgets.

The [publication recipe](../search-publication/FUNCTIONS.md) owns commit, snapshot capture, installation, cleanup
eligibility and restart. The control owner owns lookup. Do not duplicate those algorithms or create a snapshot
service.

## Acquisition and lock order

Acquire current only under a short manager mutex. A live request clones the guard and releases the mutex before
scope resolution, `TextQuery`, evidence/object reads or rendering. A live `QueryWindow` may inherit that same guard;
callers cannot reopen arbitrary old views by ID.

After durable current/evidence commit, publication opens/verifies the matching snapshot and then installs the view
under the short mutex. Do not begin another publication until install succeeds or startup/recovery refuses serving.

No manager lock spans redb writer, object, Qdrant, process or output work. Last guard drop may wake cleanup and
never performs I/O.

## Stage 3A lifetime and measurements

Old guards continue using the old immutable physical collection/snapshot after a new view installs. Whole-
generation cleanup waits for every guard, every pending/future target and every unresolved targetable attempt.

Old redb pages and Qdrant collections consume real disk. Measure current + building + reader-retained generations
under #215's maximum supported request/window lifetime. Arc count is not a byte bound. If Stage 3A fails declared
disk/latency budgets, adjust supported lifetimes or implement Stage 3B; do not hide retention behind a watermark.

## Stage 3B

If incremental publication is required, the same view carries physical generation/profile/sequence. Fixed
exclusive closure may happen while older same-generation views live; whole-generation deletion still waits for all
guards. No second pin system.

## Local object collection

There is no live local-object GC gate. Local collection owns a stopped root exclusively. Qdrant cleanup remains
online and whole-generation. Keep only the small manager in the actual publication/query owner and delete this
crate after callers move.
