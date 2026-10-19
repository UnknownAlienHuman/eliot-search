# Working on search-publication

Read the root [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).
Change planner, typed redb state, Qdrant bridge, current-view owner and query lookup together.

Implement one external-effect owner:

```text
prepared pages
→ immutable operation artifact
→ PendingPublication
→ actual Qdrant mutation/readback
→ one transaction: CurrentPublishedView + RetiredCleanup - PendingPublication
→ in-memory current-view swap
→ eligibility-gated cleanup
```

Do not clear pending at the view commit without durably transferring unfinished deletion to
`RetiredCleanup`; a crash would forget old IDs/collection. Do not retain the whole completed publication
instead; cleanup rows contain only real unfinished external delete/readback and disappear after success.

Use one short view-manager lock around the redb current-view commit and pointer swap so no new request pins
the old view between those steps. Never hold that lock or a redb writer across object/Qdrant I/O.

Current-view guards are private `Arc` ownership. The manager keeps weak retired references. Existing guards
may clone into a `QueryWindow`; callers cannot acquire a route/generation/sequence by value. Cleanup checks
all live views:

- point IDs closed at sequence N wait until no live view on the same generation has sequence < N;
- an old collection waits until no live view uses that generation.

The immediate predecessor's last guard is not sufficient when an even older view is live.

Keep view-sequence validity, ordered pages, exact readback and forward recovery. Delete phase/receipt/
observation/restore/abort APIs, old redb visibility machines, raw pins and the separate reclaimer platform.
Unknown Qdrant deletion leaks safely and retries under the same cleanup cursor.

Coordinate immutable-object GC with a small shared/exclusive gate: producers hold shared protection from
object finalization through first durable reference; bounded GC holds exclusive protection while snapshotting
current/pending/cleanup/live roots and deleting exact unreferenced objects. Do not create a lease catalogue
or sweep journal.

After code, run minimal Rust 1.98 compiler/Clippy for changed production owners. Verify the connected daemon
across crash points, the v1/v2/v3 old-view case, cleanup backlog, uncertain deletion and object-GC races.
