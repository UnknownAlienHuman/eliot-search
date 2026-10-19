# Historical P18 reclaim programme

The old P18 design added committed retired-route manifests, drain snapshots, route reclaim plans, batch
receipts, checkpoints and rollback protection. It is superseded by physical-generation publication plus
finite `RetiredCleanup` rows.

A topology/profile change builds and verifies a new collection generation, then commits the new
`CurrentPublishedView` and one cleanup row for the old generation in the same redb transaction. Existing
queries/windows keep the old generation through their private view guards. New requests can acquire only the
new view.

Delete the old collection when the live-view manager reports that no view uses its route/generation. After
restart no old interactive guards exist, while the durable cleanup row remains and is reevaluated. Unknown
collection deletion is resolved by exact existence/schema observation with the same row/cursor.

No route manifest proof, drain watermark, rollback pin, scale-specific checkpoint or terminal reclaim
receipt is required. A later rollback is another forward publication. Qdrant owns shard/replica transfer;
Eliot owns generation creation, current route selection and eventual old-generation deletion.

Remove P18 APIs/tests with this package. Current rules are in [FUNCTIONS.md](FUNCTIONS.md) and
[`search-publication`](../search-publication/FUNCTIONS.md).
