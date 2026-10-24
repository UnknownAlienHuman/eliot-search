# Working on search-contracts

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [source/caller audit](FUNCTIONS.md),
[query cutover](../search-query/README.md) and [local protocol](../search-provider-protocol/README.md).

This package is a reduction/consolidation area. Do not add a new recipe, grant, capability, plan, fence,
receipt, lifecycle record, provider-neutral vendor schema, digest wrapper or version family.

For each cutover:

1. identify the real current producers and consumers;
2. decide whether the value is genuinely shared immutable data or owner-private state;
3. change all current consumers together;
4. move concrete Qdrant/redb/protocol/window/runtime state to its owner;
5. delete the old type, alias, codec, canonical preimage, golden and generated contract in the same slice;
6. keep only actual current IDs, locations, limits, request/results and retrieval/evidence semantics.

Use one current `Search | Exact | Continue | Expand` family. Do not add `V3`, deprecated aliases, forwarding
modules or raw-byte restore constructors. Optional operations/types exist only with real handlers.

Ordinary DTOs use bounded Serde. Canonical bytes and cryptographic hashes are allowed only for a real
content/artifact/point/profile identity whose owner recomputes and verifies the exact input. Caller-supplied
digests never establish authority or evidence.

After code, run minimal Rust 1.98 check and strict Clippy for changed real producers/consumers plus focused
current serialization/identity/location/result behavior. Recipe counts, old wire bytes, proof matrices and
frozen indexed schema tests are deleted with their APIs.
