# Publication — source audit and replacement map

The [implementation guide](../../../docs/implementation/README.md) selects one publisher. Current phase names,
receipts and frozen wire bytes are inventory, not compatibility requirements.

## Current source

The package has no redb/Qdrant/daemon dependency and roughly 102 KiB of non-test Rust.

| Source | Current behavior |
|---|---|
| `admission.rs` | Reopens complete UnitSet/plans, constructs whole maps and hashes both manifests plus all diff IDs again. |
| `machine.rs` | Keeps one process-local ten-phase slot with complete manifests/diff and repeated ID lists. |
| `model.rs` | Exposes caller-constructible receipts/observations rather than concrete external state. |
| `recovery.rs` | Compares supplied Booleans/ID sets; reads no redb/Qdrant owner. |
| `machine/restore.rs` | Requires the caller to reconstruct the original operation and every receipt before “recovery”. |
| daemon/redb alternatives | Two more disconnected publication/visibility machines. |

Keep only ordered diff/page validation, stable identity/evidence checks, finite limits and exact readback
matching. Remove the three state machines rather than synchronizing them.

## Immutable artifacts

```text
PublicationOperation {
  target PublishedView
  ordered PublicationPage[]
  manifest/evidence roots
  optional CleanupPage[]
}

PublicationPage {
  full/narrow Qdrant operations
  point closures { point_id, valid_until = target.sequence }
  ViewManifestEntry[]
  exact expected readback
}
```

Write/verify the artifact before inserting `PendingPublication`. Use ordered merge walks; do not build both
whole manifests and every ID copy into another canonical digest object.

## Durable records

```text
CurrentPublishedView { view, route, generation, profile, manifest/evidence roots }
PendingPublication { artifact_ref, expected_current, target, apply/readback cursor }
RetiredCleanup { cleanup_ref, kind, route, generation, retire_before_sequence, delete/readback cursor }
```

A cleanup row is current unfinished external state. It disappears after success. No completed publication
receipt/history remains.

## Operations

### `prepare(old_view, prepared_pages) -> ObjectRef`

Validate each bounded page once, write operation/manifest/cleanup objects through the shared object store and
return the root. No redb/Qdrant effect. A changed citation alone changes the target manifest, not the stable
point.

### `start(operation_ref, expected_current)`

In one short redb transaction, compare current, reject a conflicting pending operation and insert the pending
row at page zero. Identical pending input resumes. Check cleanup-backlog capacity before any new external
mutation which would create another cleanup obligation.

### `apply_step(context)`

For the durable page cursor:

1. load the immutable page;
2. if prior dispatch may have happened, inspect exact Qdrant state first;
3. dispatch only missing full/narrow work through the one bridge runtime with `wait=true`;
4. read back exact affected vectors/payload/validity/schema;
5. advance the cursor in a short redb transaction.

Use one absolute deadline/cancellation. Failure before dispatch is definite. Failure after possible dispatch
leaves the same cursor unresolved. The bridge owns no replay ledger.

### `commit_current()`

After every page verifies:

```text
redb transaction:
  compare CurrentPublishedView and PendingPublication
  write target CurrentPublishedView
  insert RetiredCleanup if physical deletion remains
  clear PendingPublication
→ commit
→ view_manager.install(target)
→ report publication visible
```

The redb transaction and pointer swap are deliberately not enclosed by one long mutex. `install` takes a
short manager lock, inserts `Weak(old)` and replaces the strong current `Arc`.

A request which pins old after redb commit but before `install` remains valid. It sees the old sequence and
manifest, and its guard delays cleanup. A crash in the gap installs the durable target on startup. Serving and
status use the manager pointer; redb is recovery state. Publication success is emitted only after install.

If redb commit acknowledgement is uncertain, inspect current/pending/cleanup typed rows before install. If
target committed, install it; otherwise keep old. A poisoned manager stops new serving and restarts from
redb; do not roll back the committed view.

### `pin_current() -> ViewGuard`

The manager uses a standard mutex:

```text
current: Arc<PublishedViewState>
retired: BTreeMap<ViewId, Weak<PublishedViewState>>
```

Lock, clone current, unlock. A live request can clone its guard into a window. No old-view lookup by ID,
route/epoch selector, owner ID, renewal or durable query lease.

### `live_views_snapshot()`

Under the manager lock, clone current, upgrade every retired weak ref and prune dead refs. Return strong
snapshots for one bounded cleanup/GC decision. Last `PublishedViewState` drop sends a best-effort wake;
periodic scanning is fallback. Never perform Qdrant/object deletion in `Drop`.

### Cleanup eligibility

Exact IDs closed at sequence `N`:

```text
eligible iff no live view has
  same route/generation
  and sequence < N
```

Whole generation:

```text
eligible iff no live view uses that route/generation
```

The immediate predecessor is insufficient. If v1 and v2 see P and v3 closes P, a live v1 still blocks P
delete after v2 is gone.

### `cleanup_step(record, context)`

1. snapshot live views and test eligibility;
2. load next exact cleanup page;
3. inspect actual state first when prior dispatch may have happened;
4. delete exact IDs or the old collection;
5. read back exact absence;
6. advance/remove the row in a short transaction;
7. release completed cleanup-only objects to ordinary GC.

Unknown delete keeps the same row/cursor. Partial/foreign state blocks advancement. Broad-filter delete is
unavailable.

### Startup

1. load/validate current view and install it;
2. resume optional pending publication;
3. enumerate finite cleanup rows;
4. old process guards are gone, so evaluate cleanup against current only;
5. enqueue eligible cleanup and bounded object GC.

Missing/corrupt operation or cleanup artifacts block that operation but do not erase it or invent success.

## Object-GC interaction

The single live root owner holds OS exclusion. Producers hold the shared object-GC guard from final object
publication/reuse through the first durable source/prepared/pending/cleanup reference. Bounded GC holds the
exclusive guard while snapshotting typed roots plus live views and deleting one exact unreferenced page.

Do not acquire the view-manager mutex while holding redb/object/Qdrant I/O. GC may briefly clone live-view
roots under the manager lock, then release it before manifest traversal/deletion.

## Remove

- coordinator phase/receipt/recovery/restore/abort APIs;
- daemon publication journal/floor/compensation stack;
- old redb intent/VisibleEpoch/successor/snapshot codecs;
- raw pin registry and reclaimer plan/checkpoint/receipt package;
- bridge mutation receipts;
- old W7/P18/process/module/source-shape fixtures.

## Focused behavior

- crash around artifact, pending, every RPC/readback, current commit and pointer install;
- old request pinned in the commit→install gap remains valid;
- uncertain current commit resolves before install;
- v1/v2/v3 prevents early point deletion;
- generation deletion waits for every guard;
- crash after current commit retains cleanup row;
- finite cleanup backlog backpressures safely;
- unknown deletion leaks/retries;
- current/pending/cleanup/live roots survive object GC.
