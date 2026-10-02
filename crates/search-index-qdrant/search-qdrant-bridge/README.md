# search-qdrant-bridge

**C15 — Qdrant data-plane bridge.**

**Status:** implemented vendor-neutral oracle, exact qualification gate and live/real Qdrant transport.
Indexed product admission remains receipt-gated; no automatic upgrade or fallback is permitted.

This package owns qualified Qdrant collection, point and query operations behind Eliot-owned types.
Qdrant is the only search/index database. The in-memory bridge is a bounded behavioral oracle only and
is never a production fallback.

## Owns

- capability and collection-schema probes;
- exact S9.5 payload/index translation;
- strict-mode index and S10.3 filter translation;
- exact point mutation/readback/delete transport;
- filtered query/count/scroll operations with mandatory retrieval/IDF filter identity;
- private vendor-type translation;
- client/server qualification identity checks.

## Must not own

- executable/process/ACL/Job Object lifecycle;
- secret storage;
- recipe, access, publication or result semantics;
- vendor types in public ports;
- automatic download, upgrade or silent fallback.

## Canonical point and eligibility boundary

`PointPayload` is the closed S9.5 payload: installation and collection generation, projection
membership, immutable access/scoring partitions, source/revision/representation/unit identity, full
point identity digest, scoring document, profile set, modality/format metadata and epoch validity.
Source-membership arrays/names, ACL subjects, paths, source/query text, payload digests and vector
digests are not stored in Qdrant payload.

Expected payload/vector digests remain in the immutable projection manifest. Exact readback returns the
closed typed payload and actual named-vector values for verification by the publication/readback owner.
Qdrant payload and scores are never source evidence.

S8.2 policy/scoring changes mint new immutable `AccessPartitionId` and `ScoringPartitionId` values and a
new projection publication. Before bridge dispatch, the access owner compiles restrictive
shadow/deny/purge/abandoned state into the allowed projection-membership set. The bridge renders one
S10.3 base filter over installation, collection generation, projection memberships, partitions,
profile set and epoch, then reuses it verbatim for retrieval, `idf.corpus`, exact count and scroll. The
production API cannot request global or caller-substituted IDF.

## Real data plane

Connection requires an executed qualified gate and rechecks the exact server identity. Collection
creation writes the committed `CollectionSchema` digest into one versioned collection-metadata key,
creates the collection with strict admission disabled, installs all 19 mandatory S9.5 payload indexes
with exact UUID/keyword/integer types, enables strict mode, and reads the schema back. Admission requires
exact metadata, topology, vector, index-type and strict-setting equality; structurally compatible but
foreign schema identity is rejected.

Mutations use explicit IDs, `wait=true`, strong ordering and exact readback. Upsert performs the S11.2
collision guard before dispatch. Close and delete use only exact IDs and generation-bound preflight;
broad-filter compensation is absent.

Every read response is bounded and shape-checked. Duplicate, unexpected, off-filter, foreign-generation,
unknown-field or incorrectly typed responses fail closed. Filtered nominations remain untrusted until
exact point readback, authoritative projection-membership resolution and source validation.

One finite monotonic operation budget covers validation, preflight, dispatch and readback. Before a
possible write, expiry is definite. After a write may have reached Qdrant, timeout/cancellation/readback
failure returns `QDRANT_MUTATION_OUTCOME_UNKNOWN` for explicit recovery.

The live disposable-server harness is qualification-only. Product process ownership remains in
`search-qdrant-supervisor`; daemon composition consumes a qualified endpoint rather than adopting the
fixture lifecycle.

The dependency and upgrade boundary is defined in
[`docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).

Validate from the repository root:

```powershell
cargo run --locked -p xtask -- validate qdrant-boundary --json
cargo clippy --locked -p search-qdrant-bridge --lib -- -D warnings
cargo test --locked -p search-qdrant-bridge --test live_server_module_ownership
cargo test --locked -p search-qdrant-bridge --test live_probe_module_ownership
cargo test --locked -p search-qdrant-bridge --test real_query_module_ownership
cargo test --locked -p search-qdrant-bridge --test real_codec_module_ownership
cargo test --locked -p search-qdrant-bridge --test real_schema_module_ownership
cargo test --locked -p search-qdrant-bridge --test real_error_schema_module_ownership
cargo test --locked -p search-qdrant-bridge --test real_identity_module_ownership
```

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
