# Qdrant bridge — source audit and replacement map

The [implementation guide](../../../docs/implementation/README.md) selects one real Qdrant owner. Existing
public models, qualification receipts and test-oracle APIs are inventory, not compatibility requirements.

## Current source inventory

| Source/API | Actual behavior and defect |
|---|---|
| `src/real.rs` / `RealDataPlane` | Owns real SDK client, auth lease, qualification gate, in-memory schema cache and process-local mutation receipt map. The useful client is coupled to three unrelated authorities. |
| `real/connect_schema/connect.rs` | Requires exact `QualifiedGate`, supervisor binding and API-key lease. Uses fixed 5/10/15-second transport/connect checks rather than the caller's absolute context. |
| `qualified/**` | Freezes Windows x64 server 1.19.0 build/SHA, Rust client source/checksum and thirteen mandatory live probes into a runtime admission token. `QualifiedGate.collection` is exercised by tests but not used by normal routes. |
| `auth.rs` | About 15.8 KiB. Carries process/root/installation/owner/artifact/endpoint/secret equality fields and callback-only key access. The SDK still retains its own copied key after client construction. |
| `real/blocking.rs` | Reuses one Tokio runtime but exports reads only. It refuses calls from another Tokio runtime and cannot serve publication mutations. |
| `real/ledger.rs` | `operation_id → MutationReceipt` map is memory-only, lost on restart and capped for the process lifetime. |
| `OpContext` | Relative `Duration`; every RPC gets the full value. Multi-RPC operations renew time. `collection_exists` in collection creation has no context timeout. |
| `schema.rs` / `create.rs` | Schema validation requires one shard, strict mode, wait and strong ordering; collection creation accepts sparse vectors only. These are one frozen test profile. |
| `mutation.rs` / payload codec | `PointPayload` includes revision/unit citation fields and payload/identity digests; payload also stores `vector_digest_<name>`. Qdrant becomes a duplicate evidence store. |
| `query_filtered` | Accepts precomputed sparse indices/values; supports only global IDF or a clone of the complete retrieval filter. Native text/BM25 and separate base-population/query-only filters are absent. |
| mutation methods | Full upsert, validity close and exact delete exist. Narrow vector update/delete and explicit payload merge/delete/overwrite are absent. All writes hard-code strong ordering. |
| `QdrantBridge` | In-memory oracle with parallel schema/query/mutation behavior. Tests only; never a product fallback. |
| callers | Searches find real plane construction in bridge/live/reclaimer tests and the blocking wrapper. Daemon indexed query explicitly says it does not call the real plane. |

The qualification/live fixture tree is useful release evidence, but it is not the running application.
Do not connect the product by fabricating a `QualifiedGate` or retaining the whole evidence protocol.

## Target owner and context

One daemon-owned adapter serves publication and query:

```text
struct QdrantRuntime {
    client: qdrant_client::Qdrant,
    runtime: tokio::runtime::Runtime, // only if synchronous composition remains
}

struct RequestContext {
    deadline: std::time::Instant,
    cancelled: Arc<AtomicBool>,
}
```

At every blocking/async boundary compute `deadline.saturating_duration_since(Instant::now())`; never create a
fresh relative budget for a nested operation. One collection creation may perform several RPCs, but all
share the same deadline.

Connection inputs are selected endpoint, optional actual API key and ordinary transport settings. Startup
checks live server identity/capabilities and verifies the exact selected physical generation. If the API key
rotates, rebuild the client. Do not revalidate unrelated root/owner/artifact digests on every Qdrant call.

The exact crate/server pin may stay until an explicit upgrade. Qualification suites run when establishing a
release claim; their report is not passed back as a constructor credential.

## Target application API

Concrete names may vary, but keep one small boundary:

