# search-contracts

Shared types and small validation/encoding functions used by real producers and consumers. This package
is not a catalogue of every planned capability.

## Current request/result replacement

The repository currently carries eleven `RecipeIdV1` request/result families, V2 evidence/orientation
forms, public `SearchTaskPlan`, source/continuation handles and capability descriptors. Most are consumed
by codecs, fixtures or disconnected host scaffolding rather than one working search path.

Replace them with one current typed local family used by CLI, daemon and optional restricted adapters:

```text
SearchRequest      scope, query, retrieval options, limits
ExactRequest       scope, literal/regex predicate, completeness limits
ContinueRequest    query-window locator, page size
ExpandRequest      window-item locator, bounded expansion
OrientRequest      optional focus/dimensions
CompareRequest     optional selected subjects/axes

EvidenceCard       source/revision/representation/range, excerpt, rank/freshness/gaps
SearchResponse     cards, truthful coverage/gaps, optional next-window locator
```

The first implementation needs `Search`, `Exact`, `Continue` and `Expand`. Optional types exist only with
real handlers. `compile_exact_scan` plus `execute_exact_scan` collapses into one server-owned exact
operation. `expand_handle` becomes window-item expansion. Corpus profile/delta and provenance do not need
frozen public IDs before their actual data owners and callers exist.

Change decoder, planner, daemon handler, result encoder and clients together. Do not add a third V3 family,
legacy adapters or synthetic values for removed grants/fingerprints/handles.

## Useful shared data

Keep source/revision identity, source coordinates, selected scope, actual limits, indexed payloads and
results that consumers use. Runtime I/O and mutable state belong to concrete owners.

Ordinary retrieval needs no exact-scan budget, plan fingerprint, proof chain, ranking trace, capability
credential or durable handle. Retain actual revision/range/snippet, freshness and gaps. A source-record
revision is not a content revision.

## Serialization and hashes

Use typed Serde serialization for ordinary request/result/event records. Bound the frame before decoding
and bound encoded output. A canonical encoding is needed only for an identity/profile that genuinely
uses its bytes. Old recipe/wire golden bytes are not a compatibility promise.

Use shared BLAKE3/SHA-256 only for real content/artifact identity or integrity. Matching caller digest
fields does not prove that bytes were read or hashed. Do not hash ordinary requests, plans, capability
reports or state transitions merely to make them appear authoritative.

## Ownership and verification

Contract code performs no filesystem, process, network, redb or Qdrant I/O and owns no mutable runtime
state. Shared types earn their place through real consumers. Compile changed producers/consumers and test
useful size bounds, exact source identity and truthful partial results through the connected application.
