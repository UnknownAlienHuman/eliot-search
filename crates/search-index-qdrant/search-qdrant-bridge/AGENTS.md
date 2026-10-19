# Working on search-qdrant-bridge

Read the root [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source map](FUNCTIONS.md) and
[current Qdrant documentation](https://qdrant.tech/documentation/).

Use the workspace-pinned `qdrant-client` privately in this package. Implement one daemon-owned client/runtime
for both publication and query; do not add another wrapper/runtime for writes.

The current code contains real RPC translation mixed with runtime qualification and receipt machinery. Keep
private vendor conversion, exact schema/point readback, structured errors and finite bounds. Remove:

- `QualifiedGate`/probe reports as constructor credentials;
- process/root/artifact/secret equality receipts on every RPC;
- process-local mutation replay ledger;
- relative per-RPC deadline renewal;
- sparse-only, strict-mode and strong-ordering universal floors;
- revision/range citation data and vector digest echoes in payload.

Connect from selected endpoint plus optional actual credential. A key rotation rebuilds the SDK client. Run
live qualification when establishing a release/upgrade claim; normal startup checks the current service and
selected collection schema directly.

Carry one absolute deadline/cancellation context through every RPC. Before mutation dispatch, timeout is
definite. After a request may have reached Qdrant, return unresolved to publication and let its persisted
page cursor inspect actual state. The bridge owns no idempotency history.

Use native Qdrant operations with their exact semantics:

- `Document` input for stored/query `qdrant/bm25` text;
- full upsert only for complete authoritative replacement;
- `update_vectors` / `delete_vectors` for selected vectors;
- payload set/delete/overwrite without conflating merge and replacement;
- `wait=true` and exact relevant readback;
- exact-ID or whole-generation cleanup;
- native prefetch/RRF/grouping for compatible Qdrant branches.

Resolve base selected scope separately from query-only narrowing. The base population supplies
`idf.corpus`; the query filter may be narrower. Do not clone every future query-only predicate into IDF.

Qdrant points contain stable retrieval state. Exact source revision, representation and range live in the
pinned `ViewManifestEntry`; a hit remains a nomination until retained evidence is verified.

Use one-node/one-replica/default ordering for the baseline serialized publisher. Select stronger ordering or
another topology only from a measured profile. Do not preserve the old P18 qualification/receipt API.

After code, run minimal Rust 1.98 check/Clippy for this package and actual daemon/publication/query consumers.
Do not spend the implementation phase maintaining oracle/shape/qualification tests for APIs being deleted;
broad tests follow the connected product.