```text
connect(config, context) -> QdrantRuntime

ensure_generation(route, schema, context)
verify_generation(route, schema, context) -> GenerationReadback
delete_generation(route, context)

upsert_full(route, points, context) -> PointReadback
update_vectors(route, changes, context) -> VectorReadback
delete_vectors(route, ids_and_names, context) -> VectorReadback
set_payload(route, ids, fields, context) -> PayloadReadback
delete_payload(route, ids, keys, context) -> PayloadReadback
overwrite_payload(route, ids, payload, context) -> PayloadReadback
delete_points(route, ids, context) -> AbsenceReadback
read_points(route, ids, selection, context) -> PointReadback

query(route, resolved_scope, query_input, query_plan, context) -> nominations
count(route, resolved_scope, context) -> ExactCount       // explicit operation only
scroll(route, resolved_scope, cursor, context) -> Page    // explicit operation only
```

The bridge does not accept an operation digest and return a durable/replay receipt. `PendingPublication`
identifies the immutable page. If dispatch may have happened, publication calls the relevant readback and
decides whether to advance or retry.

## Current code to reuse

Preserve and simplify:

- private Qdrant request/response translation;
- full UUID/128-bit point conversion;
- exact-ID validation, finite vector values and response-shape checks;
- `wait=true` completion checks;
- exact point/vector/payload/schema readback;
- stable mapping of vendor failures into application error classes;
- one reusable Tokio runtime where synchronous daemon composition needs it;
- payload-index and validity-filter rendering after the stable point model changes;
- real live fixtures that establish behavior during a selected release/upgrade.

Do not preserve caller-produced digests/receipts merely because existing methods expose them.

## Current code to delete or move

| Source | Action after consumers move |
|---|---|
| `real/ledger.rs`, `BridgeMutation`, `MutationReceipt` | Delete. Publication artifact/page identity owns recovery; Qdrant state owns readback. |
| `qualified/**` runtime exports | Remove from the product constructor. Keep only minimal upgrade/evaluation tooling outside the hot package if still useful. |
| `auth.rs` binding/lease protocol | Replace with selected endpoint plus optional actual credential loading. Rotation rebuilds the client. |
| `capability.rs` / in-memory oracle capability receipts | Delete with oracle/product adapter; retain direct live schema/capability observations actually consumed. |
| `StrictnessFloors` | Delete. Topology, strict resource limits and write ordering are selected configuration/profile fields, not universal correctness proofs. |
| `source_revision`, `unit_ordinal`, payload/vector digests | Move exact revision/range to `ViewManifestEntry`; compare actual vectors/readback rather than echoed digest fields. |
| `BlockingRealQueryPlane` | Rename/extend to the one full runtime or replace with direct daemon async composition. Do not create a second write runtime. |
| source analyser / package-shape tests | Remove under the adapter-boundary cleanup; compile actual consumers and review the public API. |

## Stable point and published evidence

Change planner, point identity, bridge and query together. One point carries stable retrieval state:

```text
point_id: UUID / full 128-bit value
identity evidence sufficient to reject collision
occurrence incarnation
retrieval role/profile/input identity
stable source/group/filter coordinates
valid_from_sequence
valid_until_sequence?
named vectors
```

A view-manifest entry carries:

```text
view_id / sequence
point_id
exact source revision ObjectRef
representation / logical occurrence
byte/line range and verification data
```

Qdrant nominations may include the point ID and minimal stable routing metadata. They do not need payload
hashes pretending to prove the source. Query resolves the point in the pinned manifest, then verifies the
retained object/range.

## Schema and generation creation

`CollectionSchema` should describe actual selected behavior:

- physical route/generation;
- sparse/dense/multivector names and their real parameters;
- native BM25 model/options for document and query roles;
- only payload indexes used by scope/grouping;
- selected shard/replica/write-ordering/resource limits.

Local baseline: one shard, one replica, serialized publisher, `wait=true`, exact readback, default/weak
ordering. Stronger ordering is an explicit topology choice, not a hard-coded floor.

Generation creation is idempotent by observed state:

1. inspect exact collection existence/schema within the original deadline;
2. create missing collection;
3. create missing selected indexes;
4. verify complete target schema;
5. return the actual generation readback.

Any failure after a possible effect is unresolved. Publication keeps the same pending generation operation
and repeats observation; no bridge receipt ledger is needed.

## Native lexical ingestion/query

Use the Rust client's `Document` form for the baseline lexical vector:

