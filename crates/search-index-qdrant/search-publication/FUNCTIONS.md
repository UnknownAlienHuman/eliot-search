# Publication: real effects and one atomic metadata boundary

At c8e38b6, shared admission/machine/recovery code holds complete manifests and caller receipts without
redb/Qdrant effects. Daemon/control add competing machines. Keep useful validation/readback, not the machines.

The [control map](../../search-control-redb/FUNCTIONS.md) owns evidence tables/snapshots;
[projection](../search-projection-planner/FUNCTIONS.md) owns changed-source input;
[preparation](../../search-prep/search-materializer/FUNCTIONS.md) owns wanted-input/result reconciliation.
This file owns effect order. It is a proposed implementation, not executed product evidence.

## Select and prepare

Under the serialized publisher, compare eligible current preparation outcomes with published source inputs.
An in-memory wake-up is only an optimization. Startup, queue overflow and bounded reconciliation must find
unpublished current inputs even if the process died after preparation commit but before notification.

Choose expected current view and target sequence. Produce finite immutable pages with actual Qdrant inputs,
compact complete-source deltas and cleanup IDs. Validate row/byte ceilings and cleanup capacity before any
external mutation. Commit one PendingPublication referencing the verified artifact. Its source selection,
occurrence births, profile and readback targets are fixed; recovery never rereads live paths.

Before pending/effects, obsolete preparation can be discarded/replanned. After effects may exist, finish
that same operation and then reconcile newer source inputs. Never patch fresh bytes/births into an old cursor.
A built view may lag current source heads; its published inventory names its actual revisions and gaps.

No online local object deletion runs in this baseline. Stopped-root maintenance cannot race finalization
and its first durable reference.

## Apply and uncertain outcomes

One execution attempt has one absolute deadline and cancellation context across its RPCs. Durable pending
input is not an expired wall-clock job: after restart/backoff an authorized recovery attempt gets a new
finite budget, but keeps the same operation, births, target and cursor. Nested calls never renew that attempt's
deadline. Cancelling an attempt does not erase unresolved effects or permit immediate infinite retry.

With wait=true, inspect affected state before advancing:

- desired post-state: done;
- expected pre-state or absent new item: perform only the idempotent missing work;
- foreign identity/state: stop this publication with the unfinished input preserved.

Mixed pre/post state is not itself corruption. Do not repair a narrow update with whole-point upsert.
The bridge owns no second replay ledger. Do not rewind a published sequence or reopen a closed interval.

## Ordinary incremental commit

New points use valid_from=target; closed points use exclusive valid_until=target. Old guarded sequences
remain queryable. A retained point with changed citation keeps its retrieval state and changes evidence only.

After required readbacks, decode the already bounded metadata delta before the writer:

```text
one redb transaction:
  compare expected current and exact pending
  apply complete-source evidence and published inventory changes
  current = target
  insert unfinished Qdrant cleanup, if any
  remove pending
commit
open read snapshot; verify its current row is target
install snapshot-backed target under short manager lock
report visible
```

No object/RPC I/O or unbounded corpus delta inside the writer. Do not begin another publication before
snapshot capture/install. On uncertain commit inspect current/pending/cleanup. Failure after durable commit
keeps old serving state or refuses new serving until recovery; it does not report uninstalled target visible.
An old request always reads its old redb snapshot, never latest bindings under an old sequence label.

## Empty, unsupported and no-work are different

Preparation's successful zero-unit outcome is publishable. Replacing nonempty input with supported empty
input closes its former points and commits the new source/evidence state; an initial empty source requires
no upsert. Removing the last source can produce a valid empty published inventory. Do not add sentinel points
or preserve stale nonempty results to bypass EmptyUnitSet checks.

Unsupported/incomplete extraction remains an explicit source gap, not successful empty. Until an update is
published, any retained old result is visibly from the old published revision. A gap-only new source outcome
must not present old evidence as if it came from the failed input. Missing collection, missing metadata and
an unfinished initial build are unavailable/incomplete, not a successful empty view.

If the exact published input/outcome already matches, skip a redundant publication. Do not allocate sequences
or create zero-effect pending records on every reconciliation pass.

## Initial build or incompatible profile

Use a new Qdrant generation and separate source/evidence tables, populated in bounded transactions. Staging
is not query-visible. Pending names its fixed generation/inputs. After complete validation, atomically switch
current and transfer cleanup; the final transaction changes pointers, not every corpus row.

Source changes during the build remain detectable by comparing current inputs with the built inventory,
not only by queued notifications. Failed/restarted builds resume the same target. A zero-point generation
still needs its selected schema and explicit complete source inventory; absence is not equivalent.

## View lifetime and cleanup

PublishedViewState owns the real redb snapshot and route/profile/sequence. One current Arc and weak retired
refs suffice. Pin current only; a window inherits a live guard. No manager lock across database/object/RPC I/O.
Last drop may wake cleanup but does not delete. Old readers retain redb pages; finite request/window lifetime
and write backpressure require measurement.

Transfer actual unfinished deletes to finite RetiredCleanup rows in the same current commit. Points closed
at N wait for every live same-generation view below N; a collection waits for all its users. Immediate-
predecessor drain is insufficient. Exact absence advances cleanup; partially deleted pages retry remaining
expected IDs, while foreign state stops them. Bound backlog before more effects. Retire generation tables
only after their view/pending/cleanup consumers disappear. Local artifact files await stopped-root GC.

## Restart is not restore

For an ordinary restart of this same installation, open durable current consistently, resume pending and
reevaluate cleanup. Old process-local queries are gone. Reconstruct missing preparation/publication work
from current inputs/outcomes; do not require another filesystem event.

A backup copy is not authorized to enter this replay path. The [restore recipe](../../search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport)
imports supported source/prepared state into a fresh destination and builds its own generation. Copied
pending/cleanup routes are archive evidence, never mutation authority over the original installation.
A lost index in the same installation is also not an empty success: rebuild from verified retained inputs
into a fresh generation instead of silently attaching an unrelated collection.

## Remove and verify

Delete old phase/receipt/observation/restore/compensation machines, full-manifest encodings, raw pins and the
separate reclaimer after callers move. Preserve #598's private overlap, not its superseded programme.

After code check crash before enqueue, out-of-order preparation, empty/unsupported transitions, pre/partial/
post-RPC retry, fresh recovery-attempt budget, atomic source/evidence/current commit, commit/install gap,
staging isolation, v1/v2/v3 cleanup, restart, old-reader disk retention and restore isolation. Documentation
alone establishes none of those runtime outcomes.
