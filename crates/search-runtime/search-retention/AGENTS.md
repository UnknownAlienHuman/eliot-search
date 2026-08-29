# Agent contract — search-retention

Own only `crates/search-runtime/search-retention/`. Do not edit another package, root workspace, shared
contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-retention.md`.

## Ownership

- CAS retention roots/mark/sweep policy
- monotonic purge/tombstone/receipt semantics
- restore revalidation/quarantine
- lifecycle invalidation requests

## Forbidden ownership

- ordinary index reclaim or handle storage/authorization
- concrete redb/revision-store/Qdrant/process dependencies
- physical secure-erasure overclaims

## Dependencies

`search-contracts`, `search-domain`, `search-epoch-pins`, `search-index-reclaimer`, `search-handles`.
Concrete control/object/index operations arrive through ports.

## Size

Target `src/` ≤ 7,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
