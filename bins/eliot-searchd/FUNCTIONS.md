# Daemon composition — one application

The current binary owns native invocation capture, consumed root settings, root-command dispatch and factual
root output. It calls the concrete root/control owners. The source/Qdrant/query composition below is future work.

<a id="inspected-callers-and-effects"></a>
## Current entrypoints and removal seams

[src/entry.rs](src/entry.rs) calls [app::run_main](src/app.rs). One native argument capture enters
[app/bootstrap.rs](src/app/bootstrap.rs); parsed root input becomes RootInvocation, containing RootCommand
and a resolved PathBuf. [native_root_command::run](src/native_root_command.rs) calls exactly one root operation.

| Daemon command | Root owner |
|---|---|
| --inspect-data-root | inspect_plain |
| --initialize-data-root | initialize_plain |
| --check-existing-root-owner | open_plain |
| --discard-incomplete-data-root | discard_incomplete_plain |

Each operation receives (&Path, original absolute Instant deadline, &AtomicBool cancellation).
Initialization/open retain PlainRootGuard through output and then release it. Inspection reports observed
status; discard reports a subsequent inspection under the same command deadline. These are one-shot operations,
not a long-running search service. The [root recipe](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md)
owns layout, refusal and preservation rules.

Each command accepts an optional positional ROOT or --data-root ROOT, plus --config-file PATH.
Settings flags may precede or follow the daemon command. Repeated commands/options and positional-plus-flag
roots fail. Initialization takes no ID. Help/version and malformed command shape return before root effects;
standalone help/version does not load configuration or inspect the environment.

Root settings are startup-only. The daemon captures cwd once for a root invocation, reads at most 64 KiB plus
one sentinel byte from an explicit UTF-8 TOML file, and accepts only the optional data_root string.
The selected environment key is ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT. Effective precedence is file < environment
< CLI; a root must be supplied by one layer. Config-relative roots use the file parent; config-file, CLI and
environment relative paths use captured startup cwd. Native CLI/environment paths are not converted lossily.
Malformed lower layers, empty/NUL paths, unknown selected environment names, duplicate definitions and wrong
TOML types fail even if CLI would override them. No diagnostic_limit or generic --set field is consumed.

The response contains operation and root_state, or a bounded static error code. Current root/control format
does not establish source readiness, backend connectivity or search availability. Final integrity and
child-pipe changes require their own final-revision qualification.

The package has only the installed binary target. DIRECT registries, protected initializers, migration,
preparation, diagnostic scans, loopback services and their feature/harness routes are removed.
The [code map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace) names the surviving path.

<a id="query-publication-and-rebuild-boundaries"></a>
## Target data and effect path

The following is the later connected product, not current daemon behavior:

    startup: root guard → construct source/object/control/backend owners
             → recover durable current → install PublishedView/handlers → framed dispatcher

    ingest: selected source → stable owned bytes → immutable ObjectRef + guarded SourceRecord
            → preparation outcome → Qdrant build/readback → evidence/current commit → view install

    query: typed request → pinned view/scope → TextQuery or retained Exact
           → authoritative evidence lookup → retained verification → complete terminal response

    shutdown: stop admission → cancel/drain work → close windows/backend/stores → release root guard

Stage 2 connects one file to one CLI excerpt; Stage 3A adds restart/inventory and complete-generation
convergence. Source-local incrementality is conditional on failed measured budgets.
[Implementation](../../docs/implementation/README.md#delivery-sequence) owns that order.

## Load only the affected rule

| Boundary | Canonical recipe |
|---|---|
| Consumed settings | [Config](../../crates/search-config/FUNCTIONS.md); current parser is app/bootstrap.rs |
| Root and platform finalization | [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Current control format; future source metadata | [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Future immutable bodies/artifacts | [Object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Future acquisition/current rows | [Source family](../../crates/search-source/README.md) |
| Future preparation | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Future Qdrant schema/calls | [Lexical](../../crates/search-lexical/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Future publication/recovery/cleanup | [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Future query/evidence/windows | [Query family](../../crates/search-query/README.md) |
| Future framing/client | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../eliot-search/FUNCTIONS.md) |

Foundation/object code uses execution state only. Future queries add pinned view/scope/scoring state.
No writer or mutable view/window borrow spans external I/O; follow the
[lock order](../../docs/implementation/README.md#lifetime-and-lock-order).

## Removal and verification

Add the next concrete producer/consumer path directly; do not restore deleted custody, credential, migration
or command frameworks as scaffolding. Preserve user files and keys.

Minimal locked Rust 1.98 check/Clippy must select the actual daemon binary and affected owners.
Record final native root/control results at their exact source revision in the
[registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status). Root-stage and helper-native evidence does not
qualify an installed baseline, source ingestion, Qdrant publication or verified query output.
