# Function contract — `search-qdrant-bridge`

**Status:** W3/P05 data-plane contract; no server/client pair is accepted yet.

Vendor types remain private. Every public operation consumes/returns Search contract or port-support
types and binds an accepted `QdrantCapabilityReceipt`.

## Connection and admission

### `connect(endpoint, auth_lease, supervisor_receipt) -> Result<QdrantBridge, BridgeError>`

Requires exact supervisor process identity, installation incarnation, loopback endpoint and bounded
secret lease. It never discovers or starts a process.

### `probe_capabilities(disposable_route, probe_manifest, context) -> Result<QdrantCapabilityReceipt, BridgeError>`

Executes every probe in `qualification/qdrant/probes.toml`: build/auth identity, one shard, signed-i64
range behavior, missing upper bound under `must_not`, sparse IDF, independent `idf.corpus`, strict mode,
payload indexes, `wait=true`, strong ordering, exact count/readback and named sparse vectors.
Any missing probe rejects indexed admission.

### `create_candidate_collection(schema, context) -> Result<CollectionCreateReceipt, BridgeError>`

Creates one new opaque physical generation only. It creates mandatory payload indexes before ingest,
then enables the exact strict-mode restrictions and verifies the resulting schema digest.

### `verify_collection_schema(route, expected) -> Result<SchemaReceipt, BridgeError>`

Reads back topology, named vectors, payload indexes, strict-mode limits and schema identity. Version
strings alone are insufficient.

## Exact mutation operations

### `upsert_exact(batch, mutation, context) -> Result<MutationReceipt, BridgeError>`

Uses explicit point IDs, `wait=true` and strong ordering. Same mutation identity plus same canonical
batch is idempotent. Same identity plus different input is rejected.

### `close_exact(ids, valid_until_epoch, mutation, context) -> Result<MutationReceipt, BridgeError>`

Updates only the exact ID list. Broad-filter closure is absent from the correctness API.

### `delete_exact(ids, mutation, context) -> Result<MutationReceipt, BridgeError>`

Deletes only exact IDs for ordinary reclaim/compensation. It does not create a security-purge receipt.

### `readback_exact(ids, context) -> Result<BoundedPointReadback, BridgeError>`

Returns exact identity/payload/vector digests and explicit missing/unexpected IDs.

### `count_exact(filter, context) -> Result<ExactCount, BridgeError>`

Permitted only for the closed accepted filter AST and indexed fields.

## Query operations

### `query_filtered(request, context) -> Result<BoundedCandidateStream, BridgeError>`

Compiles the accepted vendor-neutral eligibility AST privately. Retrieval and `idf.corpus` receive the
same canonical base eligibility plan; unindexed or capability-unsupported filters fail rather than
scan. Results are bounded nominations, never evidence.

## Timeout, cancellation and recovery

Cancellation is checked before dispatch and between bounded pages/batches. A timeout after a mutation
may have committed; callers receive `OUTCOME_UNKNOWN` and must resolve through exact readback and the
same mutation identity. The bridge never converts timeout into a definite rollback claim.

## Configuration operations

Implements `config/sections/qdrant_data.md`. Transport batch/time settings may change live only within
accepted capability limits. Strict mode, wait-for-mutations and strong ordering are fixed correctness
floors.

## Required fixtures

Disposable full capability suite; strict unindexed retrieve/update rejection; payload indexes before
ingest; signed-i64/missing-field filter; filtered-IDF noninterference; wait/strong/readback; exact
delete; timeout unknown-outcome recovery; vendor-type API guard; no process lifecycle duplication.
