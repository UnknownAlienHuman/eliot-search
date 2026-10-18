# Shared contract code

Follow the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and
[query cutover](../search-query/README.md#direct-operations-not-a-recipe-operating-system).

Keep types and small validation/encoding functions shared by real producers and consumers. This package is
not a registry of planned features, recipes, capability credentials or proof records.

Change a contract and the necessary producer/consumer together. Remove obsolete V1 recipes,
`SearchTaskPlan`, fingerprints, grants, durable source handles and capability descriptors when the new
current request/result path replaces them. Do not create V3 wrappers or fill old constructors with fake
IDs/digests.

Keep source/revision identity, coordinates, selected scope, actual limits, indexed data and source-backed
cards. Use typed Serde for ordinary records. Canonical bytes and hashes remain only where an actual
identity or integrity rule consumes them.

Verify through the connected CLI/daemon behavior and immediate codecs. Frozen recipe counts, P00 byte
goldens, package maps and contract-change tickets do not constrain this pre-release replacement.
