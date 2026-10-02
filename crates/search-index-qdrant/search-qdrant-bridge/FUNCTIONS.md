# Function contract — `search-qdrant-bridge`

**Status:** W3/P05 data-plane contract; indexed product admission remains receipt-gated.

Vendor types remain private. Every public operation consumes or returns Eliot-owned contract or
package-owned support types and binds an accepted `QdrantCapabilityReceipt`.

## Connection and admission

### `connect(endpoint, auth_lease, supervisor_receipt) -> Result<QdrantBridge, BridgeError>`

Requires exact supervisor process identity, installation incarnation, loopback endpoint and bounded
secret lease. It never discovers or starts a process.

### `probe_capabilities(disposable_route, probe_manifest, context) -> Result<QdrantCapabilityReceipt, BridgeError>`

Executes every mandatory probe in `qualification/qdrant/probes.toml`: build/auth identity, one shard,
signed-i64 range behavior, missing upper bound under `must_not`, sparse IDF, independent `idf.corpus`,
strict mode, payload indexes, `wait=true`, strong ordering, exact count/readback and named sparse
vectors. Any missing probe rejects indexed admission.

### `create_candidate_collection(schema, context) -> Result<CollectionCreateReceipt, BridgeError>`

Creates one new opaque physical generation only. It writes the exact committed schema digest into one
versioned collection-metadata key, creates the exact S9.5 payload-index set and types before enabling
strict mode, then verifies metadata, topology, vectors, every index type and strict-mode floors. Unknown
indexes, unknown collection metadata or incompatible payload/filter changes require a new collection
generation.

### `verify_collection_schema(route, expected) -> Result<SchemaReceipt, BridgeError>`

Reads back the exact schema-identity metadata, topology, named vectors, payload index names/types and
strict settings. Version strings or structural similarity alone are insufficient, and an existing
collection is never adopted without exact equality to the caller-supplied committed expectation.

## Point and filter contracts

`PointPayload` is the exact closed S9.5 payload. It contains typed installation, collection,
projection-membership, access/scoring partition, source/revision/representation/unit, point identity,
scoring-document, profile-set, modality/format and epoch coordinates. It contains no source-membership
array/name, ACL subject, display path, source/query text, payload digest or vector digest.

Access/scoring policy changes mint new immutable partition identifiers under S8.2. Restrictive
deny/shadow/purge/abandoned fences are compiled by the access owner into the allowed projection
membership set before the bridge is called. `EligibilityFilter` renders the one S10.3 base predicate
over installation, collection generation, allowed projection memberships, access/scoring partitions,
profile set and visible epoch. Retrieval, `idf.corpus`, exact count and scroll use that same value. The
production query API has no global or caller-substituted IDF-population mode.

Expected payload and named-vector digests remain in the immutable projection manifest. Exact bridge
readback returns the closed typed payload and actual vector values so publication/readback owners can
verify that manifest; Qdrant payload is never source evidence.

## Exact mutation operations

### `upsert_exact(batch, mutation, context) -> Result<MutationReceipt, BridgeError>`

Uses explicit point IDs, `wait=true` and strong ordering. Before dispatch it reads any existing IDs and
compares the full 256-bit point identity plus every independently represented S11.1 identity coordinate.
A mismatch returns `POINT_ID_COLLISION` and does not overwrite. Same mutation identity plus the same
canonical batch is idempotent; the same identity plus different input is rejected.

### `close_exact(ids, valid_until_epoch, mutation, context) -> Result<MutationReceipt, BridgeError>`

Updates only the exact ID list after exact generation-bound preflight. Broad-filter closure is absent
from the correctness API.

### `delete_exact(ids, mutation, context) -> Result<MutationReceipt, BridgeError>`

Deletes only exact IDs for ordinary reclaim/compensation after generation-bound preflight and proves
absence through readback. It does not create a security-purge receipt.

### `readback_exact(ids, context) -> Result<BoundedPointReadback, BridgeError>`

Returns closed typed S9.5 payloads, actual named-vector values and explicit missing/unexpected IDs.
Unknown payload fields, wrong field types, duplicate responses or foreign collection generations fail
closed.

### `count_exact(filter, context) -> Result<ExactCount, BridgeError>`

Permitted only for the closed admitted S10.3 filter and exact indexed collection generation.

## Query operations

### `query_filtered(request, context) -> Result<BoundedCandidateStream, BridgeError>`

Compiles the accepted vendor-neutral eligibility filter privately. Retrieval and `idf.corpus` receive
the same canonical base filter; unindexed or capability-unsupported filters fail rather than scan.
Returned scores and point identities are bounded nominations, never evidence. Every response payload is
decoded as the closed S9.5 type and rechecked against the exact filter before the nomination is returned.

## Timeout, cancellation and recovery

One finite monotonic operation budget covers validation, preflight, dispatch and readback. Cancellation
or expiry before any possible write is definite. Once a mutation may have reached Qdrant, timeout,
cancellation or unusable readback returns `QDRANT_MUTATION_OUTCOME_UNKNOWN`; callers resolve it through
exact readback and the same mutation identity.

## Configuration operations

Implements `config/sections/qdrant_data.md`. Transport batch/time settings may change live only within
accepted capability limits. Strict mode, wait-for-mutations and strong ordering are fixed correctness
floors.

## Required fixtures

Exact S9.5 payload field-set/unknown-field rejection; exact schema-digest metadata binding; exact
19-index schema/type equality; disposable full capability suite; strict unindexed retrieve/update
rejection; signed-i64/missing-field filter; filtered-IDF noninterference; retrieval/IDF filter identity;
wait/strong/readback; point collision non-overwrite; exact close/delete; deadline/unknown-outcome
recovery; vendor-type API guard; no process lifecycle duplication.
