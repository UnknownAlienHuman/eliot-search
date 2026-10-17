# Function contract — `search-qdrant-bridge`

**Status:** operational endpoint probing is implemented; release qualification remains a separate gate.

Vendor types remain private. Operational composition obtains its private transport gate only through actual bridge-owned RPC observations; lower-level compatibility admission is not execution proof.

## Connection and admission

`BlockingRealQueryPlane::connect_and_probe(endpoint, supervisor, auth_lease, context, limits, probe_rpc_timeout)` is the operational constructor. It uses the already-supervised loopback endpoint, verifies protected collection-list access, executes all 13 mandatory bridge probes with one original bounded request context, and admits the real plane only after exact temporary-collection cleanup and absence readback. Each RPC is bounded by the smaller of the original remaining time and the supplied timeout. Initial health reads may retry finitely within the same attempt and shared RPC/response budget; mutations remain single-attempt. It returns the plane with an Eliot-owned probe report. It does not start or stop Qdrant. Unknown mutations return their exact attempt and mutation identities and are never retried or cleaned up blindly.

This establishes only that this endpoint passed the bridge's operational probe set. It is not release qualification, artifact qualification, or installed-product evidence. The older `BlockingRealQueryPlane::connect` path accepts a caller-supplied `QualifiedGate` for transport compatibility; that gate is not proof that probes executed and must not be used for operational admission.

### `BlockingRealQueryPlane::connect(endpoint, supervisor, auth_lease, gate, limits)` (compatibility path)

Accepts a caller-supplied `QualifiedGate` and validates the exact supervisor process identity, installation incarnation, loopback endpoint and bounded secret lease. It never discovers or starts a process, and the supplied gate does not prove that probes ran.

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

### Canonical indexed-point validity restoration

`RealDataPlane::restore_indexed_validity_exact(route, original, close_attempt_until, context)` restores
the original upper validity bounds after one exact close attempt. It performs an unfiltered full S11
payload/vector preflight, accepts only the original bound or that attempt's exact bound, and sends at
most one direct generated Qdrant unary. A missing bound is restored by deleting the payload key; a
present original bound is restored with `set_payload`. The original request context, endpoint, and
purpose-bound key lease govern preflight, mutation, and exact final readback. An unknown RPC outcome
returns `MutationOutcomeUnknown`; a later invocation can resolve an already-applied restore as
`AlreadyRestoredVerified` without replaying the mutation.

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
delete; timeout unknown-outcome recovery; vendor-type API guard; no process lifecycle duplication. Operational startup probes use generated one-attempt RPCs with a finite per-message decode cap and a shared worst-case response-byte reservation. Exact fixture readback compares the complete payload, named-vector set, sparse indices and float bits; any mismatch blocks the executed gate.
