# Qdrant bridge — source audit and replacement map

The [implementation guide](../../../docs/implementation/README.md) selects one real Qdrant owner. This map
owns SDK translation and readback meaning; [publication](../search-publication/FUNCTIONS.md) owns effect
ordering, ambiguous dispatch and recovery. Donor source review used Qdrant 1.19.0 on 2026-10-21; current IDF
documentation and remaining constructor requirements were rechecked at `5af8cde5` on 2026-10-23.
Target instructions are not implemented-state claims.

## Current source inventory

| Source/API | Actual behavior and defect |
|---|---|
| `src/real.rs`, `RealDataPlane` | Real SDK client coupled to auth lease, schema cache and process-local mutation receipts. The historical gate field is gone. Normal daemon publication/query is still not connected. |
| `real/connect_schema/connect.rs` | No QualifiedGate input. Still requires supervisor binding and API-key lease, checks an exact server build, and uses fixed nested transport timeouts. |
| `qualified/gate.rs` | Live-report validator now returns `()` for tests/diagnostics. It cannot produce an activation token. Do not recreate the removed wrapper under another name. |
| other `qualified/**`, `auth.rs` | Fixed artifact/client metadata and repeated binding checks remain; the SDK retains its own API-key copy. These remaining dependencies need their own caller cutover. |
| `real/blocking.rs` | Reusable Tokio runtime exposes reads only; no normal publication mutation consumer. |
| `real/ledger.rs` | Memory-only operation map disappears on restart and consumes a lifetime receipt capacity. |
| `OpContext` | Relative Duration renews the budget for nested RPCs; collection existence check escapes that timeout. |
| `schema.rs`, `create.rs` | Sparse-only strict/strong single-shard probe profile presented as universal schema validity. |
| `real/mutations/upsert.rs` | Full UpsertPoints with default update mode, wait=true and strong ordering. Tokio timeout returns unknown; it does not establish server quiescence. |
| `real/mutations/readback.rs` | Retrieves payload/vectors and compares decoded PointRecord with caller-known explicit vectors. This cannot be transplanted unchanged to native Document inference. |
| `query_filtered`, payload/vector codecs | Precomputed sparse pairs; global IDF or full retrieval-filter clone; echoed vector/evidence digest fields. |
| in-memory QdrantBridge / capability fixtures | Test oracle, never a product fallback or runtime authority. |

## One concrete runtime

One daemon-owned qdrant_client::Qdrant serves publication and queries. Use direct async composition, or one
reused Tokio runtime if synchronous composition remains; not independent read/write runtimes. Endpoint,
actual optional credential and ordinary transport settings are inputs. Credential rotation rebuilds the
client. Vendor types/status text and credentials remain inside this adapter.

Startup checks the running server's required features and the selected physical generation/schema. Keep the
existing dependency pin until an explicit upgrade. Historical probes remain release evidence, not constructor
credentials. QualifiedGate is already removed; do not replace it with another receipt, readiness wrapper or
fixture-backed constructor requirement. Removing the remaining auth-binding ceremony must preserve actual
credential loading and current endpoint/schema failures, not substitute an unconditional ready Boolean.

Every nested connect/schema/effect/readback/query call uses the same attempt's absolute monotonic deadline
and cancellation. Server operation timeout and local timeout are different observations. A local timeout,
client drop or reconnect is not proof that an already dispatched write cannot finish later.

## Implement only the operations consumed now

The connected baseline needs generation create/verify, creation of complete new points, one validity closure,
exact point read/delete, whole-generation delete and native BM25 query. Count/scroll and optional hybrid/vector
operations are added only for an actual caller. An SDK API catalogue is not a backlog.

For a new immutable point, use the pinned client's insert-only mode and verify any already-present point
against the expected identity/state. A collision or foreign existing point is never overwritten. Insert-only
is protection against overwriting an existing point, not protection against a late insert after deletion.
The publication recipe supplies the additional dispatch/lifetime rule.

Narrow SDK semantics remain useful knowledge, not permission to mutate a published point:

| SDK operation | Meaning when a selected caller actually needs it |
|---|---|
| full upsert | Replaces an existing point, including omitted vectors; not an immutable-point retry primitive. |
| update/delete vectors | Changes selected vectors, preserving others. |
| set/delete payload | Merges fields or deletes selected keys. |
| overwrite payload | Replaces payload while retaining vectors. |

Such operations may construct an unexposed staging target. They must not change retrieval state used by an
existing published view. Do not generate unused wrappers or a second mutation framework.

## Immutable retrieval state versus versioned evidence

A redb read snapshot does not snapshot Qdrant. Once a point is published, its vectors, retrieval input/profile,
source/group meaning and query-visible filtering/scoring payload are immutable. A semantic change creates a
new point (or incompatible generation) and closes the old interval. Reuse of derived vector work does not
require reusing a mutable point ID. If path/title metadata is used only for presentation, keep its changing
value in the view's evidence/source rows rather than rewriting a Qdrant payload.

The permitted in-place change is one fixed exclusive valid_until closure at the pending target sequence.
Retries write that same closure, never remove it or choose another boundary. Pure coordinate/revision changes
for unchanged retrieval meaning update only evidence metadata. The planner's occurrence/input identity rule
and the bridge must agree on every field that can affect candidates, IDF, grouping or rank.

Qdrant stores the full point address, collision/input identity, occurrence/role/profile, necessary stable
filter/group fields, validity interval and actual named vectors. The pinned view resolves exact source and
prepared ObjectRefs, representation and byte/line ranges. Payload digests are not source-byte evidence.

