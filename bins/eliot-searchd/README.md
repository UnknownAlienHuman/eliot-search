# eliot-searchd — current composition status

`eliot-searchd` is the intended composition root for the standalone local Search application. It is not yet one
application. R033 rechecked the current PR branch after R032; no daemon Rust or runtime behavior changed.

The statements below are source/caller observations, not a claim that the current binary compiled or ran.
Exact historical readsets remain in the [analysis registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-index).

## Current entrypoint chain

`src/entry.rs` still evaluates independent applications in order:

```text
1. secure_commands::maybe_refuse_legacy_effects
2. owner_composition::maybe_run_direct_registry
3. native_root_command::maybe_run
4. source_migration_command::maybe_run
5. preparation_composition::maybe_run
6. public_runtime_service::maybe_run
7. secure_commands::maybe_run
8. app::run_main
```

Each layer may recapture/parse argv and terminate the process. This is shadowed command routing, not one typed
dispatch table.

## Observed source routes

| Route in source | Current source-level disposition |
|---|---|
| `--scan-file`, `--scan-stdin` variants | Reach the final shell one-shot scanner when no earlier interceptor claims the invocation. The current R033 head has no daemon compile/runtime evidence, so documentation must not state that these commands currently work. |
| `--initialize-data-root`, inspect/owner commands | Reach the complex native pre-release root/owner lifecycle rather than the selected current-format root owner. |
| direct registry commands | Open/restore the legacy journal/registry/snapshot system; some registration/admission/DEK commands still have real effects. |
| `--serve-data-root`, `--search-root`, `--list-sources`, `--read-revision`, `--sync-source-roots` | Claimed first by the direct-registry selector and reduced to registry/root metadata paths before the later DIRECT service can handle them. |
| `--prepare-root`, `--prepare-revision` | Reach the legacy DIRECT store/profile/receipt preparation application. |
| control-migration commands | Reach a separate migration staging application. They are callable legacy routes, not test-only files. |
| `--index-file`, `--index-directory`, retire/repair/GC | Refused before selected source/publication effects as unavailable legacy mutations. |

`public_runtime_service` contains a later read-only DIRECT session with search/paging/read-revision and opaque
handles. It reports mutations unavailable; its mutation implementation is test-only. Its `--serve-data-root`
invocation is shadowed by the earlier registry selector, so the primary CLI cannot receive the readiness frame it
expects.

`--self-test` checks shell/in-memory primitives only. It does not open a current retained store, exercise real
file/stdin I/O, construct Qdrant or establish indexed Search readiness.

## Feature topology is not product topology

The default remains `wave1-shell = []`. Qdrant/query/reconcile/proof dependencies are selected through old wave
features. The loopback server, `wave7-lifecycle`, `full-baseline`, generic retention application and several
proof harnesses are already removed. Remaining waves neither repair argv shadowing nor connect retained source,
publication, query and CLI.

Replace the ladder with ordinary dependencies of the one selected composition. A dependency stays optional only
for an actually optional user capability.

## Target composition

```text
one parsed invocation + typed DaemonSettings
→ current-format root + one real process guard
→ typed redb / retained-object / source owners
→ one recoverable preparation path
→ one Qdrant client/runtime
→ publication + current published view
→ Search | Exact | Continue | Expand handlers
→ one bounded local dispatcher
→ orderly shutdown
```

The daemon owns composition, request admission, factual component status, cancellation/deadlines and shutdown.
Source policy, object format, Qdrant translation, publication recovery, query semantics and wire framing remain
with their direct owners.

## Change and verification path

Read [AGENTS.md](AGENTS.md), [FUNCTIONS.md](FUNCTIONS.md), the
[implementation delivery sequence](../../docs/implementation/README.md#delivery-sequence) and
[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264). Change one real producer/consumer chain and
remove the displaced route in the same slice.

After code, run locked Rust 1.98 check/Clippy on the actual daemon binary and immediate consumers. A library
check, command name, feature, constructor or self-test string is not daemon/product evidence.
