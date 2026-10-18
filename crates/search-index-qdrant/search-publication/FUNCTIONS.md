# Publication — current API and cutover map

The [implementation guide](../../../docs/implementation/README.md) selects one durable publication owner.
This file records current source behavior and the direct replacement; existing phase/receipt signatures are
not compatibility requirements.

## Caller reality

| Current source/API | Actual behavior / caller status |
|---|---|
| `validate_prepared_generation` | Reconstructs/checks complete manifests, UnitSet scope and diff, then builds private `PreparedGeneration`. It performs no persistence or external I/O. |
| `PublicationCoordinator::{submit,persist_intent,stage_new_points,close_old_points,verify_readback,commit_visible_epoch,publish_control_snapshot,complete}` | Mutates one in-memory phase slot after comparing supplied receipts/observations. Search finds package/test callers, not daemon serving. |
| `restore_inflight` / `recover` / abort-compensation methods | Rehydrate or classify supplied values; do not load a real operation artifact, inspect Qdrant or write control state. |
| daemon `publication_composition::Publisher` | Separate file-journal/floor/CAS/compensation machine. Declared `#[allow(dead_code)]`; used by process/module-shape tests. |
| redb `persist_publication_intent` / visibility APIs | Typed durable code exists, but search finds current callers in its tests. Intent phases retain receipt-shaped fields rather than the actual bounded mutation cursor. |
| `PublishedView` | Target concept only; no application struct/current atomic pointer was found in public main. |

The current pending intent's `prepared_manifest_ref` cannot recover full old/new manifests or mutation input
because those remain in `PreparedGeneration` memory. A matching receipt string is not a resolver.

## Planner handoff

Projection supplies an ordered immutable page stream, not one giant plan:

```text
Page {
  retrieval point create/retain/retire state
  actual BM25/vector/payload mutation input
  ViewManifestEntry bindings
  expected Qdrant readback
}
```

Write/check each page once. Use merge walks over ordered old/new pages; do not build whole-corpus maps,
clone full manifests or embed both manifests into another 8 MiB digest preimage. Cancellation between local
pages discards the unpersisted preparation.

The planner performs no redb/Qdrant effects and creates no publication receipts.

## Persistent records

Use two small current-format records:

```text
CurrentPublishedView {
  route/generation/epoch/profile
  manifest roots/pages
  source/prepared root references
}

PendingPublication {
  operation artifact reference
  target view identity
  mutation/readback/visible/cleanup cursor
  state = local_persisted | qdrant_unresolved | ready_to_commit |
          visible | cleanup_pending
}
```

The operation artifact contains ordered manifest/mutation pages and exact expected readback. It exists before
`PendingPublication` references it. Completed operation-only pages are removed after visibility and cleanup;
manifest pages remain roots of the views that use them.

Do not retain every historical intent in one reserved slot. The current view record is current state; old
views survive through immutable pages and live guards, not an eternal phase log.

## One effect loop

Conceptually:

```text
while pending:
  load artifact page at cursor
  inspect actual control/Qdrant state when outcome is uncertain
  if mutation absent: dispatch exact page
  read back exact affected state
  persist cursor

when mutation pages complete:
  commit CurrentPublishedView + clear visibility-pending state
  swap atomic current pointer

when old guards drain:
  run exact cleanup pages
  remove pending/operation-only artifacts
```

Use the same operation/page identity for inspection and retry. A different input under the same identity is
conflict. No process-local bridge ledger is needed. A pre-dispatch validation/cancellation failure is known;
a timed-out RPC after dispatch remains unresolved.

## Visible view owner

Implement one owner holding an atomic/lock-free current immutable view reference and guard counts per view
object. Public APIs are narrow:

- `pin_current() -> PublishedViewGuard`;
- `swap_after_commit(new_view) -> old_view_cleanup_handle`;
- private clone/transfer from a live guard into a `QueryWindow`;
- final-drop notification to publication cleanup.

No arbitrary `route,epoch` pin acquisition, old-route lookup, renewal, public reclaimability boolean or
sampled watermark. Rebuild the current pointer from the committed redb record at startup before serving.

## Qdrant effects

Publication calls one daemon-owned bridge/runtime:

- complete point upsert for new/complete authoritative points;
- vector add/delete and payload merge/delete/overwrite for narrow changes;
- exact readback after each page;
- exact-ID delete or collection-generation delete during cleanup.

All calls use `wait=true` and the publication operation's absolute deadline/cancellation. After possible
dispatch, cancellation is unresolved, not automatic compensation. Publication decides recovery from actual
readback; the bridge does not remember receipts.

## Recovery matrix

| Durable/external observation | Resume action |
|---|---|
| No pending record | Serve committed current view. Unreferenced local object pages may be GC candidates. |
| Pending references missing/corrupt artifact | Block publication/serving of target; preserve current view and report corruption. |
| Page definitely not dispatched | Dispatch under the same page identity if deadline/work permits. |
| Page outcome unknown | Read exact Qdrant state before deciding to dispatch or advance. |
| Expected state present | Advance cursor without replay. |
| Conflicting state | Fail/repair explicitly; never declare success from a receipt. |
| All pages verified, view not committed | Commit current view in redb. |
| View committed, pointer absent/stale | Rebuild/swap pointer from redb; do not mutate Qdrant. |
| View visible, cleanup pending | Wait for old guards and continue exact cleanup. |

## Delete with the cutover

- `PublicationCoordinator` receipt/observation phase machine and restore/abort facades;
- daemon `publication_composition` and its `FileJournal`/floor/compensation/retirement models;
- redb phase transitions whose only fact is a caller-supplied acknowledgement;
- whole-manifest reconstruction/digest admission repeated between adjacent trusted owners;
- `search-index-reclaimer` plan/proof/checkpoint/receipt package;
- raw `search-epoch-pins` route/epoch registry and watermark APIs;
- module-shape/process fixtures that preserve those boundaries.

Keep only code that contributes to actual page validation, durable current/pending records, real Qdrant
readback, view guards and exact cleanup.

## Verification

Exercise the integrated daemon around every boundary:

- cancellation before pending persistence;
- restart after artifact, pending record, mutation dispatch, readback, visibility commit and pointer swap;
- acknowledgement loss with state absent/present/conflicting;
- old query and window surviving cutover;
- impossibility of new old-view acquisition;
- same-generation retired-ID deletion and new-generation collection deletion;
- artifact/source/prepared cleanup only after all roots disappear.

No package-only fixture or matching phase enum proves the application path.
