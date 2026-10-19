# Publication — source audit and replacement map

The [implementation guide](../../../docs/implementation/README.md) selects one recoverable publisher. Existing
public structs, phase names, receipts and frozen wire bytes are inventory, not compatibility requirements.

## Current source inventory

The package has no `qdrant-client`, redb or daemon dependency. Its non-test Rust is roughly 102 KiB:

| Source | Current behavior |
|---|---|
| `admission.rs` | Reopens complete UnitSet/plans, constructs full unit/point maps, recomputes complete semantic digests and another digest over scopes, guards, both manifests and all create/retain/retire IDs. |
| `machine.rs` | Keeps one process-local slot with ten phases, complete old/new manifests and diff, stage/closure/readback/control/snapshot receipts and a one-million-point default. |
| `model.rs` | Defines caller-constructible receipts/observations and duplicates point-ID vectors already in manifests/diffs. |
| `recovery.rs` | Compares supplied Booleans and ID sets; performs no redb read, Qdrant inspection or effect. |
| `machine/restore.rs` | Requires the caller to reconstruct proposal, preparation, intent, every receipt and fresh observations before restoring the in-memory machine. |
| `machine/abort.rs` | Adds compensation/exclusion, guarded control commit, snapshot acknowledgement and release barrier; executes no real effect. |
| callers | Package tests and old documentation dominate; no connected daemon publisher was found. |

Two more disconnected machines exist:

- daemon `publication_composition::Publisher`: dead-code floor/file-journal/guard/compensation model used by
  fixtures;
- `search-control-redb`: intent, VisibleEpoch, successor, abort and snapshot-publisher codecs without the
  actual operation pages.

Do not synchronize the three systems. Replace them.

## Useful code to keep or move

- ordered old/new manifest merge and detection of changed retrieval state under a reused point ID;
- finite page/point/byte bounds;
- validation that exact readback matches the page actually sent;
- monotonic target view sequence and physical generation;
- stable point/evidence-binding validation;
- fail-closed conflict handling.

Scope, preparation and schema validation happen once at their handoff. Do not rebuild complete trusted
products during each publication step or after restart.

## Target durable records

```text
PublishedView {
  id: ViewId
  sequence: u64
  route: PhysicalRoute
  generation: CollectionGeneration
  profile: RetrievalProfileId
  manifest_root: ObjectRef
  evidence_roots: ObjectRef[]
}

PendingPublication {
  operation_artifact_ref: ObjectRef
  expected_current_view: Option<ViewId>
  target: PublishedView
  cursor: PublicationCursor
}

PublicationCursor {
  stage: apply | verify
  page: u32
  substep: observe | dispatch | readback
}

RetiredCleanup {
  id: CleanupId
  cleanup_artifact_ref: ObjectRef
  kind: PointIds | Generation
  route: PhysicalRoute
  generation: CollectionGeneration
  retire_before_sequence: u64
  cursor: CleanupCursor
}
```

The operation artifact contains ordered apply/readback pages plus the target manifest. Cleanup pages may be
separate immutable roots so apply-only pages can become unreferenced after view commit.

```text
PublicationPage {
  full/narrow point operations
  closures: { point_id, valid_until = target.sequence }[]
  ViewManifestEntry[]
  exact expected readback
}

CleanupPage {
  exact retired point IDs                  // same generation
  or old collection identity              // replaced generation
}
```

New points carry `valid_from = target.sequence`; closures set exclusive `valid_until = target.sequence`.
Unchanged points are not rewritten. A changed retrieval input/identity creates a new point; a changed
revision/range alone updates only the target view's evidence manifest.

redb contains no source bodies, vectors, batches or terminal history. `RetiredCleanup` rows are bounded
unfinished external effects, not completed publications.

## Target operations

### `prepare_operation(old_view, prepared_pages) -> ObjectRef`

