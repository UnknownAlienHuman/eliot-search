# Qdrant bridge — current API and integration map

The [implementation guide](../../../docs/implementation/README.md) selects one real Qdrant owner. Existing
APIs below are implementation inventory, not compatibility requirements.

## Caller reality

| Current API/source | Actual state |
|---|---|
| `RealDataPlane::connect` | Real async SDK connection. Search finds bridge/live/process test callers and the blocking wrapper, not the daemon product. Requires `QualifiedGate`, connection binding and API-key lease. |
| `BlockingRealQueryPlane::connect` | Builds/reuses one Tokio runtime, but has no product caller in public main. It exposes schema verify/query/readback/count only. |
| `query_filtered` | Sends an already encoded sparse vector through `Query::new_nearest(VectorInput::new_sparse(...))`; copies one eligibility filter into `idf.corpus` when requested. Native text/BM25 `Document` input is absent. |
| `upsert_exact`, close/delete/readback | Real `wait=true` mutations/readback over old `PointRecord`/payload/vector contracts. Current callers are bridge/live tests; publication is not connected. |
| `RealDataPlane.operations` / `real/ledger.rs` | Process-local exact-replay map of `MutationReceipt`; lost at restart and permanently consumes the fixed `max_operation_receipts` capacity. |
| `OpContext` | Holds a relative `Duration` and optional flag. Every SDK call wraps itself in `tokio::time::timeout(context.deadline(), ...)`, so a newly created context can renew time. |
| `QdrantBridge` oracle | In-memory behavioral test implementation. It is not a runtime fallback or source of product evidence. |

A source file is not indexed merely because live parity tests can construct `RealDataPlane`. The connected
producer must supply uploadable text/vectors and publication must call the mutation methods.

## One runtime and operation context

Daemon composition owns one connection/runtime used by both publication and query. Extend or replace the
read-only blocking facade rather than constructing a second runtime for writes.

Use an application request/publication context containing an absolute monotonic deadline and cancellation.
At each SDK call:

```text
check cancellation
remaining = deadline - now
if remaining <= 0: timeout before dispatch
otherwise timeout_at(deadline, RPC)
```

After possible mutation dispatch, timeout/cancellation maps to unresolved publication. Read/query failure is
definite because it commits nothing.

The bridge does not own idempotency history. Publication's persisted pending artifact identifies the exact
mutation pages and resumes by reading actual Qdrant state. Replaying an already verified complete page is a
publication decision; no process-local receipt is authoritative after restart.

## Connection and schema

Construct from configured endpoint and optional credentials. Check current server identity/capabilities and
the exact selected collection generation/schema. Qualification suites remain verification evidence, not a
runtime token. Do not require a probe collection or historical report to open a configured local server.

Use one shard for the local baseline. Index only selected scope/filter/group fields. Read back:

- named sparse/dense vector schema and native text model/options;
- selected payload indexes and strict-mode settings actually configured;
- physical collection/generation identity used by the published view.

Schema/profile incompatibility creates a new generation. Adding a vector name alone never claims that old
points contain values.

## Native BM25 input

Current planner/bridge pass precomputed sparse indices/values. Replace the baseline lexical path with the
pinned SDK's Qdrant `Document` input:

```text
document point: Document(text, model="qdrant/bm25", options=published_document_options)
query input:    Document(query, model="qdrant/bm25", options=published_query_options)
```

Keep document/query roles explicit. `search-lexical` may add bounded identifier expansions to document
text, but Qdrant computes the sparse vector. Do not run a second local encoder merely to predict a digest.

For client-computed optional vectors, retain actual uploadable values through projection pages and compare
actual readback values/shape. Never substitute a payload hash for vector readback.

## Scope and query

Translate one `ResolvedScope` from the pinned `PublishedView` to:

- retrieval filter;
- each sparse leaf's identical `params.idf.corpus` base population;
- optional additional query-only constraints;
- grouping field when selected.

Unknown/empty scope fails before RPC. Split only across different physical routes or incompatible profiles.
Use native prefetch/RRF/grouping for Qdrant branches and preserve returned order. Ordinary top-k returns
nominations and truthful limit/gap metadata, not a complete denominator.

## Stable point model

Change planner/contracts/bridge/query together. One Qdrant point contains stable retrieval state only:

```text
PointId128 + full identity evidence
occurrence incarnation
retrieval input/profile/role
stable source/projection grouping
selected access/filter coordinates
named vectors and validity
```

`ViewManifestEntry` separately maps that point to exact source revision, representation, logical occurrence
and range for one view. Remove revision-bound citation fields from mutable payloads. Old view manifests remain
unchanged while a reused point serves a new view.

## Mutation calls

| Need | Qdrant operation |
|---|---|
| New or complete authoritative point | `upsert_points` with every required vector and complete payload. |
| Add/replace selected vectors | `update_vectors`. |
| Remove selected vectors | `delete_vectors`. |
| Merge payload fields | `set_payload`. |
| Remove payload keys | `delete_payload`. |
| Replace complete payload, preserve vectors | `overwrite_payload`. |
| Retire exact points | exact-ID delete after view guards drain. |
| Retire replaced generation | collection delete after final old-view guard. |

Every publication page awaits `wait=true` and performs exact relevant readback. `set_payload` is not a
complete payload replacement. A narrow update must not erase unrelated vectors or retain stale payload keys.

## Delete after cutover

- `QualifiedGate`/probe receipt constructor requirements from runtime code;
- bridge mutation ledger and `MutationReceipt` replay authority;
- `max_operation_receipts` as a lifetime application limit;
- `BridgeMutation` canonical digest where only publication page identity is needed;
- old bridge `PointRecord`, payload, filter and numeric-ID conversion after shared types migrate;
- payload vector-digest extension;
- hardcoded strong ordering and strict-mode correctness checks;
- unconditional count from ranked query;
- in-memory oracle/product adapter and source-shape fixtures.

## Verification

Exercise through the connected daemon:

1. create/verify a physical generation;
2. publish native BM25 text and find it under the selected scope/IDF population;
3. resolve the returned point through the pinned manifest to retained bytes;
4. preserve unrelated vectors/payload fields under narrow updates;
5. recover timeout after possible dispatch by actual readback;
6. switch generations while an old view remains readable;
7. cancel/expire through one absolute deadline.

Do not claim these results from SDK unit tests or qualification fixtures alone.
