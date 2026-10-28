# Data and lifetime graph

Navigation only: [implementation](../implementation/README.md#owners) maps concrete owners; their recipes own
algorithms. These responsibilities do not prescribe a crate count.

```text
Invocation / settings
  → DataRootGuard ── lifetime ──→ control + object owners
  → stable acquired bytes → ObjectRef → conditional current SourceRecord
  → materialize/unitize → PreparedRef/outcome
  → fixed complete generation input → Qdrant build/readback
  → evidence/current redb commit → matching snapshot → PublishedView install
  → scoped Search / retained Exact → pinned evidence → verified excerpt → client
```

Qdrant stores derived retrieval state. Immutable objects store evidence. Redb stores compact current bindings and
matching read snapshots, not a second searchable corpus.

## Context

Root, source, object, preparation and publication use operation-specific execution state. Query additionally owns
view/scope/scoring. Lower-level I/O borrows the same deadline/cancellation/counters without a reset; it never needs
a fabricated query view. See [cross-owner values](../implementation/README.md#minimal-cross-owner-values).

## Platform finalization

Root and object writers share the root owner's small private
[platform seam](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#windows-finalization).
That candidate is not an executed Windows guarantee. Do not duplicate its algorithm or add another filesystem owner.

## Publication and lifetime

Stage 2/3A selects complete immutable generations; Stage 3B adds source-local mutation only after failed measured
budgets. Both use whole-generation cleanup, not physical point deletion. Query pins the physical collection and
matching evidence snapshot. A live window may retain that guard; last drop wakes cleanup without doing I/O.

The publisher's [cleanup rule](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup)
requires all relevant guards and targetable attempts to be gone. Reader drain alone is not proof of writer quiescence.
Local-object collection is separate stopped-root maintenance.

## Lock order

```text
process-lifetime root guard → construct stores
publication: fixed input → external I/O → bounded redb commit → matching snapshot → short view install
query: short current-view clone → scope/backend/evidence I/O → complete output → window/cursor commit
shutdown: stop work → close stores → drop root guard
```

No redb writer, view-manager mutex or window mutation borrow spans external I/O. The
[implementation lock rule](../implementation/README.md#lifetime-and-lock-order) is canonical.

## Optional leaves

Encryption, watcher, regex, PDF, MCP, models, overlay, managed Qdrant and syntax intelligence join an actual
source/preparation/query consumer. None creates a parallel store, authority or session system. Preserve private
#598 work and use only its accepted public handoff. Remove displaced routes and exclusive fixtures with callers.
