# search-point-identity

Derive one stable retrieval-point identity and its 128-bit Qdrant address. The package is stateless and
contains no registry, source authority, transport or publication state.

## Current source

`src/s11.rs` implements a revision/coordinate-bearing v1 key. Its separation of full BLAKE3 identity from
128-bit address and its collision refusal are useful. The key shape and pre-release encoded bytes are not.

## Target

Point identity contains only immutable retrieval meaning for one physical generation. Exact revision,
representation, logical occurrence and range belong to versioned evidence in the pinned published view.
Unambiguous unchanged occurrences may retain identity; ambiguous equal repeats receive new incarnations.

The complete operation, cutover and focused behavior checks live in [FUNCTIONS.md](FUNCTIONS.md). Change
point identity, projection, Qdrant payload/readback, publication and query lookup together. Build a new
physical generation; do not preserve old S11 through dual issuers, adapters or migration machinery.

[Implementation guide](../../../docs/implementation/README.md) · [Agent notes](AGENTS.md)
