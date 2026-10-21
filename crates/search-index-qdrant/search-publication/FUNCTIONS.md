# Publication: real effects and one atomic metadata boundary

At `c8e38b6`, this package's admission/machine/recovery modules hold full manifests and caller receipts without
redb/Qdrant/daemon effects. Daemon and control codecs add two competing machines. Keep useful validation and
exact readback; remove those machines rather than synchronizing their labels.

The [control map](../../search-control-redb/FUNCTIONS.md) owns evidence tables/snapshots; the
[planner](../search-projection-planner/FUNCTIONS.md) owns changed-source input. This file owns effect order.

## Preparation

Choose expected current view and target sequence under the serialized publisher. Produce finite immutable
operation pages carrying actual Qdrant inputs, compact binding/source deltas and exact cleanup IDs. Source
selection, occurrence births and inputs are fixed for this operation; recovery does not reread live paths.

Before any Qdrant mutation, validate the incremental delta's row/byte ceilings, complete-source boundaries
and cleanup capacity. Store the verified operation reference in one `PendingPublication`. A preparation
failure before that reference leaves collectible debris, not a durable success record.

No online local object deletion runs in this baseline. Offline maintenance obtains the root lock only after
the daemon stops, so it cannot collect an object between finalization and its first durable reference.

## Apply and uncertain outcomes

Each RPC uses the same operation deadline. With `wait=true`, inspect the exact affected state before advancing
the cursor. After lost acknowledgement or partial batch application, classify each item:

- desired post-state: done;
- expected pre-state or absent new item: perform the idempotent missing operation;
- genuinely foreign identity/state: stop that publication and preserve evidence of the unfinished work.

A mix of old and desired state is not by itself corruption. Do not re-upsert a whole point to repair a narrow
payload/vector update. The bridge has no second replay ledger. Once a mutation may exist, finish the same
pending operation forward; cancellation does not authorize changing inputs or reusing a closed interval.

## Ordinary incremental commit

Qdrant validity uses `valid_from=target` and exclusive `valid_until=target`, so old pinned sequences remain
queryable while effects are prepared. A point retained for a new citation keeps its old retrieval state.

After required readbacks, decode the already bounded metadata delta before opening the writer:

```text
one redb write transaction:
  compare expected current and exact pending operation
  apply evidence changes and published source inventory changes
  current = target
  insert unfinished Qdrant cleanup, if any
  remove pending
commit
open read snapshot; verify its current row is target
install snapshot-backed target under short view-manager lock
report visible
```

No object/RPC I/O or unbounded corpus delta inside the transaction. The serialized publisher does not start
another publication before snapshot capture/install. If commit acknowledgement is uncertain, resolve current,
pending and cleanup rows first. If snapshot construction/install fails after durable commit, keep old serving
state or refuse new serving and recover; do not report target visible or roll it back.

An old request uses its old redb snapshot and Qdrant sequence in the commit/install gap. A new read of the
latest evidence table must never be combined with an old view sequence.

## Initial build or incompatible profile

Build a new physical Qdrant generation and separate source/evidence tables. Populate staging metadata in
bounded transactions; it is not query-visible. Pending state names the staging generation and exact build
inputs. Validate completeness, then atomically switch current and transfer old-generation cleanup. The final
transaction changes pointers, not every corpus row. Never copy all old/new bindings into a plan in memory.

Changes to source heads during the build are queued for a later update. The published source inventory names
the actual built revisions and gaps. A failed/restarted build resumes its existing target; do not silently
adopt the newest source state or unrelated Qdrant collection.

## View lifetime

`PublishedViewState` privately owns a redb read snapshot plus route/profile/sequence. One current Arc and weak
retired references are enough. Queries acquire only current; live windows clone their existing guard. Never
acquire an old view by an arbitrary ID. Redb/table types do not enter the public protocol.

The manager lock covers pointer/weak-reference changes only, not database, object or network I/O. Last drop
can wake cleanup but performs no deletion. Long snapshots retain old redb pages; finite request/window time
and update backpressure are real resource requirements, not proof that snapshots cost nothing.

## Qdrant cleanup

Persist only unfinished `RetiredCleanup` rows in the same commit that clears pending. Point closures at N wait
until no live view on that generation has sequence below N. A whole collection waits for every live user,
not merely the immediate predecessor. v1 can still need a point when v2 is gone and v3 closes it.

Read back absence per exact ID/collection and advance/remove the cleanup row. A partially deleted page retries
its remaining exact IDs; foreign identity is an error, partial progress is not. Unknown outcomes keep the row.
Bound the backlog before accepting more effects. Generation metadata tables can be dropped after the last
view and relevant pending/cleanup consumer is gone; no permanent table per published view is created.

On restart, open durable current with one consistent read snapshot, resume pending, and reevaluate cleanup.
Old process-local queries are gone. Local object collection is an explicit stopped-root maintenance operation,
not part of startup/query or a hidden full-corpus scan.

## Remove and verify

Delete old phase/receipt/observation/restore/compensation machines, full manifest encodings, raw pins and the
separate reclaimer journal after callers move. Preserve #598's private overlap; it does not impose its old
six-domain/retention programme on these independent publication rules.

Verify after code: pre/partial/post RPC recovery, delta limit before effects, atomic bindings/current commit,
old snapshot in commit/install gap, new-generation staging isolation, v1/v2/v3 cleanup, restart and stalled
reader disk growth. No runtime result is asserted by this document.
