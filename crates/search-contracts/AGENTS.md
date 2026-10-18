# Shared contract code

Follow the [architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md). This package contains types and small
validation/encoding functions shared by real consumers. It is not a registry of every planned feature.

Change a contract and the necessary producer/consumer together. There is no package permission wall,
mandatory contract-change issue or preserved V1 byte contract for this pre-release replacement.
Remove obsolete fields and their callers instead of filling them with fake IDs, grants or digests.

## Useful shared data

Keep source/revision identity, source coordinates, selected scope, actual limits, indexed payloads and
results that consumers use. Runtime I/O and mutable state belong to their concrete owners.

Simplify `evidence.rs` for ordinary retrieval/orientation: no mandatory exact-scan budget, plan
fingerprint, proof chain, ranking trace or durable handle. Retain actual revision/range/snippet,
freshness and gaps. A source-record revision is not a content revision.

One new local request/result format can replace both old V1/V2 trees with their consumers.
Do not grow a third family or preserve eleven old recipes only for fixtures.

## Libraries and encoding

Typed Serde serialization is allowed for ordinary records. Add the direct dependency to this existing
owner when implementing; a transitive Cargo.lock entry is not an integrated codec. Do not make another
DTO/value-tree package to avoid touching the real types.

Canonical bytes are needed only for an identity/profile that genuinely uses them. Unused internal
records need no canonical hash. Rebuild derived formats when they change; do not add a legacy codec
or migration engine. Never reinterpret existing stored bytes under a new identity.

Use the shared BLAKE3/SHA-256 owner for real content/artifact integrity. Retain actual algorithm/preimage
vectors and bounded streaming where used. Do not require per-site ledgers, issue-state classifications,
source-file hashes or an unchanged canonical test catalogue.

## Verification

Check changed production code and real consumers. Test useful data bounds, exact byte identity,
request/result behavior and truthful partial results after integration. Remove legacy layout/encoding
fixtures with the format they tested. Old P00 read sets and Wave-1 exception rules are historical.