```text
stored document:
  Document(text = unit.retrieval_text,
           model = "qdrant/bm25",
           options = published_document_options)

query:
  Document(text = request.query,
           model = "qdrant/bm25",
           options = published_query_options)
```

Document/query roles and options belong to the published retrieval profile. `search-lexical` may contribute
bounded identifier expansions to document text; do not run a second local encoder to predict token IDs or a
vector digest.

Current `StoredVector` requires explicit sparse values and a fixed `dimensions` upper bound. Replace it for
native document inputs; retain explicit vectors only for optional client-computed branches.

## Scope and IDF

Render a `ResolvedScope` into two layers:

```text
base population filter
  compatible selected sources
  selected access partition when enabled
  pinned view validity interval

query filter
  base population
  + optional request-only narrowing constraints
```

Each BM25/sparse leaf uses the base population as `params.idf.corpus`. Retrieval uses the query filter.
Qdrant documents `idf.corpus` as independent from retrieval filtering; cloning all future query-only
constraints would change the denominator. Empty base population fails before RPC.

For one physical route/profile, use one native query. Split only across incompatible routes/profiles. Native
prefetch/RRF/grouping owns compatible Qdrant branches. Preserve Qdrant result order and expose truthful
limits; ordinary top-k is not complete-negative evidence and does not need `count_exact`.

## Mutation semantics

| Application change | Qdrant operation |
|---|---|
| new point or complete authoritative replacement | full upsert with all required vectors/payload |
| add/replace selected named vectors | `update_vectors` |
| remove selected named vectors | `delete_vectors` |
| merge selected payload fields | `set_payload` |
| remove selected payload keys | `delete_payload` |
| replace complete payload while preserving vectors | `overwrite_payload` |
| retire point from future view | close validity field before view commit, exact delete only after guards drain |
| retire replaced generation | delete whole collection after final old-view guard |

A full upsert of an existing point replaces the point; omitted vectors are not implicitly preserved. Read
back exactly the state the operation promises, including absence for deletes. Narrow operations must not
erase unrelated vectors or retain stale payload keys.

## Failure mapping

- validation/cancellation/expired deadline before dispatch: definite failure;
- read/query transport failure: definite operation failure, because no mutation was made;
- mutation timeout/transport/cancellation after possible dispatch: `OutcomeUnknown`;
- completed acknowledgement followed by failed readback: still unresolved until observation succeeds;
- schema or point readback conflict: block the publication target; do not convert it into success from an
  operation ID or digest.

Vendor status text remains private, but do not flatten every useful status into one generic receipt error.
Preserve enough structured class for caller action: not found, already exists, schema conflict,
authentication, resource limit, transport and unknown mutation outcome.

## Official donor mechanics

- [BM25 `Document` input](https://qdrant.tech/documentation/inference/inference-bm25/)
- [point idempotence/full replacement/narrow vector updates/`wait=true`](https://qdrant.tech/documentation/concepts/points/)
- [payload merge/overwrite/delete](https://qdrant.tech/documentation/concepts/payload/)
- [independent IDF corpus](https://qdrant.tech/documentation/tutorials/multiple-partitions/)
- [native hybrid/prefetch/RRF/grouping](https://qdrant.tech/documentation/search/hybrid-queries/)
- [weak/medium/strong ordering trade-offs](https://qdrant.tech/documentation/scaling/consistency-guarantees/)

Use these mechanisms; do not import a second index, local BM25 engine, mutation journal or capability
framework.

## Verification

Exercise through the connected daemon:

1. create/verify a physical generation within one absolute deadline;
2. publish real text through native BM25 and find it under the selected base IDF population;
3. resolve the returned point through the pinned manifest to retained bytes;
4. preserve unrelated vectors/payload fields under narrow updates;
5. recover an unknown write by actual exact readback after restart;
6. switch generations while an old view remains queryable;
7. delete exact retired IDs or an old generation only after guards drain.

Run minimal Rust 1.98 check/Clippy for the changed bridge and actual publication/query/daemon consumers after
code. Broad qualification and fault suites come after the connected product path exists; historical probe
receipts do not substitute for it.
