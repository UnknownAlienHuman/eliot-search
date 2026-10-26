# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact revision identity and explicit gaps. Qdrant is the
only selected indexed retrieval engine; retained immutable objects remain source evidence.

## Read first

| Need | Document |
|---|---|
| Product purpose and user-visible invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path and owner boundaries | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint, decisions and unrun evidence | [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| Upstream dependencies and analogue dispositions | [Donor/analogue register](docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| One concrete change | Nearest `FUNCTIONS.md`, actual current callers and owning issue |

[Documentation authority](docs/README.md) explains which layer wins. Historical architecture, waves, packets,
receipts and old issue bodies remain evidence only.

## Current truth

The product is **not implemented end to end**. R048 verifies donors/analogues and simplifies the implementation
sequence; it adds no runtime result.

```text
1. Foundation
   one invocation
   → bounded root/plain settings
   → inspect | manifest-last initialize | open
   → one DataRootGuard and factual status

2. Walking skeleton
   one selected file
   → stable read
   → plain immutable ObjectRef
   → current SourceRecord
   → one preparation result
   → one fresh physical Qdrant generation
   → one Search
   → pinned retained excerpt
   → CLI output

3A. Fresh-generation convergence
   restart + authoritative inventory
   → any selected change builds a complete new physical generation
   → matching evidence/current commit
   → explicit PublishedView install

3B. Optional measured incrementality
   stable occurrence reuse
   → insert-only additions + fixed non-creating closures
   → compaction only if full rebuild cost justifies this machinery
```

Foundation does not define future source/Qdrant/query settings. The walking skeleton and 3A use final owners and
persisted values, but avoid watcher, encrypted storage, paging, aliases and per-point cleanup.

Plain storage is baseline. [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598) is an optional
encrypted profile and cannot block the plain source → Qdrant → excerpt path.

## Safety and simplicity decisions

- One process owns the root through one actual guard; no lease/epoch/receipt owner system.
- Empty-root initialization publishes one final manifest last and uses exact discard/retry, not resume/replay.
- One immutable object owner uses same-directory no-clobber finalization and exact existing/final verification.
- Redb stores compact references/current state; no redb writer spans object/Qdrant/process I/O.
- Publication builds a complete fresh physical collection before committing/installing the matching view.
- `PublishedView` names that physical collection directly; a Qdrant alias is not visibility authority.
- Fresh visible collections are immutable. Point closures/stable identity are later measured optimization.
- Retired collections are deleted only after all guards and targetable work are gone; uncertain cleanup remains
  visible/backpressured and collection names are never reused.
- Watcher events only mark a root dirty; authoritative inventory establishes selected state.
- One query pins one view/scope and resolves evidence through retained objects.
- Qdrant remains the only selected backend; no Tantivy/Zoekt fallback trait before measurement.
- Issues track ownership, not prerequisite order. Optional profiles do not block baseline.

## Workspace map

| Responsibility | Current location |
|---|---|
| CLI and daemon composition | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Root and settings | [search-runtime-owner](crates/search-runtime/search-runtime-owner/), [search-config](crates/search-config/) |
| Source and immutable objects | [crates/search-source](crates/search-source/) |
| Preparation | [crates/search-prep](crates/search-prep/) |
| Qdrant identity, bridge and publication | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Query and retained evidence | [crates/search-query](crates/search-query/) |
| Durable compact metadata | [search-control-redb](crates/search-control-redb/) |

This is inventory, not required final crate topology. Move useful behavior with real callers and delete the
replaced path/fixtures instead of preserving forwarding facades.

## Verification status

R031 ran locked Rust 1.98 check/minimal Clippy for `search-materializer` and `search-unitizer` only. Current
binaries, Foundation, walking skeleton, fresh-generation convergence, Windows durability, live Qdrant,
restart/recovery, relevance and scale remain unrun. See the
[registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

The existing one-shot scan syntax is documented in [QUICKSTART.md](QUICKSTART.md) as an unverified diagnostic
route, not the persistent product.

## License

See [LICENSE](LICENSE).