# ELIOT Search

ELIOT Search helps agents and people navigate large codebases and document collections. A user selects
sources, asks a question and receives useful files, symbols or ranges with excerpts, revision identity
and clear gaps. The result should make the next source read or exact check obvious.

It is a standalone local application with a CLI and daemon. Qdrant supplies indexed retrieval; Search
owns source retention, preparation, publication and evidence. It runs without a model or agent controller.
Large repositories and portfolios are a core workload.

## Start here

| Document | Purpose |
|---|---|
| [Architecture](docs/architecture/README.md) | Short product manifesto: purpose, useful results, requirements and principles. Keep it in the lead developer's context. |
| [Implementation](docs/implementation/README.md) | Concrete existing owners, changes, removals and donor APIs. |

[AGENTS.md](AGENTS.md) describes repository work. The [documentation index](docs/README.md) links to
specific source evidence and optional capabilities. Historical masters and task packets are reference
material; they do not impose compatibility, cryptographic or process requirements on the new product.

## Current implementation

The public indexed application is not connected yet. The inspected code contains useful source readers,
preparation, lexical encoding, a real Qdrant adapter and query components, but their presence does not
constitute a complete ingestion/search path.

The [implementation guide](docs/implementation/README.md) identifies the missing connections: retained
bytes after native acquisition, actual source sync, persistent source/preparation records, one publication
owner and real query-to-source readback. Delivered request/result schemas also need simplification and
connection to the public client. DIRECT exact scans remain an explicit operation.

The target is one working path from a local file to a durable revision, indexed result and correct source
excerpt, followed by incremental update, restart and rebuild through the same owners. This refactoring
also removes the obsolete applications and mechanisms that would otherwise remain beside that path.

## Workspace

| Area | Current location |
|---|---|
| CLI and daemon | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Sources and retained revisions | [crates/search-source](crates/search-source/) |
| Preparation and lexical encoding | [crates/search-prep](crates/search-prep/), [search-lexical](crates/search-lexical/) |
| Qdrant projection, publication and adapter | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Planning, retrieval and source validation | [crates/search-query](crates/search-query/) |
| Durable control state | [search-control-redb](crates/search-control-redb/) |

This is the current layout, not a requirement to preserve every package. Modules that split one
responsibility without an independent dependency boundary can be consolidated.

## Development

Use the [pinned Rust toolchain](rust-toolchain.toml) and locked dependencies. Check the changed owner
and its real consumers, for example:

```sh
cargo +1.98.0 check --locked -p <owner> --lib
cargo +1.98.0 clippy --locked -p <owner> --lib -- -D warnings
```

For binaries, select the actual binary and needed features. The daemon currently defaults to
`wave1-shell`; `full-baseline` includes the indexed dependencies. The implementation removes this
delivery-wave ladder as the real path is connected. `--all-features` also enables obsolete harness code
and is not the normal product configuration.

Exercise meaningful application behavior after integration. Compilation, fixtures and feature flags do
not establish a working Windows/live-Qdrant application. Keep GitHub Actions manual-only.

## License

See [LICENSE](LICENSE).
