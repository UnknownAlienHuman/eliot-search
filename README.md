# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact retained revision identity and explicit gaps. Qdrant
is derived retrieval state; immutable objects remain the source of evidence.

## Read first

| Need | Document |
|---|---|
| Product purpose and user-visible invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path and owner boundaries | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint, decisions and unrun evidence | [R050 registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete change | Nearest `FUNCTIONS.md`, actual current callers and owning issue |

[Documentation authority and task paths](docs/README.md) explain which document wins when text conflicts.
Historical waves, packets, receipts and old issue bodies remain evidence only.

## Current truth

The product is **not implemented end to end**. R050 synchronizes all active Qdrant runtime/package instructions
with the selected implementation; it does not claim runtime success.

```text
1. Foundation
   invocation → typed root/plain settings → handle-safe final-manifest-last root → DataRootGuard

2. Walking skeleton
   one file → ObjectRef → preparation
   → Qdrant 1.20 scoring full-text generation
   → TextQuery → pinned retained excerpt → CLI

3A. Correctness/reference convergence
   restart + authoritative inventory + complete fresh generation/view switch
   → measure predeclared release budgets

3B. Required optimization only where 3A misses budgets
   source-local diff + stable occurrence identity + insert-only additions + fixed closures
```

The repository lock still contains Qdrant client 1.19.0. Upgrade server/client/schema/query together in the real
#263/#264 code cut. Baseline 1.20 retrieval stores text once in payload and uses a scoring full-text index with
explicit `code` tokenizer/text processing and `TextQuery`; no sparse BM25 inference or fixed local `avg_len`.

## Important boundaries

- Plain storage is baseline; #598 encryption is optional.
- Windows root/object publication uses an owner-private handle-based no-clobber finalizer after a native canary.
- `PublishedView` names one exact physical Qdrant collection and matching redb snapshot; aliases are not authority.
- Stage 3A uses deterministic generation-bound point IDs and whole-generation cleanup only.
- Stable occurrence identity, point closures and changed-source deltas are Stage 3B only where #215 budgets fail.
- Per-root/per-source custom sharding is not a generic optimization because BM25 statistics are shard-local.
- Qdrant Edge is a later isolated replacement candidate, not a fallback or dual backend.
- Watcher events are dirty hints; inventory establishes selected state.
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

This is inventory, not required final crate topology. Move useful behavior with real callers and delete displaced
routes/fixtures. In particular, baseline complete-generation assembly may not justify permanent projection/
identity crates, and the old reclaimer package must disappear after its whole-collection helper moves.

## Verification status

R031 ran locked Rust 1.98 check/minimal Clippy for `search-materializer` and `search-unitizer` only. Qdrant 1.20,
Windows finalization, current binaries, Foundation, walking skeleton, restart/convergence, relevance and scale
remain unrun. See [evidence status](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

The existing one-shot scan syntax remains an unverified diagnostic seam in [QUICKSTART.md](QUICKSTART.md), not the
persistent product.

## License

See [LICENSE](LICENSE).
