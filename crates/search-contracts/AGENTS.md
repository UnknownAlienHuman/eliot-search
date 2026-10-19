# Working on search-contracts

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [query cutover](../search-query/README.md) and
[local protocol](../search-provider-protocol/README.md).

Keep only small immutable data used by at least two real current producers/consumers. This package is not a
registry of planned features, recipes, capabilities, proofs, provider schemas or mutable lifecycle.

Replace the eleven V1 recipes, V2 parallel results, public plans/grants/capabilities and durable handles with
one current `Search | Exact | Continue | Expand` request/result family. Optional operations exist only with
real handlers. Change real producers and consumers together; do not add V3 wrappers or fake old IDs/digests.

Keep stable source/revision/object/view/point identities, locations/ranges, selected scope, actual limits,
retrieval-profile semantics and source-backed cards/gaps. Distinguish stable Qdrant retrieval point from
versioned `ViewManifestEntry` evidence. Concrete Qdrant/redb/protocol/session/window types belong to their
owners.

Use Serde for ordinary DTOs. Canonical bytes/hashes remain only for actual content/artifact/profile identity.
Delete generic canonical/digest frameworks and golden fixtures when their proof-oriented consumers disappear.

After code, run minimal Rust 1.98 compiler/Clippy for changed real producers/consumers and focused current
codec/identity behavior. Frozen recipe counts, sparse-only schemas and pre-release wire goldens are not gates.
