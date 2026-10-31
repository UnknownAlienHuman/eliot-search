# Code entrypoints and change map

Base inspected: `26497dfd`; CLI dispatch below is implemented in R068 (`881548f1` source). Daemon dispatch is unchanged.
This maps actual files, not a claim that the replacement product is connected.
The [owner map](README.md#owners) defines intended responsibilities; a row below does not require keeping a crate.
Recheck callers and Cargo targets before editing. Update this map only when those entrypoints change.

## Real binaries

| Package / target | Actual root | Important distinction |
|---|---|---|
| eliot-search / eliot-search | [entry.rs](../../bins/eliot-search/src/entry.rs), [Cargo.toml](../../bins/eliot-search/Cargo.toml) | src/main.rs is a legacy test target, not the installed CLI |
| eliot-searchd / eliot-searchd | [entry.rs](../../bins/eliot-searchd/src/entry.rs), [Cargo.toml](../../bins/eliot-searchd/Cargo.toml) | src/main.rs and sealed-* targets are legacy harnesses; checking the library alone misses entrypoint wiring |

## Existing dispatch to replace

These are actual routes, not proposed public interfaces:

```text
CLI main → app::run_main → capture native argv once → bounded dispatch
  help/version                → local output, no daemon
  serve-data-root             → public_client::run_command(tail)
  remote/indexed (harness)     → selected legacy handler(tail)
  remaining command           → app::core::run_command(command, tail)

daemon main
  secure_commands::maybe_refuse_legacy_effects
  → owner_composition::maybe_run_direct_registry
  → native_root_command::maybe_run
  → source_migration_command::maybe_run
  → preparation_composition::maybe_run
  → maybe_run_legacy_loopback (feature-gated)
  → public_runtime_service::maybe_run
  → secure_commands::maybe_run
  → app::run_main (fallback)
```

Only the daemon arrows are still short-circuit alternatives: later handlers run if earlier ones decline.
F1 remains partial: CLI capture/routing is connected, but typed settings and the daemon path still need conversion.
Existing DIRECT/line-protocol handlers are not the final serviced transport. Do not rebuild a second dispatcher.
The daemon also prints help additions after dispatch; update that path when changing help. Do not leave an earlier
legacy handler able to intercept a converted command. Do not route current requests through old protected-store,
key-selection or migration commands merely to preserve their constructors.

## Work by path

| Task | Existing source to trace | Rule owner / consumer to include |
|---|---|---|
| Capture settings | [search-config/src](../../crates/search-config/src/), daemon config_composition | [Settings](../../crates/search-config/FUNCTIONS.md); actual daemon constructors |
| Root and exclusion | [runtime-owner/src](../../crates/search-runtime/search-runtime-owner/src/), [owner_composition](../../bins/eliot-searchd/src/owner_composition/) | [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md); daemon + object store |
| Acquire and retain a body | [native_source.rs](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs), [safe-reader/src](../../crates/search-source/search-safe-reader/src/) | [Reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md), [objects](../../crates/search-source/search-revision-store/FUNCTIONS.md); source-head installation |
| Current rows and stale work | [source-registry/src](../../crates/search-source/search-source-registry/src/), [control-redb/src](../../crates/search-control-redb/src/) | [Source rows](../../crates/search-source/search-source-registry/FUNCTIONS.md); reconcile + preparation + publication |
| Transform and units | [materializer/src](../../crates/search-prep/search-materializer/src/), [unitizer/src](../../crates/search-prep/search-unitizer/src/) | Their FUNCTIONS.md; object reader + projection consumer |
| Qdrant calls | [bridge/src](../../crates/search-index-qdrant/search-qdrant-bridge/src/) | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md); publication + actual query caller |
| Publish and retain a view | [publication/src](../../crates/search-index-qdrant/search-publication/src/), control-redb | [Publisher](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md); query + cleanup |
| Query, evidence, Exact | [query family](../../crates/search-query/) | [Query map](../../crates/search-query/README.md); retained reader + protocol/daemon |
| Child connection and output | [provider-client/src](../../crates/search-provider-client/src/), [protocol/src](../../crates/search-provider-protocol/src/) | [Client](../../crates/search-provider-client/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md); both binaries |
| Remove legacy security | CLI/daemon manifests; runtime secret/crypto packages; pairing/key-selection callers | [Removal map](../architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring); preserve user data/private work, not unused public wrappers |
| Remove Qdrant process scaffolding | Existing supervisor and its Cargo/feature callers | [Removal boundary](../../crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md#removal-boundary); baseline connect uses only the bridge |

Use [caller-search/check commands](WORKING_GUIDE.md#find-callers-before-changing-a-boundary) for each row. Legacy
feature names are not product milestones. Shared data belongs with its real producer/consumer; do not implement
the old search-contracts/search-ports catalogues just because their folders exist.

## Keep the implementation small

Start at the current stage, not every row. Root/settings → one retained file → Qdrant → CLI is the first path.
Use one plain writer, one typed metadata store, one publisher and one serviced client. No crypto/ACL subsystem,
second search engine, controller, duplicate queue database or new wrapper crate is needed to complete that path.
