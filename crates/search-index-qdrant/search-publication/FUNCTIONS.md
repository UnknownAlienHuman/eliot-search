# Publication — complete generation first, measured incrementality later

Publication is the only owner of Qdrant mutation, published source/evidence state, current `PublishedView` and
retired-generation cleanup. The bridge translates SDK operations; control owns redb tables/snapshots;
preparation owns desired outcomes.

## Two explicit retrieval profiles

### Fresh-generation profile — walking skeleton and stage 3A

The first working and restart-convergent product publishes complete immutable physical collections:

```text
complete desired selected inventory
→ fresh never-reused physical collection
→ bounded immutable build pages with actual Qdrant inputs
→ explicit insert_only dispatch + exact readback
→ complete evidence/inventory snapshot
→ one redb current commit
→ open/verify matching read snapshot
→ explicit PublishedView install
```

Once visible, the collection receives no point mutation. A source/preparation/membership/profile change builds the
next generation. Cross-generation point IDs need not be stable and there is no `valid_until` closure in this
profile.

This is a valid production mode, not a temporary backend. It provides correctness, restart, old-view lifetime,
rebuild and cleanup before incremental point identity/mutation complexity.

### Incremental profile — stage 3B

Add only after full-generation build cost is measured and materially excessive. It keeps the same retained
objects, evidence rows, view manager and cleanup owner, but uses an explicit new retrieval profile/generation.

- unambiguous occurrence reuse supplies stable identity within one physical generation;
- new immutable point uses explicit `insert_only`;
- retry repeats identical fixed ID/input/payload and verifies exact readback;
- replacement writes one fixed non-creating exclusive `valid_until = target_sequence` and verifies it;
- old sequences remain eligible below the boundary, so closure does not wait for old readers;
- individual point IDs are never physically deleted;
- compaction/profile/schema change builds a fresh never-reused generation.

Default upsert is forbidden. A missing-point closure must not create a point. Do not add the incremental profile
if measured gains do not justify its identity/recovery surface.

## Reconcile desired and published state

Compare under a consistent redb snapshot:

- current selected source/preparation desired state;
- current published source inventory.

Use bounded ordered pages; do not collect the corpus in memory. Queue entries only accelerate discovery. Startup,
explicit and periodic reconciliation must rediscover differences.

| Desired state | Fresh-generation action |
|---|---|
| Exact desired/published match | no publication |
| Usable preparation, including zero units | include exact result/gaps in next complete generation |
| Selected terminal unsupported/failure | include gap-only selected outcome |
| Confirmed removal/unregistration/exclusion | omit membership only after complete inventory proof |
| Preparation running or observation incomplete | preserve prior served view and expose pending/gap; do not infer empty/removal |

For stage 3A, any real difference builds a complete next generation. Stage 3B may instead calculate a bounded
complete-source delta.

## Fixed build artifact

Under one serialized publisher:

```text
pin expected current view and desired-state snapshot
→ allocate fresh installation-scoped generation name
→ capture bounded immutable pages containing actual Qdrant inputs
→ capture complete matching evidence/inventory rows
→ validate page/row/byte ceilings
→ persist one PendingGenerationBuild referencing those exact pages
```

Retries never reread live paths or splice newer state into the build. Newer desired state becomes a following
publication.

## Apply build pages

Use one absolute attempt deadline/cancellation. For each point:

```text
explicit insert_only fixed point
→ completed response or repeat identical insert
→ exact readback of ID, retrieval bindings, profile/filter/group fields and vector/Document shape
```

A timeout/lost response is unresolved for that attempt. Exact identical readback settles it; foreign/conflicting
state fails the generation. A failed/uncertain generation is never served and its name is never reused.

No Qdrant alias is switched. The build targets the exact physical collection which the future `PublishedView`
will name.

## Atomic metadata commit and serving visibility

After every required build/schema observation succeeds:

```text
bounded redb transaction:
  compare expected current + exact pending build
  install complete evidence/inventory rows
  set durable current physical generation/profile
  add retired-generation cleanup row for predecessor
  remove pending build
commit
→ open/verify target read snapshot
→ short view-manager install of matching PublishedView
→ report visible
```

No RPC, object I/O or corpus-sized encoding occurs inside the writer. Do not start another publication before
view installation succeeds or recovery refuses serving.

This follows the useful Tantivy distinction: durable commit and serving-reader visibility are separate events.
ELIOT makes the second event explicit instead of relying on delayed automatic reload.

If redb commit outcome is uncertain, resolve from current/pending rows. If commit succeeded but process died before
view installation, restart opens durable current and installs its matching snapshot before serving.

## Initial build, restart and lost index

The same complete-generation path owns:

- first build;
- restart convergence after desired-state change;
- lost/corrupt collection rebuild;
- incompatible retrieval profile/schema;
- stage-3B compaction.

Source changes during a build become following reconciliation. Never mutate a completed old generation to
pretend it contains the newer desired state.

## Physical collection identity, not aliases

`PublishedView` stores the exact physical collection. Query addresses it directly. A Qdrant alias switch cannot
atomically install ELIOT's redb snapshot and cannot preserve old pinned views, so aliases are not serving,
recovery or cleanup truth. Do not add an alias manager to baseline.

## View lifetime and whole-generation cleanup

One strong current `Arc` plus weak retired view references is sufficient. Queries/windows retain guards; no
manager lock spans I/O.

A `RetiredCleanup` row names an exact never-reused generation. Cleanup starts only when:

1. generation is not durable current;
2. no pending/current/future publication can target it;
3. no live `PublishedView`/`QueryWindow` guard uses it;
4. no unresolved build/mutation attempt owned by this process remains targetable to it.

Then:

```text
terminal delete exact collection
→ query exact collection identity through exists/get
→ observe logical absence
→ remove cleanup row and obsolete metadata
```

Last guard drop only wakes cleanup. Never reuse the collection name. Deletion/absence uncertainty retains the row
and bounded publication backpressure. Logical absence is not physical-disk-erasure evidence.

The delayed request versus collection-delete behavior remains a required live Qdrant 1.19 case. Fresh-generation
publication confines that risk to a never-served or retired physical name; it does not justify a permanent
possible-writer ledger.

Local immutable objects are not deleted here; stopped-root maintenance owns them.

## Restore/import

Import creates a fresh installation and physical generation. Copied pending/cleanup rows and aliases are archive
evidence only and authorize no operation against the original endpoint.

## Remove with callers

Delete competing phase/receipt/compensation machines, alias-as-authority paths, full manifest proof encodings, raw
pin registry, per-point reclaimer, bridge replay ledger and fixtures which exist only for those systems.

## Focused evidence after code

### Walking skeleton / 3A

- complete one-file fresh build and exact retained excerpt;
- incomplete/foreign point readback refuses visibility;
- redb commit then explicit view install;
- restart between commit/install;
- source edit/removal builds new complete generation;
- old pinned view remains exact;
- lost notification repaired from desired/published comparison;
- interrupted build never becomes current;
- cleanup after last guard and uncertainty retention.

### 3B only if selected

- identical insert retry after lost response;
- missing-point closure refusal;
- fixed closure while older view lives;
- ambiguous duplicate occurrence and conservative reminting;
- measured rebuild/write reduction versus 3A;
- compaction threshold and generation switch.

Run minimal Rust 1.98 check/Clippy for changed publication/bridge/control/query consumers before live Qdrant cases.