# Agent contract — search-publication

Own only `crates/search-index-qdrant/search-publication/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-publication.md`.

## Ownership

- serialized publication/commit/recovery state machine
- journal/index operations through vendor-neutral ports
- exact readback and guarded VisibleEpoch commit
- committed retired-manifest emission

## Forbidden ownership

- concrete redb/Qdrant/process dependencies
- multiple active commits, broad closure or epoch reuse
- physical retired-point deletion

## Dependencies

`search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`.

## Size

Target `src/` ≤ 7,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
