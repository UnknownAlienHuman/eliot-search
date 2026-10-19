# search-retention source map and removal plan

## What exists

| Source | Actual behavior |
|---|---|
| `src/lib.rs` (38,059 bytes) | Broad error/policy vocabulary, process-local lease/operation state, restore/reclaim concepts and large default capacities. |
| `src/sweep.rs` (30,401 bytes) | Accepts caller-supplied roots, pins, leases, graph and inventory; constructs protection/mark/sweep sets with custom digest/operation machinery. No production object owner supplies a complete authoritative graph. |
| `src/purge.rs` (13,605 bytes) | General multi-plane purge coordinator with caller authority, receipts, backup disposition and tombstones. It is not ordinary source removal. |
| daemon `restore_composition` | Behind `wave7-lifecycle`, marked dead code and exercised mainly by process fixtures; adds another phase/count/digest coordinator rather than normal startup. |

The crate is about 82 KiB before tests/config/tooling. The source defects in old issues are reasons not to
use these APIs, not reasons to harden the platform.

## Current lifetime data

Ordinary local lifetime is determined by typed records already required by the product:

```text
current source heads / prepared refs
CurrentPublishedView
PendingPublication?
RetiredCleanup[]
live PublishedView guards / QueryWindows
optional explicitly retained history
```

`ViewManifestEntry` and prepared/publication manifests contain the exact current-format references needed to
reach source revisions, prepared artifacts and retrieval data. Do not accept an injected generic graph,
caller lease list or proof digest as deletion authority.

## Process-local object GC gate

### Race being prevented

Current object ordering is intentionally:

```text
final immutable object exists
→ short redb transaction records first durable reference
```

A concurrent GC which snapshots roots between those steps can delete the object. Deduplication makes the
same race possible even when the writer reuses an old unreferenced object instead of creating new bytes.

### Minimal coordination

Use one process-local read/write gate owned by the immutable object/control composition:

```text
ObjectGcGate {
  inner: RwLock<()>
}
```

Every producer acquires a shared/read guard **before final publication/reuse of the final object name** and
keeps it through the first redb transaction which references the object. This applies to:

- source revision objects and source-head commit;
- prepared artifacts and current prepared-ref commit;
- publication operation/manifest pages and pending row;
- cleanup pages and `RetiredCleanup` insertion;
- any explicit user-retained-history reference.

A producer failure before reference commit leaves an unreferenced object. After releasing the shared guard,
bounded GC may remove it.

GC acquires the exclusive/write guard for one bounded page:

1. open one consistent redb read transaction;
2. collect current source/prepared refs, current view, optional pending and finite cleanup roots;
3. under the view-manager lock, clone strong snapshots of current and every live retired view;
4. traverse only known current-format manifest/object references under finite item/byte/depth ceilings;
5. enumerate one bounded page of final objects;
6. delete exact unmarked candidates;
7. release the gate and yield before the next page.

The strong view snapshot keeps old manifest roots alive through the pass. New producers cannot finalize/reuse
and reference an object while the exclusive guard is held. Reads may continue because every live/current
object is marked.

If any root/manifest is missing, corrupt, unsupported or exceeds traversal bounds, do not delete affected
objects. If inventory is incomplete or delete outcome is uncertain, retain and retry. A failed delete remains
visible in the next enumeration, so no sweep journal/checkpoint is required.

### Staging debris

Staging names are outside the final content-addressed namespace and never treated as roots. Clean only stale
staging files which are not held by an active writer, using bounded age/owner rules selected by the object
store. Success removes its stage immediately. Staging cleanup cannot delete final objects.

## Qdrant lifetime

Qdrant is derived external state and uses publication's durable cleanup queue, not object GC:

```text
RetiredCleanup::PointIds {
  route, generation, retire_before_sequence, exact pages, cursor
}

RetiredCleanup::Generation {
  route, generation, cursor
}
```

Point cleanup is safe only when no live same-route/generation view has sequence below
`retire_before_sequence`. Generation cleanup is safe only when no live view uses it. Crash removes old query
guards but cleanup rows survive. Unknown deletion retains the same row/cursor and observes actual Qdrant
state again.

## Source removal

Removing a source changes current source state and publishes a new view which excludes it. Old view guards
continue to reference exact old revisions. Only after publication cleanup and object-root analysis say the
objects are unreachable may they be deleted. There is no global deny, tombstone, legal hold, backup
instruction or secure-erasure receipt in the baseline.

## Backup/restore

Offline current-format backup:

1. stop new work;
2. finish/report pending publication and preserve cleanup rows;
3. close redb/object writers;
4. copy one coherent data root for the same supported storage/secret profile;
5. restore by opening the copy with the current application;
6. rebuild Qdrant and resume durable cleanup.

No live root switch, paired Qdrant snapshot, compatibility reader, cross-user secret migration or generic
restore quarantine.

## Donor mechanics

### Lucene

`SnapshotDeletionPolicy` holds an in-memory refcount per commit generation and suppresses deletion while a
snapshot is held. Its documentation explicitly says the snapshot state is not persisted. Use this only as
the process-local view-guard rule; durable cleanup obligations remain typed Eliot records.

### Tantivy

The segment updater calls directory GC with a closure returning useful committed/in-flight files after
metadata save. Eliot likewise computes a living set from current/pending/cleanup/live typed roots. Failed or
uncertain deletion remains for later discovery. Do not import Tantivy's segment system or persisted managed-
file registry.

## Code removal sequence

1. Implement the current-view manager and finite `RetiredCleanup` table.
2. Move exact Qdrant delete/readback into publication/bridge.
3. Add `ObjectGcGate` around finalization→reference and bounded root-snapshot→delete.
4. Connect known current-format object enumeration/traversal.
5. Add the small offline backup/startup path if selected.
6. Remove `search-retention`, `wave7-lifecycle`, restore composition and lifecycle configs/fixtures/tools.
7. Remove dormant features and forwarding types; do not keep a future-compatibility shell.

## Verification

- object finalized/reused while GC waits, then first reference commits safely;
- crash before reference leaves an object eventually collected;
- GC root snapshot protects current/pending/cleanup/live views;
- concurrent old view drop may retain extra data but cannot cause early deletion;
- malformed/incomplete manifest blocks deletion;
- bounded pages yield between batches;
- failed delete remains discoverable;
- source removal waits for old view evidence;
- Qdrant cleanup threshold/restart behavior is correct;
- no ordinary code path constructs lease/purge/tombstone/restore records.
