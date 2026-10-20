# ELIOT Search

ELIOT Search helps agents and people navigate large codebases and document collections. A user selects
sources, asks a question and receives useful files, symbols or ranges with excerpts, revision identity and
clear gaps. The result should make the next source read or exact check obvious.

It is intended to be one standalone local application with a CLI and daemon. Qdrant supplies indexed
retrieval; Search owns source retention, preparation, publication and evidence. It runs without a model or
agent controller. Large repositories and portfolios are a core workload.

## Start here

| Document | Purpose |
|---|---|
| [Architecture](docs/architecture/README.md) | Short product manifesto: purpose, useful results, requirements and principles. |
| [Implementation](docs/implementation/README.md) | Concrete existing owners, changes, removals and donor APIs. |
| [Current smoke](QUICKSTART.md) | The only simple runnable search route and the exact persistent-product blocker. |

[AGENTS.md](AGENTS.md) describes repository work. The [documentation index](docs/README.md) links to
specific source evidence and optional capabilities. Historical masters, waves, packets and receipts are
reference material only.

## Current implementation

The indexed application is not connected. The source contains useful readers, preparation code, lexical
encoding, a real Qdrant adapter and query components, but no production route joins them from one fresh root
to a source-backed CLI result.

The primary daemon is currently a chain of overlapping command interceptors. A direct registry/journal owner
captures several search/read/service command names before later handlers and emits registry metadata rather
than executing the named operation. A separate read-only DIRECT service exists in source, but its
`--serve-data-root` route is shadowed by that earlier interceptor. Other native initialization, preparation and
migration applications are separate systems.

The default Cargo feature is empty `wave1-shell`, so Qdrant/query dependencies are excluded. Enabling
`full-baseline` merely compiles additional old composition systems; it does not create a connected product
entrypoint. There is currently no feature combination which should be represented as the finished Search
application.

The target remains one working path from a local file to a durable revision, indexed result and correct source
excerpt, followed by incremental update, restart and rebuild through the same owners. DIRECT exact scan
remains a separate explicit operation.

## Workspace

| Area | Current location |
|---|---|
| CLI and daemon | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Sources and retained revisions | [crates/search-source](crates/search-source/) |
| Preparation and lexical encoding | [crates/search-prep](crates/search-prep/), [search-lexical](crates/search-lexical/) |
| Qdrant projection, publication and adapter | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Planning, retrieval and source validation | [crates/search-query](crates/search-query/) |
| Durable current state | [search-control-redb](crates/search-control-redb/) |

This is the current layout, not a requirement to preserve every package. Consolidate modules which split one
responsibility without isolating a real dependency or effect.

## Development

Use the pinned Rust toolchain and locked dependencies. Check the changed owner and its immediate real
consumers, for example:

```sh
cargo +1.98.0 check --locked -p <owner> --lib
cargo +1.98.0 clippy --locked -p <owner> --lib -- -D warnings
```

Do not use `--all-features` or `full-baseline` as a product profile during the refactor: they also compile
obsolete harness and lifecycle systems. Select only the actual changed targets. Exercise real application
behavior after integration; compilation, fixtures, feature flags and hard-coded status fields do not establish
a working Windows/live-Qdrant application. Keep GitHub Actions manual-only.

## License

See [LICENSE](LICENSE).
