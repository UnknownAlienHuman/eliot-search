# search-index-qdrant

**Cells C13 to C17** — Projection Planner, Point Identity, Index Bridge, Publication Coordinator,
Pin and Reclaimer.

- **Owns:** projection plans and manifests; collision-safe point identity; vendor transport and
  capability probes; the epoch publication state machine; in-memory epoch and route pins.
- **Must not own:** project semantics, query interpretation, source truth, proof denominators.

Qdrant is the only search and index database, and it stores rebuildable projections only. Publication
is linearized in the control journal; an index alias is never the commit point.
