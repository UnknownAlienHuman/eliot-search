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
| secrets/pairing | Remove public wrappers with plain/local callers; preserve user bytes/keys and unpublished work. No new secret owner. |
| process/models/overlay | Concrete owner only for an explicitly selected real consumer |

Use concrete calls by default. A narrow consumer-local trait may isolate actual implementation variation or a
real dependency boundary; use existing library traits where they fit. Apply the
[implementation choices](../../docs/implementation/WORKING_GUIDE.md#implementation-choices), not a quota of
implementations or a mandatory trait per owner. Do not introduce ports-v2, a service locator or forwarding crates.

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

The [delivery sequence](../../docs/implementation/README.md#delivery-sequence) and real compile/runtime dependencies
choose the next cut, not the table above. Do not rebuild all control/source/port families before a root or CLI
change. Replace the selected path, remove each unconsumed trait/adapter and its exclusive fixtures with that cut,
and delete the workspace member after its final caller. Preserve useful behavior, not trait counts or old shapes.

After production changes run minimal locked Rust 1.98 check/Clippy for affected owners and immediate consumers.
Connected application behavior, not a package-local fake, establishes the replacement boundary.