## Native lexical input and bounded readback

Use Document(text, model="qdrant/bm25", options=published_role_options) for both ingestion and query, with
the appropriate published document/query options. Preparation may supply bounded identifier expansion;
it does not run another BM25 encoder to predict token IDs or manufacture expected vector digests.

Pending pages retain the actual text/options or concrete optional vector values needed for dispatch. Validate
input/page/encoded-request ceilings before effects. Bound response bytes and vector entries before accepting
or copying a readback; a small point count alone does not bound a vector response.

Readback has two different contracts:

- **Client-computed vectors:** compare actual returned values against known expected values under the selected
  backend's documented storage transformation, plus required names, shape, finite values and known payload.
- **Native Document inference:** compare exact IDs and all known immutable input/profile/filter/validity
  bindings; require the selected vector field and validate its returned sparse structure and finite values.
  The resulting coefficients are generated by Qdrant, not known from a local expected PointRecord. A valid
  zero-term vector is not evidence that the retained source was empty. Missing/malformed vectors are not
  success. Do not invent a vector digest or claim equality to an independently computed expected embedding.

This establishes observed storage shape/bindings and uses Qdrant as the trusted selected inference engine.
It is not independent proof that every coefficient represents the submitted text. Source evidence still
requires the real immutable reader and range checks. Retrieval correctness/quality is exercised with actual
text queries against the selected server/profile, not a digest echoed in payload. Unsupported inference
fails explicitly; there is no silent oracle/local-encoder fallback.

## Schema, scope and IDF

Schema binds the actual physical generation, selected named-vector parameters, required payload indexes,
retrieval options and resource/topology settings. Create missing indexes and verify the complete schema within
the attempt budget. Schema incompatibility builds a new generation; never silently adopts a foreign collection.
Local baseline is one node/shard/replica and serialized publisher. Strong ordering is a topology choice, not
an application-time order token and not a cure for a delayed old request submitted later.

Render scope into:

```text
base population = compatible selected sources + pinned validity + selected access partition
retrieval filter = base population + request-only narrowing
sparse leaf params.idf.corpus = base population
```

Qdrant 1.19 supports the independent IDF corpus. Require that feature when this profile needs it; do not fall
back to shard-global statistics. Apply it to each relevant sparse leaf. One compatible route uses one native
query; native fusion/grouping serves selected compatible branches. Preserve final server order. Top-k does
not prove complete absence and needs no unconditional exact count.

Index the corpus-filter fields. Statistics are shard-local and the corpus filter does not restrict returned
points; the retrieval filter still carries scope/validity. An empty IDF corpus removes the rarity signal rather
than falling back to global statistics; BM25 text queries also lose length normalization. Keep `idf.corpus`
equal to the selected base population. A zero-result response proves neither an empty corpus nor complete
absence. Determine known-empty from the published inventory; do not add per-query count/scan to diagnose IDF.
[Donor evidence](../../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#native-bm25-and-separate-statistics-scope)
records the documented limits and the separate population-work cost.

An explicitly selected empty set, or a known valid empty published inventory, returns the corresponding empty
result without Qdrant RPC; never omit the filter and widen to all sources. Unknown IDs are errors. Missing
metadata/collection or an incomplete inventory is not known-empty success. The exact operation reports only
its actual coverage. This replaces the earlier blanket 'empty scope fails' rule.

## Failure boundary

Before dispatch, validation/cancellation/expiry is a definite non-write. Read failure commits nothing. After
possible mutation dispatch, retain uncertainty. Preserve whether a terminal completed response was actually
received: completed-but-unverified is different from a request that may still run. Neither a receipt ID nor
one matching readback can settle every outstanding attempt. Publication owns that decision and keeps only
finite unfinished state; the bridge has no replay ledger in the target design.

Keep useful error classes: not found, already exists, auth, schema/identity conflict, resource limit,
transport, definite pre-dispatch failure and unresolved mutation. Do not flatten them into a generic receipt
error or infer successful cancellation from dropping a local future.

## Remove and verify

The runtime gate-token cut is implemented at `7d92fb8b`. Remove the remaining auth-binding ceremony, fixed
build admission, operations map and receipt ceiling, duplicated point/payload models, relative deadline
renewal, unconsumed mutation wrappers, revision-bound citation and vector-digest payload extensions, and
product-reachable oracle/shape fixtures with their actual caller cutovers. Keep UUID conversion, finite
validation, private SDK translation, relevant readback and structured errors.

After code check native ingestion/query/evidence end to end; old-view retrieval during a changed-vector/filter
publication; exact closure; native versus explicit-vector readback; empty/unknown scope; per-leaf filtered IDF;
response bounds; and delayed-dispatch recovery from the publication recipe. Minimal changed-owner check/Clippy
comes first. The gate-removal library check does not establish those runtime behaviors or a Windows result.

## Primary donor references

Source semantics checked 2026-10-21; IDF documentation rechecked 2026-10-23. Project pin remains 1.19.0.

- [Native Document input and generated-vector retrieval](https://qdrant.tech/documentation/inference/inference-bm25/)
- [Point replacement, insert-only and narrow-update semantics](https://qdrant.tech/documentation/manage-data/points/)
- [Filtered IDF, available since 1.19.0](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
- [Write ordering](https://qdrant.tech/documentation/scaling/consistency-guarantees/#write-ordering)
- [1.19.0 server update task](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/collection/src/collection/point_ops.rs)