Validate pages once, verify stable identity/evidence bindings, write immutable operation/manifest/cleanup
pages through the shared object store and return the operation root. No redb/Qdrant effect.

The artifact's exact old/new ordered merge determines closures and cleanup pages. Do not accept a separate
caller-produced retired manifest or publication proof.

### `start(operation_ref, expected_current_view)`

In one short redb transaction:

1. verify `CurrentPublishedView == expected_current_view`;
2. verify no conflicting pending publication;
3. insert `PendingPublication` at page zero.

An identical existing pending operation resumes. Different input conflicts. Apply backpressure before this
step when the finite retired-cleanup backlog is full and the operation will create another cleanup record.

### `step(pending, context)`

For the durable cursor page:

1. load the immutable page;
2. if a prior dispatch may have happened, inspect exact Qdrant state first;
3. dispatch only the missing full/narrow operation through the one Qdrant runtime with `wait=true`;
4. read back exact point/vector/payload/validity/schema state required by the page;
5. advance the cursor in a short redb transaction.

Use one absolute deadline/cancellation. Failure before dispatch is definite. Failure after possible dispatch
is unresolved and leaves the same cursor. The bridge keeps no replay ledger.

### `commit_view(pending, view_manager)`

After every apply page verifies:

1. acquire the view manager's short commit/swap lock, blocking new `pin_current()` calls;
2. open one redb write transaction;
3. compare the pending row and expected current view;
4. write target `CurrentPublishedView`;
5. insert one bounded `RetiredCleanup` when exact point or generation deletion remains;
6. clear `PendingPublication`;
7. commit redb;
8. replace the in-memory current `Arc` with the target and retain only a `Weak` reference to the old state;
9. release the manager lock and wake cleanup.

No object or Qdrant I/O occurs under this lock. Existing old guards continue. New requests after unlock can
only clone the target.

If redb commit acknowledgement is uncertain, resolve `CurrentPublishedView`, `PendingPublication` and the
expected cleanup row while new pins remain blocked. Install exactly the committed view or keep the old
pointer. A process crash reconstructs from redb.

Do **not** keep cleanup only in `PendingPublication`: clearing pending would lose deletion work after a crash.
Do **not** keep the whole pending operation forever: that is a history ledger. Transfer only unfinished
cleanup pages to `RetiredCleanup`.

### `pin_current() -> ViewGuard`

Baseline manager:

```text
ViewManagerState {
  current: Arc<PublishedViewState>
  retired: BTreeMap<ViewId, Weak<PublishedViewState>>
}
```

A short mutex is sufficient; no lock-free pointer or new crate is required. `pin_current()` locks, clones
`current`, unlocks. A live request can clone/transfer its guard to a `QueryWindow`. There is no method taking
a route, generation, sequence or owner ID.

The manager stores weak retired refs only. Last strong-drop sends a cleanup wake; periodic scanning handles
lost wakes. On restart only the durable current view is reconstructed; all old ephemeral guards are gone.

### `cleanup_eligible(record, live_views)`

Check all live views under the manager, not one sampled pin count.

For exact point cleanup closed at sequence `N`:

```text
eligible iff no live view has
  same route/generation
  and sequence < N
```

For whole-generation cleanup:

```text
eligible iff no live view uses that route/generation
```

This distinction is mandatory. If v1 and v2 see a point and v3 closes it, no v2 guard does not make deletion
safe while v1 remains live.

The manager may prune dead `Weak` entries and return strong view snapshots for a bounded GC/eligibility pass.
If a weak reference cannot upgrade, no new old strong guard can appear because old views are never acquired
by ID. Existing guards may clone; their existing strong count keeps the weak upgradeable.

### `cleanup_step(record, context)`

1. evaluate eligibility from the live manager;
2. load the next exact cleanup page;
3. inspect state first when prior dispatch may have occurred;
4. delete exact IDs on the same route/generation or delete the old collection;
5. read back exact absence;
6. advance the cleanup cursor in a short transaction;
7. remove the row and cleanup-only pages after final success.

