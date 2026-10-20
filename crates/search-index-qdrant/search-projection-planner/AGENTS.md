# Working on search-projection-planner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source/caller audit](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

Do not repair or version the current whole `ProjectionPlan`, canonical manifest, vector-digest proof,
revision-bound scoring identity or `PublicationAdmissionOwners` seam. Their production path is disconnected
and they cannot upload the data they claim to plan.

Change preparation, identity, bridge, publication and query together:

```text
one current prepared unit stream
→ stable point identity
→ actual native BM25 text/options or actual selected vectors
→ exact versioned evidence binding
→ bounded ordered operation pages
→ publication apply/readback
```

Keep scope constants once in the operation header. Diff stable point state separately from view evidence by
merge-walking sorted page iterators. Enforce page point/byte limits before allocation growth and checkpoint the
same absolute deadline/cancellation between pages. Never allocate both complete old/new manifests, construct a
generic canonical tree or hash values as a substitute for uploadable input.

The planner owns no redb/Qdrant effect, current view, receipt or recovery state. Remove fixture-only owner
traits, full reconstruction checks and pre-release codec goldens with their callers. If the surviving page
builder and merge are small, move them to preparation/publication and delete the package.

After code, run minimal Rust 1.98 check/strict Clippy for changed preparation/identity/planner/publication/
bridge/query consumers and focused page/identity/incremental cases. Measure peak page memory and actual Qdrant
writes, not only digest equality.
