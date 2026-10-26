# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact retained revision identity and explicit gaps. Qdrant
is derived retrieval state; immutable objects remain the source of evidence.

## Read first

| Need | Document |
|---|---|
| Product purpose and user-visible invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path and owner boundaries | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint, decisions and unrun evidence | [R049 registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete change | Nearest `FUNCTIONS.md`, actual current callers and owning issue |

[Documentation authority and task paths](docs/README.md) explain which document wins when text conflicts.
Historical waves, packets, receipts and old issue bodies remain evidence only.

## Current truth

The product is **not implemented end to end**. R049 corrects implementation choices; it does not claim runtime
success.

```text
1. Foundation
   invocation → typed root/plain settings → final-manifest-last root → DataRootGuard

2. Walking skeleton
   one file → ObjectRef → preparation
   → Qdrant 1.20 scoring full-text generation
   → TextQuery → pinned retained excerpt → CLI

3A. Correctness/reference convergence
   restart + authoritative inventory + complete fresh generation/view switch
   → measure release budgets

3B. Required optimization only where 3A misses budgets
   stable occurrence reuse + insert-only additions + fixed closures
```

Qdrant 1.20 is selected for the implementation cut because direct BM25 over a scoring full-text payload index and
the `code` tokenizer remove baseline sparse-vector inference and fixed average-length management. The repository
lock still contains client 1.19.0; no dependency has changed yet.

## Important implementation boundaries

- Plain storage is baseline; #598 encryption is optional.
- Windows root/object publication uses an owner-private handle-based no-clobber finalizer after a focused canary.
  The current path-based `tempfile::persist_noclobber` implementation is not trusted as that finalizer.
- `PublishedView` names one exact physical Qdrant collection; aliases are not serving or recovery authority.
- Stage 3A visible collections are immutable. A changed desired state builds a complete new generation.
- Stage 3A is a release mode only for corpus classes whose declared latency, throughput and disk budgets pass.
- Point-level incrementality is required only for classes where those budgets fail.
- Per-root/per-source custom sharding is not a generic shortcut because BM25 statistics are shard-local.
- Qdrant Edge is a promising later replacement spike, not the current baseline.
- Watcher events are dirty hints; startup/explicit/periodic inventory establishes selected state.
- No redb writer, view-manager mutex or QueryWindow mutation borrow spans object/Qdrant/process/model I/O.

## Workspace map

| Responsibility | Current location |
|---|---|
| CLI and daemon composition | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Root and settings | [search-runtime-owner](crates/search-runtime/search-runtime-owner/), [search-config](crates/search-config/) |
| Source and immutable objects | [crates/search-source](crates/search-source/) |
| Preparation | [crates/search-prep](crates/search-prep/) |
| Qdrant bridge/publication | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Query and retained evidence | [crates/search-query](crates/search-query/) |
| Durable compact metadata | [search-control-redb](crates/search-control-redb/) |

This is inventory, not required final crate topology. Move useful behavior with real callers and remove displaced
routes/fixtures rather than preserving forwarding facades.

## Verification status

R031 ran locked Rust 1.98 check/minimal Clippy for `search-materializer` and `search-unitizer` only. Qdrant 1.20,
Windows finalization, current binaries, Foundation, walking skeleton, restart/convergence, relevance and scale
remain unrun. See [evidence status](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

The existing one-shot scan syntax remains an unverified diagnostic seam in [QUICKSTART.md](QUICKSTART.md), not the
persistent product.

## License

See [LICENSE](LICENSE).
