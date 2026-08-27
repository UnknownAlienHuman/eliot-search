# search-source

**Cells C03 to C07** — Source Registry, Source Identity, Change Reconciler, Safe Reader, Revision Store.

- **Owns:** roots and path bindings; physical and logical source identity with path history; watcher
  and inventory reconciliation; stable no-execute reads; immutable retained revision bytes and manifests.
- **Must not own:** access decisions, corpus policy, ranking, search queries.

Paths are locators, not identity. Watchers are hints: completeness comes from reconciliation, and an
unresolved observation gap is reported rather than presented as a current view.
