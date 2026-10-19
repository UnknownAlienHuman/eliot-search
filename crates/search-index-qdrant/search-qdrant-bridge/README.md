# search-qdrant-bridge

Private translation between Eliot Search and the workspace-pinned `qdrant-client`. The bridge owns concrete
collection, point, query and readback RPCs. Publication owns mutation recovery and visible-view commit;
query owns selected scope and result use; retained source plus `ViewManifestEntry` own evidence.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md) and
[runtime boundary](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).

## Verified public-main state

The useful vendor adapter exists but is not connected to the daemon product.

- `RealDataPlane` performs real Qdrant SDK RPCs. Repository search finds its constructors in bridge/live
  fixtures, the reclaimer live test and `BlockingRealQueryPlane`, not normal daemon publication/query.
- `BlockingRealQueryPlane` owns one reusable Tokio runtime but exposes only schema verify, sparse query,
  exact readback and count. Publication mutations require the separate async type.
- `query_filtered` accepts an already computed sparse `(index, value)` vector. It does not use Qdrant's
  native `Document` input for `qdrant/bm25`.
- `upsert_exact`, `close_exact` and `delete_exact` are real `wait=true` operations with readback, but there
  are no connected narrow vector/payload operations.
- `RealDataPlane.operations` is a process-local `operation_id → MutationReceipt` map. Restart loses it;
  successful calls consume its fixed lifetime capacity.
- `OpContext` holds a relative `Duration`. Each RPC receives the full duration again; multi-step collection
  creation can exceed the caller's intended total deadline. `collection_exists` is currently outside that
  timeout entirely.

The current runtime admission is larger than the data plane:

- `QualifiedGate` requires one exact Qdrant 1.19.0 Windows executable build/SHA, exact Rust client identity
  and thirteen live-probe outcomes before every real connection can be constructed;
- the gate's probe collection is used by tests but is not consulted when production routes are used;
- `QdrantApiKeyLease` carries process, creation marker, root, installation, owner epoch, artifact, endpoint,
  secret-reference/purpose and expiry equality fields, then asks an external provider to revalidate them
  before every RPC;
- the SDK already copied the API key into its client interceptor at connect time, so rotation/revocation
  still requires rebuilding the client; repeated callback ceremony does not remove that copy.

Keep the exact dependency pin until a deliberate upgrade, but remove qualification reports and equality
receipts from runtime construction. A selected build's live suite is evaluation/release evidence, not a
credential presented to the application it tested.

## Current schema and point-model defects

`CollectionSchema::validate` hard-codes one shard, strict mode, `wait_for_mutations` and strong ordering as
correctness floors. `create_collection` rejects every dense vector and always creates sparse-only schema.
These are one frozen qualification profile, not the Qdrant adapter boundary.

The current payload also mixes unrelated ownership:

```text
stable filter/validity fields
+ source_revision
+ unit_ordinal
+ payload_digest / identity_digest
+ vector_digest_<name>
```

This makes Qdrant both retrieval index and citation/digest database. `decode_vectors` reads actual sparse
values but takes each vector digest from payload; query nominations echo payload digests. Neither proves the
retained source excerpt.

The replacement Qdrant point contains only stable retrieval state:

```text
PointId128 + full stable identity evidence
occurrence incarnation / retrieval role / profile
stable source or grouping coordinates
selected filter fields
valid_from / valid_until view sequence
actual named vectors
```

For one immutable published view, `ViewManifestEntry` maps the point to exact retained source revision,
representation, logical occurrence and byte/line range. Reusing a stable point in a later view never mutates
an earlier citation.

## One daemon-owned runtime

```text
publication page ─┐
                  ├→ one Qdrant client + one Tokio runtime → Qdrant
query request ────┘
```

Construct it from the selected endpoint and optional real credential. Verify the current server capabilities
and exact collection generation/schema needed by the installation. If credentials rotate, rebuild the
client. Do not require a historical probe report, executable digest receipt or purpose lease on each RPC.

Use one application context with an absolute monotonic deadline and cancellation:

