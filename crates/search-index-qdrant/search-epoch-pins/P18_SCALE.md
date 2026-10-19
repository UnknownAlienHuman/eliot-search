# Historical P18 route-pin programme

The old P18 design added migration-route pins, fences, drain snapshots, reclaimability values and owner-epoch
invalidation. It is superseded by the ordinary published-view manager and physical-generation publication.

A topology/profile change builds a new physical generation, verifies it and commits a new `PublishedView`.
New requests then pin only that view. Existing requests/windows retain their old view through private Arc
ownership. Publication persists one `RetiredCleanup` record for the old collection.

Cleanup checks the live view manager directly:

```text
delete old collection iff
  no live PublishedView uses its route/generation
```

No migration pin, route fence, drain snapshot, watermark, rollback-protection pin or owner epoch is needed.
A later rollback is another forward publication. Restart drops old interactive guards while the durable
cleanup record remains.

For many rapid publications, use a finite cleanup backlog and backpressure new publication before external
work when full. Do not create a scale-specific pin service. Qdrant owns shard/replica transfer; Eliot owns
only generation creation, committed route selection and eventual exact collection deletion.

Remove the P18 APIs/tests with the raw pin registry. Current requirements are in [FUNCTIONS.md](FUNCTIONS.md)
and [`search-publication`](../search-publication/FUNCTIONS.md).