Unknown deletion retains the row and retries. Partial/foreign state blocks advancement. Broad-filter deletion
is unavailable.

### `startup_recover()`

1. load/validate `CurrentPublishedView` and install it in the manager;
2. load one optional `PendingPublication` and resume its exact page;
3. load finite `RetiredCleanup` rows;
4. because old process guards did not survive, evaluate cleanup against current view only;
5. enqueue eligible cleanup and ordinary artifact GC.

Missing/corrupt pending or cleanup artifacts block the affected operation but do not erase it or invent
success. The committed current view remains serviceable when its own manifest is valid.

## Local object GC coordination

Publication artifacts and manifests are immutable objects. A producer holds the shared object-GC gate from
finalization through the first durable reference transaction. Bounded GC takes the exclusive gate, snapshots
current/pending/cleanup and live-view roots, traverses only known current-format manifests and deletes exact
unreferenced objects. This prevents GC from deleting a newly finalized/deduplicated object before its redb
reference commits.

The gate is process-local coordination, not a lease catalogue or sweep journal. Failed delete remains
visible to later enumeration.

## Recovery rules

| Observation | Action |
|---|---|
| no pending row | Serve committed current view; process cleanup rows and later GC unreferenced local pages. |
| missing/corrupt pending artifact | Keep current view, block new publication, report corruption. |
| page definitely absent | Dispatch same page. |
| page outcome unknown | Inspect exact Qdrant state. |
| expected page state present | Advance without replay. |
| conflicting state | Block target; never infer success from receipt text. |
| all pages verified, target not committed | Run `commit_view`. |
| target committed, pointer stale/absent | Rebuild/swap pointer from redb. |
| cleanup row exists | Wait for eligibility, inspect/delete/read back and advance same row. |
| delete uncertain | Retain row and inspect again. |

Once a Qdrant effect may exist, baseline forward recovery completes the same operation. Later source changes
become the next publication. No epoch rewind, generic compensation, membership-fence abandonment or
invalidation-only publication.

## Physical-generation replacement

Schema/profile/vector incompatibility uses a new generation and the same pending operation. Build from one
consistent current-source/preparation snapshot; baseline may serialize, replan or apply one bounded ordered
delta before commit. At commit insert a generation cleanup row. Old guards keep the old collection; cleanup
deletes it only when no live view uses that generation.

## Removal boundary

After direct callers switch, delete:

- current coordinator/phase/receipt/recovery/restore/abort public APIs;
- daemon publication composition and file/floor/compensation/retirement machinery;
- old redb publication intent/VisibleEpoch/successor/checkpoint/snapshot-publisher codecs;
- repeated whole-manifest canonical digest admission and duplicate point-ID copies;
- process-local bridge mutation receipts;
- reclaimer proof/plan/checkpoint package and raw epoch-pin registry;
- old P18/W7/process/module/source-shape fixtures.

Keep/move only ordered merge/page validation, exact effect/readback helpers, typed current/pending/cleanup
rows, view-manager guards and known-root GC inputs.

## Behavior to verify

- restart before/after artifact write, pending commit, each RPC/readback, current-view commit, cleanup-row
  insertion and pointer swap;
- uncertain redb commit cannot expose an uncommitted target or lose a committed one;
- v1/v2/v3 example prevents premature point deletion;
- no new old-view acquisition after swap;
- query-window transfer keeps exact old view;
- same-generation cleanup waits for every older sequence on that generation;
- generation cleanup waits for every view using it;
- crash after view commit retains cleanup obligation;
- cleanup backlog bound backpressures safely;
- uncertain delete leaks/retries and never broad-deletes;
- current/pending/cleanup/live roots survive object GC.

Run minimal Rust 1.98 compiler/Clippy for changed publication/control/bridge/view/query owners after code.
Broad fault matrices follow the connected path.
