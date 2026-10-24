# search-contracts — keep only current shared data

This package must not be the catalogue of every planned operation, proof, provider and lifecycle. It should
contain only small immutable values shared by real current producers and consumers.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [exact source/caller audit](FUNCTIONS.md),
[query cutover](../search-query/README.md) and [local protocol](../search-provider-protocol/README.md).

## Historical baseline inventory

The figures below were measured at the original public-main baseline
`c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`. They are preserved as gross decomposition evidence, not a
current-branch recount, deletion promise or package-size target:

```text
production src                       425,255 bytes (~415.3 KiB)
tests                                131,618 bytes (~128.5 KiB)
Cargo manifests depending directly        47
```

At that baseline production code divided roughly into:

- **127.5 KiB** common bounds/canonical/digest/ID/error/schema infrastructure;
- **168.3 KiB** grants, recipes, plans, protocol, results, evidence frames and lifecycle;
- **113.1 KiB** frozen indexed/Qdrant-neutral collection/payload/vector/readback models;
- **16.4 KiB** source/view vocabulary.

The V2 evidence family was added beside the frozen eleven-recipe V1 system, not used to replace it.
`EvidenceFrameV2` appeared only in its definition, provider codec and protocol test. Those measurements explain
planned-system gravity; current callers must still be traced at the implementation cut before removing code.

## What remains shared

Keep only values with at least two real current consumers:

```text
SourceId / RevisionId / ObjectRef / RepresentationId
ViewId / ViewSequence / stable PointId
source location/range/page/region and logical occurrence
selected source scope and actual finite request limits
retrieval profile/role/input identity
stable RetrievalPoint semantics and versioned ViewManifestEntry

SearchRequest
ExactRequest
ContinueRequest { WindowId, page_size }
ExpandRequest { WindowItemRef, bounded relative context }

EvidenceCard with actual verified excerpt/location
GapSummary and truthful coverage
Search/Exact/Continue/Expand response variants
stable public failure code/class
```

Runtime guards, streams, cancellation, redb rows, Qdrant SDK records, publication cursors and query-window
contents belong to their concrete owners.

## Remove and move

Delete with their old consumers:

- provider binding/grant/capability authority;
- eleven V1 recipes and parallel V2 evidence/orientation families;
- public plans, snapshot/security fences, leg records and fingerprints;
- durable source/continuation handles and validation receipts;
- recipe-specific graph/comparison/provenance result catalogue;
- progressive evidence-frame state machine;
- frozen sparse-only indexed schema, payload/index table and digest readback authority;
- optional-provider/readiness lifecycle records;
- conformance/golden fixtures for removed pre-release bytes.

Move physical collection/vector/payload/query types into the private Qdrant boundary. Move current redb,
publication and window state to their owners. Shrink IDs/reasons/schema to current semantics.

Use Serde for ordinary current request/result records. Keep canonical bytes/hashes only for real content,
artifact, point or profile identity whose owner recomputes them. Do not hash requests, plans, grants or state
transitions into authority.

## Cutover

Change current protocol/client/daemon/result consumers together. Do not add `V3`, deprecated aliases,
forwarding wrappers or raw-byte compatibility constructors. After each family moves, delete its types,
fixtures and generated P00/coverage records immediately.

The crate may remain as a small shared DTO/identity package. If the final set is trivial and used only by the
protocol/application boundary, merge it there and remove this workspace member instead of preserving a name.

## Verification

Run minimal Rust 1.98 check/strict Clippy for changed real producers/consumers and focused current Serde,
identity, location and source-backed result cases. Frozen recipe counts, canonical wire goldens and planned
schema matrices are not gates.
