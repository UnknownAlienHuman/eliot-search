# Code entrypoints and change map

This map describes the current filesystem after the root cut. Current binaries expose root operations and one
explicit retained-file operation. Native binary and library cases have distinct exact-source scopes in the [registry](../audit/ANALYSIS_REGISTRY.md#evidence-status);
S1 final gates/builds/basic execution passed at 06bb6877. This map does not qualify the future source-to-Qdrant product.
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
      retain-file --file PATH → same owned child → bounded retained-source response validation → output

    daemon entry → app::run_main → app/bootstrap::parse
      standalone help/version → local output, no config/root acquisition
      root command            → capture startup cwd → bootstrap::resolve → typed RootInvocation
                              → native_root_command::run → search-runtime-owner
      retain-file             → open current root → source-row capture → safe read → object readback
                              → current-source CAS in search-control-redb → factual result

| CLI subcommand | Daemon flag | Root function |
|---|---|---|
| inspect-data-root | --inspect-data-root | inspect_plain |
| initialize-data-root | --initialize-data-root | initialize_plain |
| check-existing-root-owner | --check-existing-root-owner | open_plain |
| discard-incomplete-data-root | --discard-incomplete-data-root | discard_incomplete_plain |
| retain-file --file PATH | --retain-file --source-file PATH | open_plain → acquire_file → store_source_bytes/read_source_object → commit_current_source |

Root functions take (&Path, Instant, &AtomicBool). Init/open return PlainRootGuard; inspection returns
PlainRootInspection and discard returns unit. The daemon holds guards through terminal root output.
Its original deadline/cancellation is passed to root/control effects.

The CLI operation comes first and accepts --daemon PATH for child selection. Daemon settings flags may surround
its root command. Positional ROOT or --data-root selects the CLI layer; --config-file and
ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT can supply a missing root. See the
[daemon recipe](../../bins/eliot-searchd/FUNCTIONS.md) for precedence and bounds.

retain-file emits its source row only after bounded same-handle acquisition, immutable object readback, and the
short guarded transaction succeed. It does not produce preparation, Qdrant, search, or evidence results.

There is no broad scan, preparation, migration, persistent search, framed-query or alternate harness route.
Root-state output does not report Qdrant availability. Final native qualification remains distinct from this map.

## Work by path

The first five rows are current consumers. Remaining rows identify future integration owners, not installed
commands or completed stages.

| Boundary | Source / rule |
|---|---|
| Native CLI and child lifetime | [CLI app](../../bins/eliot-search/src/app.rs), [CLI recipe](../../bins/eliot-search/FUNCTIONS.md) |
| Consumed settings and root dispatch | [bootstrap](../../bins/eliot-searchd/src/app/bootstrap.rs), [root handler](../../bins/eliot-searchd/src/native_root_command.rs) |
| Plain root/control | [root source](../../crates/search-runtime/search-runtime-owner/src/plain_root.rs), [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [control source](../../crates/search-control-redb/src/plain_control.rs), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Stable acquisition and retained body | [Reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md), [Objects](../../crates/search-source/search-revision-store/FUNCTIONS.md), daemon native_root_command |
| Current source rows and stale work | [Source family](../../crates/search-source/README.md), [Source values](../../crates/search-source/search-source-registry/FUNCTIONS.md), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Future transforms and unit ranges | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [Unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Future Qdrant calls | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [Lexical](../../crates/search-lexical/FUNCTIONS.md) |
| Future publication and view lifetime | [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md); connect control and query consumers |
| Future evidence and Exact | [Query family](../../crates/search-query/README.md), retained reader and daemon consumer |
| Future framed connection and presentation | [Client](../../crates/search-provider-client/FUNCTIONS.md), [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |

Use the [caller-search method](WORKING_GUIDE.md#find-callers-before-changing-a-boundary) within the selected owner.
Do not restore deleted daemon implementations or unconsumed APIs because an older map or test named them.

## Keep the implementation small

The next connected product path is root/settings → one retained file/current row → preparation → Qdrant → CLI.
Build the actual owners and delete displaced code with its callers. Root-only command checks, helper-native
probes and retained workspace libraries do not qualify that pipeline or an installed baseline.
