# ELIOT Search

ELIOT Search helps agents and people navigate large codebases and document collections. A user selects
sources, asks a question and receives useful files, symbols or ranges with excerpts, revision identity and
clear gaps. The result should make the next source read or exact check obvious.

The target is one standalone local application with a CLI and daemon. Qdrant supplies indexed retrieval;
Search owns source retention, preparation, publication and evidence. A model or agent controller is not
required by that design. Large repositories and portfolios are a core workload.

## Start here

| Document | Purpose |
|---|---|
| [Architecture](docs/architecture/README.md) | Short product manifesto: purpose, useful results, requirements and principles. |
| [Implementation](docs/implementation/README.md) | Concrete owners, changes, removals and donor APIs. |
| [One-shot scan example](QUICKSTART.md) | Existing narrow route; current-HEAD build/run is unverified. Not a persistent-product quickstart. |
| [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md) | Inspected revisions, findings, actual changes, execution evidence and remaining gaps. |

[AGENTS.md](AGENTS.md) describes repository work. The [documentation index](docs/README.md) links to specific
source evidence and optional capabilities. Git history preserves retired specifications; they are not current
implementation requirements.

## Current implementation

The indexed application is not connected. Source contains useful readers, preparation code, lexical encoding,
a real Qdrant adapter and query components, but no production route joins them from one fresh root to a
source-backed CLI result.

The primary daemon still has overlapping command interceptors. The inspected registry/journal interceptor
claims several search/read/service names before later handlers. Native initialization, preparation, migration
and the read-only DIRECT service are not one product. See the [daemon map](bins/eliot-searchd/FUNCTIONS.md).

The standalone snapshot/sealed applications and daemon TCP/token proxy were removed in R016/R019. R020 removes
the empty retention package and `wave7-lifecycle`/`full-baseline` Cargo entries. The remaining `wave1`–`wave6`
features still select old composition fragments; enabling them does not establish a working product. No new
all-in-one acceptance feature replaces the deleted aliases.

The default feature remains the empty `wave1-shell`. The source's narrow one-shot exact-scan route is distinct
from persistent indexed search. Neither that route nor the current daemon build was rerun during the latest
cleanup. An earlier broad daemon check failed; old successful library checks do not certify this checkout.
Exact evidence and limitations belong to the registry, not an inferred green status here.

## Workspace

| Area | Current location |
|---|---|
| CLI and daemon | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Sources and retained revisions | [crates/search-source](crates/search-source/) |
| Preparation and lexical encoding | [crates/search-prep](crates/search-prep/), [search-lexical](crates/search-lexical/) |
| Qdrant projection, publication and adapter | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Query and evidence | [crates/search-query](crates/search-query/) |
| Durable current state | [search-control-redb](crates/search-control-redb/) |

This is the current layout, not a requirement to preserve every package. Consolidate modules that split one
responsibility without isolating a real dependency or effect.

## Development

Use the pinned Rust toolchain and locked dependencies. Check the actual changed target and its immediate
consumers. For example, changes to daemon composition require the binary, not just its small library:

```sh
cargo +1.98.0 check --locked -p eliot-searchd --bin eliot-searchd
cargo +1.98.0 clippy --locked -p eliot-searchd --bin eliot-searchd -- -D warnings
```

These are commands to run, not reported passing results. Use `--lib` only for a genuinely library-scoped
change. Select existing features required by the real consumer; do not equate `--all-features` with a product
profile or restore the removed `full-baseline` alias. Broader tests follow the connected implementation.
Compilation, fixtures and hard-coded status fields do not establish Windows/live-Qdrant behavior. Keep Actions
manual-only.

## License

See [LICENSE](LICENSE).
