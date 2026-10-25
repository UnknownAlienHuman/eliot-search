# Working on search-projection-planner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source/caller audit](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

The established audit found the whole `ProjectionPlan`, canonical manifest, vector-digest proof,
revision-bound scoring identity and `PublicationAdmissionOwners` production path disconnected at the inspected
revision. Treat that as revision-specific evidence. Before deletion or inlining, search current manifests,
Rust/non-Rust callers, constructors, module inclusion, features, targets, tests and workflows. Move every
surviving preparation/identity/publication/query consumer in the same cut.

Do not repair or version the proof protocol. Connect actual data instead:

```text
current prepared unit stream
→ stable point identity
→ actual native BM25 text/options or selected vectors
→ exact versioned evidence binding
→ bounded ordered operation pages
→ real publication apply/readback
```

Keep scope constants once in an operation header. Diff stable point state separately from view evidence by
merge-walking sorted page iterators. Enforce page point/byte limits before allocation growth and reuse the same
absolute deadline/cancellation between pages. Do not allocate complete old/new manifests, construct generic
canonical trees or hash values as a substitute for uploadable input.

The planner owns no redb/Qdrant effect, current view, receipt or recovery state. Remove fixture-only owner traits,
full reconstruction checks and pre-release codec goldens with their last callers. If the surviving page builder
and merge are small, move them into preparation/publication and delete the package.

After code, run minimal Rust 1.98 check/Clippy for changed preparation/identity/publication/bridge/query consumers.
Measure peak page memory and actual Qdrant writes/readback; digest equality alone is not delivery.
