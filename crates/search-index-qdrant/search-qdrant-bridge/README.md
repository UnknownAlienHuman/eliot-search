# search-qdrant-bridge

Private translation between the application and the pinned Qdrant Rust client. The bridge owns vendor
schema, mutation, query and readback I/O. Publication owns recovery; `PublishedView` owns route/profile and
evidence bindings; query owns scope and result use.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source and missing connection

`RealDataPlane` contains real async SDK calls, but public-main searches find its constructors in bridge/live
fixtures and the synchronous wrapper, not the daemon product. `BlockingRealQueryPlane::connect` has no
product caller. The wrapper exposes schema verification, query, readback and count, but not publication
mutations.

Current real query accepts an already encoded sparse `(index, value)` vector. Current upsert accepts a
complete `PointRecord`. Native `qdrant/bm25` document/query input is not connected. The planner drops the
uploadable text/vector values needed by the real mutation.

`RealDataPlane` also keeps an in-memory `operation_id → MutationReceipt` map. It is lost at restart, grows
until a fixed receipt cap and duplicates the pending operation that publication must own. `OpContext` stores
a relative `Duration`; constructing one for each nested call can reset the request deadline. Strong ordering
and an executed `QualifiedGate` are hard requirements in code even though the selected one-node/one-writer
local product needs actual endpoint/schema checks, not a qualification credential.

## Target boundary

One daemon-owned Qdrant connection/runtime serves both publication and query:

```text
publication pages → complete/narrow mutation → wait=true → exact readback
query scope + query input → bounded native query → point nominations
```

- Connection uses configured endpoint and optional real credentials, then verifies the server capabilities
  and selected collection schema needed by this installation.
- Every call receives the original absolute deadline/cancellation context and computes remaining time.
- A possible mutation effect returns unresolved to publication. The bridge keeps no replay ledger or
  permanent receipt map.
- Vendor types, status text and credentials remain private.
- The in-memory `QdrantBridge` remains tests only and cannot be a product fallback.

## Retrieval point and evidence

Qdrant stores stable retrieval behavior:

```text
point identity
occurrence incarnation
retrieval role/profile/input
stable scope/filter/group coordinates
named vectors
validity interval
```

Exact source revision, representation, logical occurrence and byte/line range live in the versioned
`ViewManifestEntry`. A retained point can be reused by a new view without mutating an old view's citation.
Index only fields used by real filters/grouping.

Use full 128-bit point addresses and their full identity evidence. Delete numeric-ID zero padding, legacy
bridge payloads, duplicate S9/S11 adapters and payload `vector_digest_*` fields with their consumers.
Actual named vectors or generated native-vector presence are read back; a payload digest is not vector
validation.

## Native lexical and selected hybrid query

Use Qdrant 1.19 `Document` input with explicit `qdrant/bm25` document/query options. Bind tokenizer, case,
stemmer, stopwords, ASCII folding and BM25 parameters to the published profile. Document and query roles
are distinct.

The resolved compatible source scope becomes both the retrieval filter and each sparse leaf's
`params.idf.corpus`. An ordinary filter alone does not scope IDF. Use native prefetch/RRF/grouping for real
Qdrant branches; a later merge cannot recover candidates excluded by a small prefetch.

Baseline top-k is non-exhaustive and needs no `count_exact`. Count/scroll remain explicit operations. A
Qdrant hit is a nomination; source evidence requires pinned-manifest lookup and retained-byte validation.

## Mutation semantics

- full upsert: new point or complete authoritative point replacement only;
- `update_vectors` / `delete_vectors`: selected named-vector changes;
- `set_payload`: merge fields;
- `delete_payload`: remove selected fields;
- `overwrite_payload`: replace complete payload while preserving vectors;
- exact point delete or whole collection-generation delete for cleanup.

Use `wait=true`. Read back exactly affected IDs, vectors, payload fields, validity and schema. A timeout or
cancellation after possible dispatch is unresolved publication work, not rollback. Default/weak ordering is
sufficient for one node, one replica, one serialized writer; another topology may select a stronger policy.
Strict mode is a configured resource guard, not proof of source correctness.

Adding a named-vector schema does not populate existing points. Baseline incompatible profile change builds
and verifies a complete new physical generation, then publication switches views. In-place backfill is an
optional measured optimization with dual writes and explicit completeness.

## Removal boundary

After the connected caller lands, remove:

- `QualifiedGate` as a runtime constructor credential and caller-supplied probe receipts;
- API-key lease ceremony for unauthenticated local Qdrant while retaining actual configured secrets;
- process-local mutation receipt/replay maps and their lifetime cap;
- relative per-call deadline reset;
- hardcoded strong ordering/strict-mode correctness requirements;
- old bridge point/payload/filter/schema models and field-count fixtures;
- product-reachable in-memory oracle paths and unconditional exact count.

Verify one retained file through native BM25 ingestion/query, selected-scope IDF, full/narrow mutations,
readback, unknown outcome recovery, generation switch and old-view evidence lookup. A live probe or passing
constructor without the daemon caller is not product integration.
