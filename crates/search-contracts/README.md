# search-contracts

Shared types and small validation/encoding functions used by real producers and consumers.
Read the [implementation guide](../../docs/implementation/README.md#4-simplify-and-connect-the-query)
for the application path and the [package instructions](AGENTS.md) for changes here.

## Existing code and the useful contract

[evidence.rs](src/evidence.rs) already contains `RetrieveEvidenceRequestV2`,
`OrientScopeRequestV2` and their result/card structures. [lib.rs](src/lib.rs) exposes shared source,
revision, scope, coordinate, budget and indexed types. Their presence does not establish a connected
handler or working retrieval.

Keep the data needed by that handler:

| Data | Purpose |
|---|---|
| Selected source scope, question and requested result kind | Say what to search and what the caller wants to understand. |
| Published source view and actual profile | Use one coherent revision/index interpretation. Resolve authoritative values in the application. |
| Finite request/output limits and cancellation/deadline inputs | Bound useful work; ordinary retrieval does not require a full-scan budget. |
| Source/content-revision identity, location/range and excerpt | Reopen and verify the source behind a result. Source-record revision is not content revision. |
| Freshness, unsupported/missing-input gaps and optional continuation | Explain limits of the result and allow bounded further reading. |

Change the real request/result model and its decoder, planner, handler and encoder together.
The current V1 and V2 families can be replaced by one local format. Their old names, eleven-recipe
registry, fixture bytes and delivery-wave status do not require compatibility or a third family.

Remove mandatory plan IDs/fingerprints, result fingerprints, ranking traces, hashed group IDs and durable
handles where no selected consumer needs them. Ordinary top-k results are non-exhaustive; exact counts
and absence claims belong to an explicitly requested exact check. Keep the actual scope/revision/range
connection rather than filling obsolete constructors with synthetic domains, grants or digests.

## Serialization

[canonical.rs](src/canonical.rs) implements the current closed `CanonicalValue` and JSON/CBOR codecs.
That implementation is existing code, not a requirement to route every local record through a custom
canonical value tree.

Use typed Serde serialization for ordinary request/result/event records when implementing the new
exchange. Add direct dependencies to the existing owners. This crate's [Cargo.toml](Cargo.toml)
currently declares only `blake3` and `sha2`; transitive entries in Cargo.lock do not mean Serde is
integrated here. Bound the incoming frame before decoding and use meaningful semantic size checks.
Do not introduce another DTO/codec package merely to avoid changing real consumers.

An identity or profile that actually depends on canonical bytes needs a specified encoding. Other
records do not. Old golden bytes are not a compatibility promise: remove retired encoders and their
exclusive fixtures with the format. A new format may reject an old root before writes, leaving it
intact; no legacy decoder, profile migration or export prerequisite follows from a byte change.
Never reinterpret existing stored bytes under a new identity.

## Hashes with a concrete purpose

[digest.rs](src/digest.rs) provides shared real BLAKE3/SHA-256 computation. The existing API distinguishes
domain-framed canonical/raw input from unframed exact bytes:

- `blake3_canonical` / `sha256_canonical` hash the selected canonical encoding.
- `blake3_raw` / `sha256_raw` hash exact payload bytes with domain framing.
- `blake3_exact_bytes` computes standard unframed BLAKE3 for actual source bytes.
- `blake3_canonical_array` and its usage variant stream finite rows where a retained artifact actually
  needs that identity.

Use these owners where content identity or integrity is required. Decode-only digest wrappers and
matching digest fields do not prove that bytes were hashed. Verify actual source/artifact bytes in
their owner. Do not assign hashes to every local request, configuration, plan or state transition.

Retain useful algorithm/preimage vectors and bounded streaming tests for functions still used.
Delete unused hash producers with their consumers. Per-site ledgers, source-file hashes, issue-state
classifications and an unchanged golden catalogue are not integration requirements.

## Ownership and verification

Contract code performs no filesystem, process, network, redb or Qdrant I/O and owns no mutable runtime
state. Shared types earn their place through real consumers; future capability names do not need
records, registries or a lifecycle model here.

Compile changed production owners and consumers, then check actual request/result behavior, size
bounds, exact byte identity and truthful partial results. W0/G0 gates, P00/Wave-1 freezes, mandatory
contract-change issues and source-line quotas no longer govern this replacement.
