# Ports — replace the catalogue with concrete consumers

This inherited global abstraction catalogue is a deletion target, not the current application boundary.
The [pre-R055 audit](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/crates/search-ports/FUNCTIONS.md)
retains the exact public-main trait/caller census. Recheck current callers at each actual code cut; do not recreate
unused implementations to satisfy the catalogue.

## Concrete replacements

| Old family | Actual owner |
|---|---|
| journal/snapshot | typed current redb methods and real read transactions |
| inventory/ownership | current source rows and authoritative bounded inventory |
| safe read/revision | stable acquisition and one immutable-object reader/writer |
| materialize/unitize | direct transformations of the owned retained product |
| lexical/index/admin | one Qdrant scoring-text/TextQuery bridge; no local aliases or sparse BM25 inference |
| pins/access | pinned PublishedView and direct scope resolution |
| handles/continuation | one process-local QueryWindow |
| secrets/process/models/overlay | selected concrete owner only when a real consumer needs it |

A trait belongs beside a consumer that really needs multiple implementations. Use concrete current values and
actual errors. Do not introduce ports-v2, a service locator or a forwarding package.

<a id="why-the-common-context-is-wrong"></a>
## Execution versus query state

Follow the [cross-owner values](../../docs/implementation/README.md#minimal-cross-owner-values):

```text
ExecutionContext = original absolute deadline + cancellation + operation-specific remaining counters
RequestContext   = query ExecutionContext + pinned view + resolved scope; Search adds scoring
```

Root/bootstrap/object/preparation/publication operations do not construct a dummy query context. A query borrows
its existing execution state into lower-level I/O. Nested calls never renew time or reset charged counters.
Small private values/parameters suffice; do not build a shared context registry or universal budget schema.

## Effect resolution

Redb uncertainty reads the affected typed rows; object publication verifies the exact final object; Qdrant
uncertainty follows fixed pending input and dispatch observations owned by publication. Query and pure transforms
create no durable receipt. Process handling uses the actual owned handle.

A matching readback does not prove all possible external attempts stopped. Keep only bounded unfinished state
where the effect can outlive the process, not a completed-operation archive.

## Deletion order

Move control callers from generic journal/snapshot; source callers from registry replay; acquisition/preparation
from generic ports; publication/query to the concrete bridge/view/window; then protocol and optional consumers.
Delete each unconsumed trait, fake/conformance catalogue and dependency edge with its last real caller. Preserve
useful behavior, not trait counts or pre-release wire/shape fixtures.

After production changes run minimal locked Rust 1.98 check/Clippy for affected owners and immediate consumers.
Connected application behavior, not a package-local fake, establishes the replacement boundary.
