# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. A user
selects sources and receives ranked, source-backed files, symbols or ranges with verified excerpts, revision
identity and explicit gaps. The result should make the next source read or exact check obvious without loading
the entire corpus into a model context.

The target product is one CLI and daemon. Qdrant is the only indexed retrieval engine; Search owns retained
source bytes, preparation, publication state and evidence. Models, agent controllers and ELIOT Memory OS are
optional clients, not prerequisites.

## Read in this order

| Need | Document |
|---|---|
| Product purpose and observable requirements | [Architecture](docs/architecture/README.md) |
| Selected mechanisms, owners and delivery sequence | [Implementation](docs/implementation/README.md) |
| User-facing navigation, comparison, document and exact behavior | [Product behavior](docs/product/AGENT_ANALYSIS_FRAMEWORK.md) |
| Current checkpoint, inspected evidence and unresolved work | [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md) |
| Exact source seams for one change | Nearest `FUNCTIONS.md` plus actual callers |

The [documentation index](docs/README.md) explains authority and task-specific reading paths. Historical
specifications, waves, packets and generated status files remain in Git history; they do not override current
architecture, implementation owners or observed code.

## Current truth

The persistent application is **not connected**. Useful source readers, preparation code, lexical helpers, a
real Qdrant adapter and query components exist, but no default production route currently joins:

```text
selected source
→ retained bytes and current head
→ recoverable preparation
→ real Qdrant publication and query
→ pinned evidence lookup
→ useful CLI result
```

R031 adopted `pulldown-cmark 0.13.5` and synchronized its lock entry and materializer donor identity. Rust 1.98
`check` and minimal Clippy passed for `search-materializer` and `search-unitizer` in
[Actions run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130). That is
narrow owner evidence, not a daemon, Windows, live-Qdrant or end-to-end result.

The next implementation slice is typed settings and their real consumers under
[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333), followed by retained-source/preparation and
the connected product path coordinated by [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352).

## Workspace map

| Responsibility | Current location |
|---|---|
| CLI and daemon composition | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Sources and retained revisions | [crates/search-source](crates/search-source/) |
| Preparation and lexical input | [crates/search-prep](crates/search-prep/), [search-lexical](crates/search-lexical/) |
| Qdrant projection, publication and adapter | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Query, exact operations and evidence | [crates/search-query](crates/search-query/) |
| Durable current metadata | [search-control-redb](crates/search-control-redb/) |

This layout is an inventory, not a package-count target. Consolidate forwarding or duplicated owners when a
real consumer moves; keep boundaries that isolate a real state, effect, platform or dependency.

## Verification

Use Rust 1.98, locked dependencies and the smallest compiler/Clippy closure covering changed production owners
and immediate consumers. Record exactly what ran in the analysis registry and PR. Tests, native recovery,
relevance and scale evidence follow the implementation they exercise; source presence, receipts and closed
issues are not substitutes.

The current narrow one-shot scan syntax is documented separately in [QUICKSTART.md](QUICKSTART.md). It is a
diagnostic example, not the persistent-product quickstart.

## License

See [LICENSE](LICENSE).
