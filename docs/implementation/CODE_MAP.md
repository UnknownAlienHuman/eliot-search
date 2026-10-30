# Code entrypoints and change map

Inspected main: `6ae24197`. This maps existing files, not a claim that the replacement product is connected.
The [owner map](README.md#owners) defines intended responsibilities; a row below does not require keeping a crate.
Recheck callers and Cargo targets before editing. Update this map only when those entrypoints change.

## Real binaries

| Package / target | Actual root | Important distinction |
|---|---|---|
| eliot-search / eliot-search | [entry.rs](../../bins/eliot-search/src/entry.rs), [Cargo.toml](../../bins/eliot-search/Cargo.toml) | src/main.rs is a legacy test target, not the installed CLI |
| eliot-searchd / eliot-searchd | [entry.rs](../../bins/eliot-searchd/src/entry.rs), [Cargo.toml](../../bins/eliot-searchd/Cargo.toml) | src/main.rs and sealed-* targets are legacy harnesses; checking the library alone misses entrypoint wiring |

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
| Remove legacy security | CLI/daemon manifests; runtime secret/crypto packages; pairing/key-selection callers | [Removal map](../architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring); move actual consumers before deleting packages |

Use [caller-search/check commands](WORKING_GUIDE.md#find-callers-before-changing-a-boundary) for each row. Legacy
feature names are not product milestones. Shared data belongs with its real producer/consumer; do not implement
the old search-contracts/search-ports catalogues just because their folders exist.

## Keep the implementation small

Start at the current stage, not every row. Root/settings → one retained file → Qdrant → CLI is the first path.
Use one plain writer, one typed metadata store, one publisher and one serviced client. No crypto/ACL subsystem,
second search engine, controller, duplicate queue database or new wrapper crate is needed to complete that path.
