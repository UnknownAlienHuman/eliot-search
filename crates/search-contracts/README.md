# search-contracts

Small shared data used by real producers and consumers. This package performs no filesystem, process,
network, redb or Qdrant I/O and owns no mutable lifecycle.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md),
[query cutover](../search-query/README.md) and [local protocol](../search-provider-protocol/README.md).

## Current source is a planned-system catalogue

`src/` is about 486 KiB. It includes:

- authority/grant/capability records;
- 49.8 KiB canonical encoding and 25.1 KiB digest framework;
- V1 recipes, request/task-plan/query/lifecycle structures;
- evidence frames and parallel result families;
- indexed field/payload/vector/schema/readback/eligibility models;
- durable source/continuation handles and proof-oriented IDs.

Most of these types are consumed by codecs, fixtures or disconnected host/planner scaffolding. A shared type
is not justified merely because several obsolete layers import it.

## Current shared application data

Keep only data with at least two real current consumers:

```text
SourceId / RevisionId / ObjectRef / RepresentationId
ViewId / view sequence / stable PointId
SourceLocation / byte-line range / logical occurrence
SelectedSourceScope and finite query limits
Retrieval profile identity actually used by preparation/publication/query

SearchRequest
ExactRequest
ContinueRequest { WindowId, page_size }
ExpandRequest { WindowItemRef, bounded context }
optional OrientRequest / CompareRequest only with real handlers

EvidenceCard
GapSummary / truthful coverage
Search/Exact/Continue/Expand response variants
stable public failure code/class
```

Runtime-only guards, streams, clocks, cancellation objects, Qdrant SDK types, redb rows and mutable stores
belong to their concrete owners.

## Direct request/result replacement

Replace eleven `RecipeIdV1` request/result families, V2 evidence/orientation forms, `SearchTaskPlan`, grant/
capability/dependency records and durable source handles with one current family used by CLI, daemon and
optional restricted adapters.

The first implementation needs `Search`, `Exact`, `Continue` and `Expand`. Optional operations do not exist
on the wire until their handlers exist. Exact compile/execute collapses into one server-owned operation.
Handle expansion becomes one query-window item plus bounded relative context.

Change current decoder, daemon handler, result encoder and client together. Do not add a V3 compatibility
family, forwarding facade or synthetic fields for removed IDs/digests/grants.

## Stable retrieval versus versioned evidence

Shared indexed data distinguishes:

```text
stable retrieval point:
  full point identity and collision evidence
  occurrence incarnation
  retrieval role/profile/input identity
  stable source/group/filter coordinates
  valid_from / valid_until view sequence
  actual named vectors or native document input

ViewManifestEntry:
  view / point
  exact retained source revision ObjectRef
  representation / logical occurrence
  byte/line range and real verification data
```

Do not place exact citation revision/range or payload/vector digest echoes in the Qdrant point contract.
Qdrant is a retrieval index; the pinned manifest and immutable reader own evidence.

Collection configuration types belong to the Qdrant bridge/publication owner unless preparation/query also
need an abstract retrieval-profile field. Do not freeze vendor topology, strictness or payload-index tables
into a provider-neutral global contract.

## Serialization and identity

Use Serde for ordinary request/result/event records. Bound the complete frame before decoding and bound
encoded output. A canonical representation exists only where an actual content/artifact/profile identity
consumes those bytes.

Keep BLAKE3/SHA-256 only for real content/object/artifact integrity or a stable point/profile identity whose
owner verifies it. Do not hash ordinary requests, plans, capability reports, grants or state transitions to
make them appear authoritative. Matching caller-supplied digests does not prove bytes were read.

## Removal boundary

Delete with their last consumers:

- `recipes`, public plan/query authority and capability/grant/lifecycle families;
- generic canonical/digest framework used only by those records;
- evidence-frame compatibility families and proof chains;
- durable handle/continuation and protocol session records;
- frozen sparse-only indexed schema, 20-field payload table and digest readback authority;
- provider/process/store-specific error and receipt shapes;
- conformance/golden fixtures for removed pre-release bytes.

Move concrete collection schema/point/payload/query records into the private Qdrant boundary. Move redb rows,
publication cursor and query window state to their owners. Keep this crate small enough to understand by
reading it, not by consulting a contract registry.

## Verification

Compile changed real producers/consumers. Exercise request/result Serde bounds, exact source/location identity,
truthful partial/gap behavior, stable point/manifest linkage and CLI/daemon round trips through the connected
application. Frozen recipe counts, canonical byte goldens and all-planned-feature matrices are not gates.
