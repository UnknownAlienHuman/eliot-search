# Publication — one pending operation, immutable points, atomic view

Publication is the only owner of Qdrant mutation, published source/evidence state, current `PublishedView` and
retired-generation cleanup. The bridge owns SDK translation; control owns redb tables/snapshots; preparation owns
current wanted outcomes.

## Baseline safety decision

Individual Qdrant points are never physically deleted in the baseline.

New retrieval points are immutable and created with explicit `insert_only`. Default upsert is forbidden because
it overwrites an existing point. A retry carries the same fixed point ID, retrieval input and immutable payload.

Replaced points receive one fixed exclusive `valid_until = target_sequence` through an operation which cannot
create a missing point. Repeating the same closure is allowed; reopening the interval or choosing another
boundary is not.

A fixed closure may be applied while older views still live: their sequence is below the exclusive boundary and
remains eligible. Reader lifetime blocks whole-generation deletion, not the closure mutation.

Because a closed point remains present, a delayed duplicate insert cannot recreate or overwrite it. Storage is
compacted by building a fresh never-reused generation and switching the current view. Cleanup deletes only whole
retired generations. This removes the old per-point possible-writer/reclaimer/isolated-route system.

These are selected invariants. The pinned Qdrant 1.19 server/client behavior must still be executed before a
runtime claim.

## Reconcile desired and published state

Compare both sides under one consistent redb snapshot:

- current source/preparation desired state;
- current published source inventory.

Use bounded ordered pages; never collect the corpus in memory. Published-only removal/exclusion is work even
without a new preparation event. Repeated identical state is a no-op.

| Desired state | Result |
|---|---|
| usable preparation, including zero units | publish exact prepared input/gaps; create new points; close replaced points |
| selected terminal unsupported/failure | publish a gap-only outcome; close former points |
| confirmed removal/unregistration/exclusion | remove membership and close former points |
| preparation running or observation incomplete | retain prior served revision as prior/stale and expose pending/gap; do not infer empty/removal |
| exact desired/published match | no publication |

Queues are acceleration only. Startup/explicit/periodic reconciliation rediscovers differences.

## Prepare one fixed pending artifact

Under one serialized publisher:

```text
pin expected current view
→ choose target sequence/generation
→ build bounded immutable pages containing actual Qdrant inputs
→ build compact evidence/inventory delta
→ validate row/byte limits
→ persist one PendingPublication referencing those exact pages
```

After pending is durable, retries never reread live paths or splice newer work into the cursor. Newer source/config
state becomes later work.

Only one publisher may target a generation at a time. Once a generation is retired, no new pending operation may
name it.

## Apply incremental pages

Use one absolute deadline/cancellation per attempt. Persist page progress only after exact observation.

### Create point

```text
explicit insert_only of exact immutable point
→ completed response or retry the same fixed insert
→ exact readback of ID, immutable bindings, validity and vector shape
```

A timeout or lost response is unresolved for that attempt, but the identical retry is safe: an existing point is
not overwritten. Exact identical readback accepts completion; foreign/conflicting state stops publication.

### Close point

```text
non-creating update of exact valid_until = target_sequence
→ completed response or repeat the same fixed closure
→ exact readback
```

Use `update_only` when the selected full-point API is used, or another selected Qdrant operation whose documented
semantics cannot insert a missing point. Do not use default upsert. Missing point/readback is an inconsistency or
rebuild input, never permission to recreate a closed identity through a closure call.

No operation may remove the closure or choose another boundary.

## Atomic commit and visibility

After every required schema/point observation succeeds:

```text
one redb transaction:
  compare expected current and exact pending
  apply complete-source evidence and published inventory delta
  current = target
  add retired-generation cleanup row if needed
  remove pending
commit
open and verify target read snapshot
install snapshot-backed PublishedView under a short manager lock
report visible
```

No RPC, object I/O or corpus-sized encoding occurs inside the writer. Do not start another publication before the
matching snapshot is installed. Resolve uncertain commit from current/pending rows.

## Initial build, rebuild and compaction

A fresh or incompatible profile uses a new physical generation and separately captured bounded input pages.
Staging remains unqueryable. Complete Qdrant effects and evidence precede the atomic pointer switch. Source changes
during the build are follow-up reconciliation.

Use the same path for first build, lost/corrupt collection rebuild, incompatible schema/profile and explicit
compaction. Generation names are installation-scoped and never reused, including failed targets.

## View lifetime and whole-generation cleanup

`PublishedView` owns a real redb snapshot plus generation/sequence. One strong current `Arc` and weak retired
references are sufficient. Queries and windows hold guards; no manager lock spans I/O.

A `RetiredCleanup` row names one whole generation. It becomes eligible only when:

1. the generation is not current and no future/pending publication can target it;
2. no live `PublishedView` or `QueryWindow` guard uses it;
3. the cleanup owner can execute the exact collection operation under a finite context.

Then:

```text
delete exact collection with terminal/wait semantics
→ verify exact logical collection absence through the selected exists/get operation
→ remove cleanup row and obsolete generation metadata
```

A collection name is never reused. Point APIs are not collection-creation APIs, but this project must verify that
race against the pinned Qdrant 1.19 server before claiming runtime safety. If deletion result, exact absence or
backend state is uncertain, retain the cleanup row and apply bounded publication backpressure. A collection
missing from a list is not sufficient evidence, and logical absence is not a claim that backend files were
physically erased.

Last guard may wake cleanup but never deletes inline. Local immutable objects are separate stopped-root
maintenance.

## Restart and restore

Restart opens durable current, resumes exact pending pages, installs the current snapshot and reevaluates retired
generations. It never rescans live paths as an old pending operation's input.

Restore/import creates a fresh installation and generation. Copied pending/cleanup records are archive evidence,
not authority over the original endpoint.

## Remove with callers

Delete competing phase/receipt/compensation machines, full-manifest proof encodings, raw pin registry, per-point
reclaimer, bridge replay ledger and exclusive fixtures.

## Focused checks after code

- published-only removal after lost notification;
- successful empty and gap-only replacement;
- explicit `insert_only` request on pinned client/server;
- identical insert retry after lost response;
- foreign point conflict;
- fixed non-creating closure retry and missing-point refusal;
- old view remains exact after closure at a later sequence;
- atomic commit/snapshot-install gap;
- restart from pending pages;
- rebuild/compaction and never-reused generation;
- no new dispatch to retired generation;
- collection delete/absence race with delayed point attempt;
- cleanup row retained on any uncertainty;
- bounded retired-generation backpressure;
- no claim of physical erasure from logical absence.