```text
check cancellation
remaining = deadline - now
remaining <= 0  → definite pre-dispatch timeout
otherwise       → timeout_at(deadline, RPC)
```

Reads commit nothing. A mutation failure after dispatch may have occurred and returns unresolved to the
publication cursor. The bridge keeps no replay ledger; publication's persisted page identity and actual
Qdrant readback own recovery.

## Use native Qdrant mechanisms

The pinned 1.19 client supports the mechanisms the product needs:

- server-side BM25 `Document` input for both stored document text and query text;
- explicit `idf.corpus` separate from the retrieval filter;
- `wait=true` for completed writes;
- full point upsert, selected vector update/delete, payload merge/delete/overwrite and exact point delete;
- native query prefetch, RRF/grouping for selected compatible Qdrant branches.

Official references:

- [server-side BM25](https://qdrant.tech/documentation/inference/inference-bm25/)
- [points, full replacement and narrow vector updates](https://qdrant.tech/documentation/concepts/points/)
- [payload merge/overwrite/delete semantics](https://qdrant.tech/documentation/concepts/payload/)
- [per-scope IDF population](https://qdrant.tech/documentation/tutorials/multiple-partitions/)
- [hybrid/prefetch/RRF/grouping](https://qdrant.tech/documentation/search/hybrid-queries/)
- [write ordering choices](https://qdrant.tech/documentation/scaling/consistency-guarantees/)

A full upsert replaces the authoritative point and unspecified vectors do not survive. Use narrow vector and
payload operations when unrelated fields must remain. `set_payload` merges; it is not overwrite.

The local baseline is one node, one replica and one serialized publisher. Keep `wait=true` plus exact
readback; do not hard-code strong ordering as a correctness proof. Another measured topology may select a
stronger policy explicitly.

## Scope and ranking

Resolve one non-widening scope from the pinned `PublishedView`. Separate:

- base population: compatible selected sources/access partition/view validity, used for retrieval and each
  sparse leaf's `idf.corpus`;
- query-only constraints: narrower filters which must not silently narrow the intended IDF denominator.

Current `IdfScope::ScopedToRetrieval` clones the complete retrieval filter. Replace that binary enum with the
actual base-population filter plus optional query constraints. Empty/unknown scope fails before RPC.

Baseline lexical search sends Qdrant `Document(query, "qdrant/bm25", options)` and preserves Qdrant order.
Optional compatible dense/sparse branches use native prefetch/fusion/grouping with explicit limits; a later
merge cannot recover candidates omitted by too-small prefetches. A hit is only a nomination until pinned
manifest lookup and retained-byte validation succeed.

## Mutation and generation semantics

Expose exactly the operations publication and query use:

- create/verify/delete one physical collection generation;
- complete authoritative point upsert;
- selected vector update/delete;
- payload merge/delete/overwrite with explicit names;
- exact point read and exact point delete;
- native BM25/hybrid/grouped query and explicit count/scroll when requested.

Every publication page awaits completion and reads back the actual affected vectors, payload fields,
validity and schema. Adding a named-vector schema does not populate existing points. Incompatible schema or
retrieval profile builds a new physical generation; publication switches `CurrentPublishedView` only after
full readback, then deletes the old collection after its final view guard.

## Remove after caller cutover

- `QualifiedGate`, `LiveProbeReceipt` and exact artifact/client identity as runtime constructor inputs;
- `QdrantConnectionBinding`/API-key lease equality ceremony for the ordinary local path;
- process-local mutation receipt/replay map and `max_operation_receipts` lifetime cap;
- relative per-RPC deadline reset and fixed connect/mutation timeouts outside the application context;
- hard-coded sparse-only, one-shard, strict-mode and strong-ordering correctness floors;
- revision/unit citation fields and payload `vector_digest_*` values;
- duplicate bridge point/payload/filter types after shared current types migrate;
- product-reachable in-memory oracle, qualification source analyser and shape fixtures.

Keep private vendor conversion, stable structured errors, real schema/readback, one runtime and the selected
live behavior suite. Verify through the connected retained-file → publication → Qdrant BM25 → pinned manifest
→ retained excerpt path; constructor/probe tests alone are not product integration.
