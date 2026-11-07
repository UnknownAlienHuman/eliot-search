# Code entrypoints and change map

Current binaries expose root operations, index [PATH], retain-file, prepare-source, publish-prepared, foreground watch and read-only search. Native binary and library
cases have distinct exact-source scopes in the [registry](../audit/ANALYSIS_REGISTRY.md#evidence-status).
U1 targeted Windows debug acceptance is at c843171, with earlier transport/budget and initial-publication scopes kept separate.
The [owner map](README.md#owners) defines responsibilities; a row does not require keeping a crate.

## Real binaries

| Package / target | Actual entrypoint and implementation |
|---|---|
| eliot-search / eliot-search | [entry.rs](../../bins/eliot-search/src/entry.rs) → [app.rs](../../bins/eliot-search/src/app.rs); [manifest](../../bins/eliot-search/Cargo.toml) |
| eliot-searchd / eliot-searchd | [entry.rs](../../bins/eliot-searchd/src/entry.rs) → [app.rs](../../bins/eliot-searchd/src/app.rs); [manifest](../../bins/eliot-searchd/Cargo.toml) |

Both packages define only their binary target. The former library, main/sealed harness and wave/loopback feature
routes are removed. Check the real binary; package names and retained workspace crates do not imply handlers.

<a id="existing-dispatch-to-replace"></a>
## Current operator dispatch

    CLI entry → app::run_main → native command parse
      standalone help/version → local output, no daemon
      root subcommands        → owned eliot-searchd child with matching --operation and settings
                              → bounded root response validation → output
      watch                   → repeated owned saved-selection Index → cleanup → cancellable interval
      index [PATH]            → same owned child → closed refreshed result → output
      retain-file --file PATH → same owned child → bounded retained-source response validation → output
      prepare-source          → same owned child → parsed source ID/unit-bound stored-unit validation → output
      publish-prepared        → same owned child → closed generation-state validation → output
      search                  → QueryClient serviced framed child → complete validated terminal → safe output

    daemon entry → app::run_main → app/bootstrap::parse
      standalone help/version → local output, no config/root acquisition
      root command            → capture startup cwd → bootstrap::resolve → typed RootInvocation
                              → native_root_command::run → search-runtime-owner
      index                   → open current root once → scoped admission or reload all selections
                              → bounded unique file union + small scope metadata → typed retain/prepare
                              → verify scan → guarded scoped removal → publish → refreshed result
      retain-file             → open current root → source-row capture → safe read → object readback
                              → current-source CAS in search-control-redb → factual result
      prepare-source          → open current root → preserve settings → read-only preparation capture
                              → retained input → prepare/store/read/decode artifact → outcome CAS → stored unit
      publish-prepared        → open root → all-source/preparation capture → fixed pages/evidence
                              → actual backend build/readback + checked canary → current CAS
                              → complete matching RO PublishedView → generation state/counts
      search-stdio            → framed Query/Cancel I/O + one joinable synchronous worker
                              → root + bounded pin_published → exact backend query
                              → returned evidence/CapturedSource in same snapshot
                              → verified Source/Prepared + full unit/citation → bounded terminal flush

| CLI subcommand | Daemon flag | Root function |
|---|---|---|
| inspect-data-root | --inspect-data-root | inspect_plain |
| initialize-data-root | --initialize-data-root | initialize_plain |
| check-existing-root-owner | --check-existing-root-owner | open_plain |
| discard-incomplete-data-root | --discard-incomplete-data-root | discard_incomplete_plain |
| watch [--interval-ms N] --qdrant-url URL | owned --index --index-owner-stdio child, no PATH | app/watch.rs → one bounded iteration → observed cleanup → after-completion sleep |
| index [PATH] --qdrant-url URL | --index [--index-path PATH] with matching settings | open_plain_with_recovery → capture_selections/admit_selection → unique scoped/union scan → retain/prepare/remove_current_sources/publish |
| retain-file --file PATH | --retain-file --source-file PATH | open_plain → acquire_file → store_object/read_object → commit_current_source |
| prepare-source --source-id UUID [--unit N] | --prepare-source --source-id UUID [--unit N] | open_plain → prepared_source::run → retained read/prepare/store/decode → commit_preparation |
| publish-prepared --qdrant-url URL | --publish-prepared with matching settings | open_plain → published_corpus::run → compact references/Prepared hydration/backend/current/view → generation state |
| search --query TEXT [--limit N] --qdrant-url URL | --search-stdio with framed SearchRequest | framed_search::run → pin_published → verified_query::execute → full retained unit/citation |

Read-only/root operator functions take (&Path, Instant, &AtomicBool); open_plain_with_recovery takes the shared
Arc cancellation flag for maintained engine repair callbacks. Index alone uses that capability.
Init/open return PlainRootGuard; inspection returns
PlainRootInspection and discard returns unit. The daemon holds guards through terminal root output.
Its original deadline/cancellation is passed to root/control effects.

The CLI operation comes first and accepts --daemon PATH for child selection. Daemon settings flags may surround
its root command. Positional ROOT or --data-root selects the CLI layer; --config-file and
ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT can supply a missing root. See the
[daemon recipe](../../bins/eliot-searchd/FUNCTIONS.md) for precedence and bounds.

retain-file emits its source row only after bounded same-handle acquisition, immutable object readback, and the
short guarded transaction succeed. It does not produce preparation, Qdrant, search, or evidence results.

prepare-source resolves a retained SourceId without opening the live path. It returns actual complete stored
text/ranges or explicit empty/Gap, using the serialization-derived allowance for a full 64 KiB unit. The CLI
preserves selectors from its first parse; legal native path values may equal option names.

Framed search is connected. It performs no live-source scan, reprepare, publish or CONTROL/backend writes;
only returned retained bindings/objects are read. Publication/recovery keeps complete graph validation.
Root-state output does not report Qdrant availability. Final native qualification remains distinct from this map.

## Work by path

Rows marked future identify integration owners, not installed commands or completed stages.

| Boundary | Source / rule |
|---|---|
| Native CLI and child lifetime | [CLI app](../../bins/eliot-search/src/app.rs), [CLI recipe](../../bins/eliot-search/FUNCTIONS.md) |
| Consumed settings and root dispatch | [bootstrap](../../bins/eliot-searchd/src/app/bootstrap.rs), [root handler](../../bins/eliot-searchd/src/native_root_command.rs) |
| Plain root/control | [root source](../../crates/search-runtime/search-runtime-owner/src/plain_root.rs), [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [control source](../../crates/search-control-redb/src/plain_control.rs), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Stable acquisition and retained body | [Reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md), [Objects](../../crates/search-source/search-revision-store/FUNCTIONS.md), daemon native_root_command |
| Current source rows and stale work | [Source family](../../crates/search-source/README.md), [Source values](../../crates/search-source/search-source-registry/FUNCTIONS.md), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Retained-only preparation and stored-unit output | [Daemon handler](../../bins/eliot-searchd/src/prepared_source.rs), [CLI validator](../../bins/eliot-search/src/prepared_response.rs), [CONTROL rows](../../crates/search-control-redb/src/current_preparation.rs) |
| Exact UTF-8 and complete artifact/unit ranges | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [Unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Concrete Qdrant operations | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [lexical policy](../../crates/search-lexical/FUNCTIONS.md) |
| Corpus publication/current/view | [Daemon publisher](../../bins/eliot-searchd/src/published_corpus.rs), [CONTROL](../../crates/search-control-redb/src/publication.rs), [RO graph validator](../../crates/search-control-redb/src/publication/verify.rs), [policy](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Returned evidence and retained units | [Query worker](../../bins/eliot-searchd/src/verified_query.rs), [CONTROL resolver](../../crates/search-control-redb/src/publication.rs), [evidence rule](../../crates/search-query/search-candidate-validator/FUNCTIONS.md) |
| Serviced framed child and presentation | [Daemon I/O](../../bins/eliot-searchd/src/framed_search.rs), [client](../../crates/search-provider-client/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Future Exact/windows | [Query family](../../crates/search-query/README.md); retained policy recipes, no restored dormant crates |

The old projection-planner/publication source, manifests and exclusive fixtures were retired in P1 with their
VerifiedUnitSet consumers. Their FUNCTIONS.md and AGENTS.md remain canonical Q1 recipes at the existing paths;
their concrete Q1 producer is now the daemon/CONTROL pair above. No old crate was restored.

Use the [caller-search method](WORKING_GUIDE.md#find-callers-before-changing-a-boundary) within the selected owner.
Do not restore deleted daemon implementations or unconsumed APIs because an older map or test named them.

## Keep the implementation small

That read-only query → pinned published evidence → verified retained text/citation → CLI path is connected.
Scoped index, bare selection reload and foreground watch now use those helpers. R1 next qualifies remaining
recovery/convergence/lifetime gates; it does not require a concurrent view framework by default.
Confirmed absence and inaccessible inventory stay distinct.
Build the actual owners and delete displaced code with its callers. Root-only command checks, helper-native
probes and retained workspace libraries do not qualify that pipeline or an installed baseline.
