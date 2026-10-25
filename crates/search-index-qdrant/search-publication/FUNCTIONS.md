# Publication — one pending operation, immutable points, atomic view

Publication is the only owner of Qdrant mutation, published source/evidence state, current `PublishedView` and
retired-generation cleanup. The bridge owns SDK translation; control owns redb tables/snapshots; preparation owns
current wanted outcomes.

## Baseline safety decision

The baseline never physically deletes individual Qdrant points.

New retrieval points are immutable and created with insert-only semantics. A retry uses the same fixed point ID,
retrieval input and payload. Replaced points receive one fixed exclusive `valid_until = target_sequence`;
repeating that same closure is allowed, reopening or changing the boundary is not.

Because closed points remain present, a delayed duplicate insert cannot recreate a deleted ID or overwrite the
closed point. This removes the previous isolated-route/possible-writer recovery system. Storage is compacted by
building a fresh never-reused generation and switching the current view; cleanup deletes whole retired
generations only.

## Reconcile desired and published state

Compare both sides under a consistent redb snapshot:

- current source/preparation desired state;
- current published source inventory.

Use bounded ordered pages; do not collect the corpus in memory. A published-only removed/excluded source is work
even without a new preparation event. Repeated identical state is a no-op.

| Desired state | Result |
|---|---|
| usable preparation, including zero units | publish exact prepared input/gaps; create new points; close replaced points |
| selected terminal unsupported/failure | publish gap-only outcome; close former points |
| confirmed removal/unregistration/exclusion | remove membership and close former points |
| preparation running or observation incomplete | preserve prior served revision as prior/stale and expose pending/gap; do not infer empty/removal |
| exact desired/published match | no publication |

Queue entries are acceleration only. Startup/explicit/periodic reconciliation must rediscover differences.

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
state becomes the next publication.

## Apply incremental pages

Use one absolute deadline/cancellation per attempt. Persist page progress only after exact observation.

### Create point

```text
insert-only exact immutable point
→ completed response or retry same fixed insert
→ exact readback of ID, immutable retrieval bindings, validity and generated-vector shape
```

A timeout/connection loss is unresolved for that attempt, but retrying the same insert-only value is safe because
the ID is not physically deleted inside the generation. If another attempt inserted it, exact readback accepts
the identical state; foreign/conflicting state stops publication.

### Close point

```text
set exact valid_until = target_sequence
→ completed response or repeat the same closure
→ exact readback
```

No operation may remove the closure or choose a different boundary.

Do not use full upsert as retry, compare raw Qdrant scores, or persist a generic bridge operation ledger.

## Atomic commit and visibility

After every required point/schema observation succeeds:

```text
one redb transaction:
  compare expected current and exact pending
  apply complete-source evidence and published inventory delta
  current = target
  add retired-generation cleanup rows if needed
  remove pending
commit
open and verify target read snapshot
install snapshot-backed PublishedView under a short manager lock
report visible
```

No RPC, object I/O or corpus-sized copy occurs inside the writer. Do not start the next publication before the
matching snapshot is installed. If commit outcome is uncertain, resolve from current/pending rows.

## Initial build, rebuild and compaction

A fresh or incompatible profile uses a new physical generation and separately captured bounded input pages.
Staging is unqueryable. Complete Qdrant effects and evidence precede the atomic pointer switch. Source changes
during the build are follow-up reconciliation.

Use the same path for:

- first build;
- lost/corrupt collection rebuild;
- incompatible schema/profile;
- explicit compaction when closed-point growth exceeds the selected threshold.

Generation names are installation-scoped and never reused, including failed targets.

## View lifetime and cleanup

`PublishedView` owns a real redb snapshot plus generation/sequence. One strong current `Arc` and weak retired
references are enough. Queries and windows hold guards; no manager lock spans I/O.

Baseline `RetiredCleanup` names whole generations only:

```text
generation no longer current
+ no live view/window uses it
→ delete exact collection with wait=true
→ verify collection absent
→ remove cleanup row and obsolete generation metadata
```

Never reuse the collection name. A delayed point mutation cannot recreate a deleted collection, and no current
view can reference it. Individual closed points remain until generation retirement; there is no per-point
cleanup journal.

Bound retained generations/cleanup rows. When the bound is reached, stop new publication visibly until cleanup or
operator action succeeds.

Local immutable objects are not deleted here; stopped-root maintenance owns them.

## Restart and restore

Restart opens durable current, resumes the exact pending pages, installs the current snapshot and reevaluates
retired generations. It does not rescan live paths as the input of an old pending publication.

Restore/import creates a fresh installation and fresh generation. Copied pending/cleanup records are archive
evidence, never authority over the original Qdrant endpoint.

## Remove with callers

Delete competing phase/receipt/compensation machines, full manifest proof encodings, raw pin registry, per-point
reclaimer, bridge replay ledger and fixtures that exist only for them.

## Focused checks after code

- published-only removal after lost notification;
- successful empty and gap-only replacement;
- identical insert retry after lost response;
- conflicting existing point refusal;
- fixed closure retry;
- atomic commit/snapshot-install gap;
- old view exact while new view publishes;
- restart from pending pages;
- rebuild/compaction and never-reused generation;
- whole-generation cleanup after last guard;
- bounded retained-generation backpressure.
