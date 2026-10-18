# search-retention source map and retirement

## What exists

| Source | Actual behavior |
|---|---|
| [`src/lib.rs`](src/lib.rs) | Defines a broad error/policy vocabulary, process-local lease/operation state and restore/reclaim concepts. The baseline policy alone reserves up to 65,536 lease records and 100,000 operation identities. |
| [`src/sweep.rs`](src/sweep.rs) | Accepts caller-supplied roots, pins, leases, graph and inventory; builds whole protection/mark/sweep structures with custom digest/operation machinery. No production object-store owner supplies a complete graph. |
| [`src/purge.rs`](src/purge.rs) | Implements a general multi-plane purge coordinator with caller-shaped authority, receipts, backup disposition and tombstone state. It is not connected to ordinary source removal. |
| daemon `restore_composition` | Feature-gated by `wave7-lifecycle`, marked dead code and used primarily by restore process fixtures. It adds another counts/digests/phase coordinator rather than a normal startup restore. |

The three crate source files total about 82 KiB; the listed restore kernel adds about 34 KiB before tests,
configs and validation tooling. These are gross source bytes, not a promised deletion count.

## Why not repair the platform

The local product already has the information needed for ordinary lifetime:

```text
current source heads
current published view
pending publication artifact
live query/window guards
exact manifest references
```

Adding durable lease types, legal holds, purge planes, backup dispositions, graph-completeness proofs and
replay journals would make every ordinary update depend on a subsystem that the application does not need.
The source defects recorded in old issues are valid reasons not to use these APIs, not reasons to rebuild
them more elaborately.

## Minimal lifetime model

### Roots

Keep exactly the data reachable from:

1. current source heads and settings;
2. the current published view;
3. one unresolved pending publication;
4. live process-local view guards/query windows;
5. explicit user-retained history only if that feature is actually selected.

A view manifest references the source revisions, prepared artifacts and retrieval points it serves. The
pending artifact references its bounded old/new pages and unresolved external effects.

### Reclamation

- Publication swaps the current view atomically.
- New queries can pin only the new view.
- Existing old-view guards drain monotonically.
- Same-generation retired points delete by exact IDs after the relevant guards drain.
- A replaced physical generation deletes as a whole after its last guard drains.
- Immutable source/preparation artifacts delete only when no current, pending or pinned manifest refers to
  them.
- Unknown delete/readback retains data and retries. No physical-erasure claim.

See [`search-epoch-pins`](../../search-index-qdrant/search-epoch-pins/FUNCTIONS.md) and
[`search-index-reclaimer`](../../search-index-qdrant/search-index-reclaimer/FUNCTIONS.md).

### Source removal

Removing a selected source publishes a new view that excludes it. Old queries keep their old exact
revision until their view closes. Ordinary GC then removes unreferenced points and derived artifacts.
This is not legal/security purge, cannot promise deletion from backups and needs no global deny fence.

### Backup and restore

Baseline backup is offline:

1. stop normal work and report/resolve any pending publication;
2. close redb and immutable-object writers;
3. copy one current-format data root for the same supported storage/secret profile;
4. restore by starting the current application on a copied empty destination/root selection;
5. rebuild Qdrant from retained source/preparation state.

There is no live root cutover, paired Qdrant snapshot, compatibility import, re-encryption or cross-user
migration. If protected storage's required OS secret is unavailable, the copied data is not restorable and
the application reports that limitation instead of constructing a migration framework.

## Donor mechanisms

- Lucene `SnapshotDeletionPolicy`: generation refcounts suppress deletion while a snapshot is held;
  release permits later unused-file cleanup.
- Tantivy segment updater: committed/in-flight metadata lists useful files; directory GC removes untracked
  files after commit.

Use the lifetime rules only. Qdrant remains the index and existing stores remain the artifact owners.

## Code removal sequence

1. Implement current-view guards and transfer continuations from a live query guard.
2. Move exact Qdrant delete/readback into publication/bridge cleanup.
3. Connect source/prepared-artifact GC to current/pending/pinned manifest references.
4. Add the small offline backup/startup restore if selected.
5. Remove `search-retention` from daemon features/dependencies.
6. Delete `wave7-lifecycle`, `restore_composition`, lifecycle tests/configs/tools and obsolete open tasks.
7. Remove this crate from the workspace after no real caller remains.

Do not keep forwarding types or a dormant feature for hypothetical future compatibility.
