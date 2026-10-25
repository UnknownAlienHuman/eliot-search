# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact revision identity and explicit gaps. Qdrant is the
only indexed retrieval engine; retained immutable objects remain the source of evidence.

## Read first

| Need | Document |
|---|---|
| Product purpose and user-visible invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path and owner boundaries | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint, decisions and unrun evidence | [Analysis registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete change | Nearest `FUNCTIONS.md`, actual current callers and the owning issue |

[Documentation authority and task paths](docs/README.md) explain which document wins when text conflicts.
Historical architecture, waves, packets, receipts and old issue bodies remain evidence only.

## Current truth

The product is **not implemented end to end**. R047 verifies and corrects the implementation instructions; it
does not claim runtime success.

The first two production cuts are deliberately small:

```text
Foundation:
  one invocation capture
  → bounded typed root/plain settings
  → inspect | initialize | open current root
  → one DataRootGuard and factual status

Walking skeleton:
  one explicitly selected supported file
  → stable read once
  → plain immutable ObjectRef
  → current SourceRecord
  → one preparation result
  → one fresh Qdrant generation
  → one one-shot Search
  → pinned retained excerpt
  → CLI output
```

Foundation does not define source/Qdrant/query settings before those consumers exist. The walking skeleton uses
final owners and values, but initially omits broad inventory, watcher, encrypted storage, paging, physical cleanup
and general recovery.

Plain storage is the baseline. [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598) is an optional
encrypted profile and cannot block the plain source → Qdrant → excerpt path.

## Safety decisions

- One process owns the current root through one actual guard; no lease/epoch/receipt owner system.
- One immutable object owner stores source and prepared bytes; redb stores references and compact current state.
- One publisher owns Qdrant effects and `PublishedView` installation.
- New Qdrant points use explicit `insert_only`; default upsert is not a retry primitive.
- A point is closed by one fixed non-creating `valid_until` mutation and is not individually deleted.
- Compaction/rebuild uses a never-reused generation; cleanup deletes only whole retired generations after guards
  are gone and exact collection absence is observed.
- One query pins one view, resolves one non-widening scope and reads evidence through immutable objects.
- One-shot output exposes no reusable window locator; live paging is a later same-session capability.
- Issues track work ownership, not prerequisite order. Optional profiles do not block the baseline.

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

This is an inventory, not a required final crate topology. Move useful behavior with real callers; remove the
replaced path and exclusive fixtures rather than preserving forwarding facades.

## Verification status

R031 ran locked Rust 1.98 check and minimal Clippy for `search-materializer` and `search-unitizer` only. Current
binaries, Foundation, the walking skeleton, Windows durability, live Qdrant, restart/recovery, relevance and scale
remain unrun. See the [registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

The existing one-shot scan syntax is documented in [QUICKSTART.md](QUICKSTART.md) as an unverified diagnostic
route, not the persistent product.

## License

See [LICENSE](LICENSE).
