# Physical topology and scale

The old P18 advanced-scale supplement is not a product state machine or runtime qualification protocol.
The local baseline is one Qdrant node, one shard, one replica and one serialized publisher. Measure that
path before adding topology.

When a measured limit requires another topology, select it as collection-generation configuration:

```text
measured bottleneck
→ explicit shard/replica/resource profile
→ create new physical generation
→ ingest bounded publication pages
→ exact schema/query/mutation readback
→ CurrentPublishedView switch
→ old-view drain
→ delete old generation
```

Qdrant already owns shard distribution, replication, transfer and cluster consistency. The bridge translates
the selected API; Eliot does not need `QualifiedScaleProfile`, capability receipts, equivalence receipts,
R0/R1 catch-up or another migration coordinator.

A changed topology may change scoring/IDF behavior. Verify it against the actual product corpus/evaluation set
before view commit. If semantics differ materially, assign a new retrieval profile and build a new generation.
Do not pretend equality through a digest or historical probe report.

During a large build, baseline may serialize publication, replan against a newer current-source snapshot or
apply one bounded ordered delta before final readback. Add a durable change stream only after measurement
shows those simple choices insufficient.

Write ordering is selected by topology. Weak/default is sufficient for the one-node/one-writer baseline when
each publication page uses `wait=true`, exact readback and a separate committed-view switch. Medium/strong
ordering may be chosen for a distributed profile with a demonstrated need; it is not a universal schema
floor.

No scale-specific bridge APIs are required beyond create/verify/delete generation, ordinary bounded point
operations and query/readback. Publication remains the only owner of recovery and route visibility.
