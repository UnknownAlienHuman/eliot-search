# search-contracts

Keep only small immutable values shared by real producers and consumers. This package is not a catalogue of
planned recipes, grants, proofs, lifecycle states or backend schemas.

[FUNCTIONS.md](FUNCTIONS.md) owns the cutover and shared-data boundary. [Indexed navigation](INDEXED_CONTRACT.md)
points to concrete index/evidence owners. Runtime guards, execution contexts, redb rows, vendor SDK types and
window contents stay private to their owners.

Add each request/result family with its handler; baseline Search does not wait for Exact, sessions or optional
operations. Stage 2/3A does not require sequence/validity/incarnation fields. Ordinary DTOs use Serde, not another
versioned canonical protocol. Move consumers and delete displaced types/fixtures together; merge this package
into the application/protocol boundary if no meaningful shared core remains.
