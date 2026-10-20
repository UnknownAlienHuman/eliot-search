# eliot-searchd

Composition root for the intended local Search application. Public main is not yet one application: it
multiplexes several independent pre-release roots, stores and protocols by argv interception.

## Exact production entrypoint order

`src/entry.rs` currently evaluates:

```text
1. secure_commands::maybe_refuse_legacy_effects
2. owner_composition::maybe_run_direct_registry
3. native_root_command::maybe_run
4. source_migration_command::maybe_run
5. preparation_composition::maybe_run
6. optional legacy loopback
7. public_runtime_service::maybe_run
8. secure_commands::maybe_run
9. app::run_main
```

Each layer recaptures/parses argv and may terminate the process. This creates shadowed commands, not one
coherent dispatch table.

### What actually reaches production code

| Route | Actual result |
|---|---|
| `--scan-file`, `--scan-stdin` variants | bounded one-shot scan over current bytes through the final shell application |
| `--initialize-data-root`, inspect/owner check | complex native pre-release root lifecycle application |
| direct registry commands | open/restore the old journal/registry/snapshot system; registration/admission/DEK commands have their own effects |
| `--serve-data-root`, `--search-root`, `--list-sources`, `--read-revision`, `--sync-source-roots` | captured first by direct-registry selection and reduced to registry/root metadata work rather than the named later service/read operation |
| `--prepare-root`, `--prepare-revision` | legacy `DirectStore` preparation application |
| control-migration commands | separate offline migration staging application |
| `--index-file`, `--index-directory`, retire/repair/GC | refused before root/store work as `LEGACY_EFFECTS_UNAVAILABLE` |

A later `public_runtime_service` contains a read-only DIRECT session with search/paging/read-revision and opaque
handles. It reports `service_mutations_available:false` and refuses indexing, sync, preparation, retirement,
GC and migration. Its mutation implementation is `#[cfg(test)]`. In the current entrypoint its
`--serve-data-root` argv is shadowed by the earlier direct-registry interceptor, so the primary CLI cannot
receive the `data_root_ready` frame it expects.

`--self-test` checks shell readiness, one-shot scanning and a SHA-256 vector, then prints
`persistent_direct_store_available:true` as a literal. It does not traverse the persistent entrypoint route and
is not product evidence.

## Feature topology is not product composition

The default is `wave1-shell = []`. Qdrant dependencies are behind `wave3-index`; query dependencies are behind
`wave4-query`; reconcile/overlay, proof and generic retention continue through waves 5–7. `full-baseline`
selects that whole historical ladder, including systems scheduled for deletion, but does not change the
argv chain or connect the source/object/publication/query path.

Replace the ladder with ordinary direct dependencies of the one product composition. Optional dependencies
remain optional only for actual optional capabilities—not delivery waves.

## Target ownership

```text
one parsed invocation + DaemonSettings
→ current-format root + one process guard
→ current redb/object/source owners
→ one preparation path
→ one Qdrant client/runtime
→ publication and current-view owner
→ Search | Exact | Continue | Expand handlers
→ one bounded local dispatcher
→ orderly shutdown
```

The daemon owns startup, factual readiness, request admission, cancellation/deadlines and shutdown. It does not
own duplicated source policy, query algorithms, object formats or operation histories.

Read [AGENTS.md](AGENTS.md), [FUNCTIONS.md](FUNCTIONS.md), the
[architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md). Historical DIRECT/sealed/proxy/migration
commands and fixtures are deletion inputs, not supported product surfaces.
