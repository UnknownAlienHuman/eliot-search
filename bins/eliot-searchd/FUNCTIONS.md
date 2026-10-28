# Daemon composition — one application

The daemon owns invocation capture, construction of concrete owners, handler availability, cancellation/deadlines,
shutdown and one local dispatcher. Target composition below is not current runtime evidence.

<a id="inspected-callers-and-effects"></a>
## Current entrypoints and removal seams

At inspected head `45ddf887`, src/entry.rs orders independent interceptors before app::run_main:
legacy-effect refusal, direct registry, native-root lifecycle, migration, preparation, DIRECT service and secure
commands. The [immutable route audit](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/bins/eliot-searchd/README.md)
records which command is shadowed and why a compiled module is not a connected handler. Recheck actual callers
at the source cut; do not add another interceptor or enable all old wave features to mask this topology.

Useful source seams to move with their callers:

| Existing seam | Required cutover |
|---|---|
| owner_composition/kernel/native_source | Transfer the already acquired bytes to immutable retention; do not drop and reread them. |
| safe_reader_adapter | Preserve actual containment, cancellation/deadline and byte limits through the real reader. |
| native_root_command / registration_mutation / registration_native | Move useful verified-open/identity mechanics to the root owner; remove legacy custody/publication wrappers. |
| preparation_composition | Preserve usable/empty/gap distinctions, replace duplicate DIRECT store/profile/receipt paths. |
| public_runtime_service / provider composition | One current framed dispatcher; remove shadowed line/tab/hex, pairing/grant and duplicate session routes. |
| publication/rebuild/query proof modules | Connect actual owners and effects, not fixture-backed plans or supplied readback. |

<a id="query-publication-and-rebuild-boundaries"></a>
## Target data and effect path

```text
startup: capture invocation/settings once → current root + process-lifetime DataRootGuard
         → construct concrete stores/backend → recover durable current → install view/handlers
         → one framed dispatcher

ingest:  selected source → stable owned bytes → immutable ObjectRef → conditional SourceRecord
         → preparation outcome → fixed complete Qdrant build → evidence/current commit → view install

query:   typed request → pin current view → resolve scope → bounded TextQuery or retained Exact
         → pinned evidence lookup → retained verification → complete terminal response

shutdown: stop admission → cancel/drain finite work → drop windows/views → stop backend users
          → close stores → release root guard
```

Stage 2 supplies one file and one-shot output using final persisted owners. Stage 3A adds restart/inventory and
complete-generation convergence. Source-local incrementality is conditional on failed measured budgets.
[Implementation](../../docs/implementation/README.md#delivery-sequence) owns this order.

## Load only the affected rule

| Boundary | Canonical recipe |
|---|---|
| Settings | [Config](../../crates/search-config/FUNCTIONS.md) |
| Root and platform finalization | [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Immutable bodies/artifacts | [Object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Source/current state | [Source family](../../crates/search-source/README.md), [control](../../crates/search-control-redb/FUNCTIONS.md) |
| Qdrant schema/calls | [Lexical profile](../../crates/search-lexical/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Build/recovery/view/cleanup | [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query/evidence/window | [Query family](../../crates/search-query/README.md) |
| Framing and CLI | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../eliot-search/FUNCTIONS.md) |

Foundation/object code uses execution-only context; query constructs its own view/scope/scoring state. No redb
writer, view mutex or window mutation borrow spans external I/O. Preserve the
[lock order](../../docs/implementation/README.md#lifetime-and-lock-order). No copied algorithm or second runtime
ledger belongs in daemon composition.

## Removal and verification

Remove duplicate argv parsing/interception, source-registration publication, obsolete migration/secure/sealed
custody, registry replay/snapshots, duplicate writers, old wave dependencies, provider pairing/grants and separate
handle/continuation catalogues as their consumers move. Unsupported roots fail before writes and remain intact.

After production changes run minimal locked Rust 1.98 check/Clippy for the real binary and affected consumers.
Record executed root/file/backend/excerpt behavior separately from compilation. Historical detailed findings stay
in the [pre-R055 map](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/bins/eliot-searchd/FUNCTIONS.md)
and [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
