# Publication — source audit and replacement map

The [implementation guide](../../../docs/implementation/README.md) selects one recoverable publisher. This
file maps current source to that implementation. Existing public structs and phase names are not
compatibility requirements.

## Current source inventory

The package has no `qdrant-client`, `redb` or daemon dependency. Its non-test Rust is roughly 102 KiB:

| Source | Current behavior |
|---|---|
| `admission.rs` | Reopens complete UnitSet/plans, constructs full unit/point `BTreeMap`s, recomputes complete semantic digests and builds another canonical digest containing scopes, guards, both manifests and create/retain/retire IDs. |
| `machine.rs` | Keeps a process-local single slot with ten phases. Stores complete old/new manifests and diff plus stage/closure/readback/control/snapshot receipts. `DEFAULT_MAX_PUBLICATION_POINTS` is one million. |
| `model.rs` | Defines caller-constructible receipts and recovery observations. Several values duplicate exact point-ID vectors already present in manifests/diffs. |
| `recovery.rs` | Compares supplied booleans and ID sets. It performs no redb read, Qdrant inspection or effect. |
| `machine/restore.rs` | Cannot restore from durable state. The caller must reconstruct the original proposal/preparation, intent, every acknowledged receipt, control observation and fresh effect observation. |
| `machine/abort.rs` | Adds compensation/exclusion, another guarded control commit, another snapshot acknowledgement and a final release barrier. It executes no effect. |
| package callers | Repository search finds package tests and old documentation, not a connected daemon product caller. |

Two more disconnected machines exist:

- daemon `publication_composition::Publisher`: dead-code floor/file-journal/guard/compensation model used by
  process/module tests;
- `search-control-redb`: durable intent, VisibleEpoch, successor, abort and snapshot-publisher state.

The daemon model persists an opaque marker but still has no Qdrant operation or `PublishedView`. The redb
model persists receipt-shaped phase data but not the actual operation pages. Synchronizing all three would
preserve the defect.

## Useful code to keep or move

- ordered old/new manifest merge and detection of changed data under a reused point ID;
- finite page/point/byte bounds;
- validation that exact readback matches the actual page sent;
- stable target view sequence and physical generation;
- fail-closed conflict handling.

Scope/UnitSet/schema validation belongs at preparation/planning handoff and runs once. Do not rebuild
complete trusted products at every publication phase or after restart.

## Target records

```text
PublishedView {
  id: ViewId
  sequence: u64
  route: PhysicalRoute
  profile: RetrievalProfileId
  manifest_root: ObjectRef
  evidence_roots: ObjectRef[]
}

PendingPublication {
  operation: ObjectRef
  expected_current_view: Option<ViewId>
  target: PublishedView
  cursor: PublicationCursor
}

PublicationCursor {
  stage: apply | verify | commit | cleanup
  page: u32
}
```

The immutable operation object owns ordered pages:

```text
PublicationPage {
  upserts: QdrantPoint[]
  closures: { point_id, valid_until = target.sequence }[]
  manifest_entries: ViewManifestEntry[]
  expected_readback: ExactPageExpectation
}
```

New upserts carry `valid_from = target.sequence`; unchanged points are not rewritten. A changed evidence
binding receives a new stable point identity. Exact old IDs to close/delete come from the old/new ordered
merge, not from caller-produced proof objects.

The concrete encoded types may differ, but there must be one artifact, one small pending row and one current
view row. Do not add separate stage, receipt, checkpoint, floor or snapshot tables.

## Target operations

### `prepare_operation(old_view, prepared_pages) -> ObjectRef`

Validate ordered pages once, verify stable identity/evidence bindings, write immutable pages through the
shared object store and return the exact operation root. No redb/Qdrant effect.

### `start(operation, expected_current_view)`

In one short redb transaction, verify the expected current view and absence/identity of the pending row,
then store `PendingPublication`. An identical existing pending operation resumes; different input conflicts.

### `step(pending, context)`

1. Load the page at the durable cursor.
2. If prior dispatch is uncertain, inspect exact relevant Qdrant state first.
3. Dispatch the absent full/narrow mutation through the one Qdrant runtime with `wait=true`.
4. Read back exact point/vector/payload/validity fields required by the page.
5. Advance the cursor in a short redb transaction.

Carry one absolute deadline and cancellation. Failure before dispatch is definite. Failure after possible
dispatch is unresolved and leaves the same cursor.

### `commit_view(pending)`

After every page is verified, compare/set `CurrentPublishedView` against `expected_current_view` and move the
cursor to cleanup in one transaction. Build/swap the in-memory current view from the committed record. A
crash after commit rebuilds the pointer; it does not rerun Qdrant.

### `pin_current() -> PublishedViewGuard`

The query owner exposes no arbitrary old route/sequence acquisition. A live request can privately transfer
a guard into its `QueryWindow`. Last old-guard drop schedules the matching cleanup cursor.

### `cleanup_step(pending, old_view)`

- same generation: exact bounded ID delete + absence readback;
- replaced generation: whole old collection delete + absence readback;
- operation-only/local artifacts: remove after current, pending and live roots no longer reference them.

Unknown deletion leaks safely and retries. Cleanup has no separate plan/proof/journal identity.

## Recovery rules

| Observation | Action |
|---|---|
| no pending row | Serve committed current view; unreferenced local pages are later GC candidates. |
| missing/corrupt pending artifact | Keep serving current view, block new publication and report corruption. |
| page definitely absent | Dispatch same page. |
| page outcome unknown | Inspect exact Qdrant state. |
| expected page state present | Advance without replay. |
| conflicting state | Block target; never infer success from receipt text. |
| all pages verified, target not committed | Commit current view. |
| target committed, pointer stale/absent | Rebuild/swap pointer from redb. |
| cleanup incomplete | Wait for guards/read back and continue same cursor. |

Once any Qdrant effect may exist, baseline forward recovery completes the same operation. Later source
changes become the next publication. Do not implement epoch rewind, generic compensation, membership-fence
abandonment or invalidation-only publication.

## Physical-generation replacement

Schema/profile/vector incompatibility creates a new physical generation using the same operation type. Build
from one consistent current-source snapshot. Before commit, either apply a bounded delta from newer current
heads or replan; baseline may serialize this work. After view commit, old guards retain the old collection,
then cleanup deletes that collection whole. No R0/R1 migration state machine is required.

## Delete after cutover

- all current coordinator/phase/receipt/recovery/restore/abort public APIs;
- daemon publication composition and `publication_codec::{FileJournal,PublicationFloor,cas}`;
- redb publication intent/VisibleEpoch/successor/checkpoint/snapshot-publisher codecs;
- whole-manifest canonical digest admission and duplicate million-ID vectors;
- reclaimer proof/permit/checkpoint package and raw epoch-pin registry;
- old P18/W7 state machines and source-shape/process fixtures.

## Verification

Use one connected daemon path. Exercise restart before/after each local commit and external dispatch/readback,
old-query survival, impossibility of new old-view acquisition, same/new-generation cleanup and conflicting
Qdrant state. Run minimal Rust 1.98 check/Clippy for changed production owners and actual consumers; broad
tests remain after the product path exists.
