# search-retention — remove the lifecycle platform

This package is not the owner of the current product. Its three production files total about 82 KiB and
implement process-local leases, generic graph mark/sweep, multi-plane purge, restore/quarantine and broad
operation/receipt vocabularies. Normal ingestion/query does not need them.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md),
[published-view manager](../../search-index-qdrant/search-epoch-pins/FUNCTIONS.md) and
[publication cleanup](../../search-index-qdrant/search-publication/FUNCTIONS.md).

## Current lifetime owners

The current-format application already has all necessary roots:

- current source heads and current prepared references;
- `CurrentPublishedView` and its manifest/evidence roots;
- one optional `PendingPublication` operation artifact;
- finite `RetiredCleanup` artifacts/pages for real unfinished external deletes;
- live process-local `PublishedView` guards/`QueryWindow`s;
- explicit user-retained history only if that feature is actually selected.

No completed-operation history, generic lease list, tombstone catalogue, legal hold, backup disposition or
caller-provided object graph participates in ordinary local GC.

## Safe immutable-object GC

Object finalization precedes the redb reference which makes an object live. Without coordination, GC could
observe a newly finalized or deduplicated object as unreferenced and delete it before the source/view commit.
Use one small process-local shared/exclusive gate:

```text
producer:
  acquire shared GC gate
  → finalize/verify immutable object
  → commit its first typed redb reference
  → release

bounded GC:
  acquire exclusive GC gate
  → snapshot durable current/pending/cleanup roots in one redb read transaction
  → clone live view roots from the view manager
  → traverse only known current-format manifests
  → enumerate/delete one bounded exact unreferenced page
  → release
```

Reads need not take the gate. Staging names are not final objects and have separate bounded debris cleanup.
If root traversal, inventory or deletion is incomplete/uncertain, retain bytes and retry. Failed deletes
remain discoverable on the next enumeration; no sweep journal is needed.

Holding the exclusive gate only for a bounded page keeps the pause finite. Every producer which may reference
an already existing deduplicated object must hold the shared gate through its reference commit, not only
writers which created new bytes.

## Qdrant cleanup

Publication owns physical Qdrant lifetime through durable `RetiredCleanup` rows:

- exact point IDs wait until no live same-generation view has a sequence below their `valid_until`;
- an old collection waits until no live view uses its generation;
- crash drops old interactive guards but not the cleanup row;
- unknown deletion leaks and retries by exact readback.

This is separate from local immutable-object GC and still does not justify a generic retention service.

## Source removal, backup and restore

Source removal publishes a new view without the source. Old guards retain the exact old revision until they
finish; ordinary GC later removes unreferenced current-format artifacts. This is not legal/security purge and
cannot promise deletion from backups or physical media.

Baseline backup is offline: resolve/report pending publication, close redb/object writers, copy one coherent
current-format root for the same supported storage/secret profile and rebuild Qdrant after restore. No live
root cutover, cross-user key migration, re-encryption or old-format import.

## Donor interpretation

Lucene `SnapshotDeletionPolicy` demonstrates in-process snapshot lifetime and explicitly states that its
snapshot state is not crash-persistent. Eliot keeps query guards process-local and persists only real cleanup
obligations.

Tantivy's segment updater supplies its directory GC with the files useful to committed/in-flight metadata.
Eliot similarly derives a living set from typed current/pending/cleanup/live roots. It does not import a
second index or generic object graph service.

## Removal boundary

After active callers adopt the view/object model, delete:

- this crate's lease, sweep and purge code;
- `wave7-lifecycle`, daemon restore composition and lifecycle config/validation packets;
- legal-hold/quarantine/tombstone/backup-disposition fixtures;
- dormant feature wiring and forwarding types;
- old issues whose replacement is the finite view/publication/object-GC path.

Keep any useful exact object enumeration/delete helper in the immutable object owner. Do not replace this
package with another retention framework.

## Completion

Current/pending/cleanup/live roots protect their exact objects; a producer/GC race cannot delete an object
before its first reference commits; bounded GC eventually removes unreferenced current-format objects;
uncertain work leaks safely; and no ordinary ingestion/query path depends on the legacy lifecycle package.
