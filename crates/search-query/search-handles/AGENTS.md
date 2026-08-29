# Agent contract — search-handles

Own only `crates/search-query/search-handles/`. Do not edit the root workspace, shared contracts,
architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-handles.md`.

## Ownership

- opaque handle ID generation and mutable handle records
- ephemeral/durable eligibility, TTL/count/binding quotas
- expansion authorization and disclosure/range budgets
- security, purge, owner-generation and view invalidation

## Forbidden ownership

- ranking, result projection or continuation windows
- treating handle possession as authorization
- raw source bytes, paths, Qdrant IDs or cursors in tokens
- durable handles to unsaved or unretained revisions
- direct redb/CAS/Qdrant dependency

## Dependencies

Only `search-contracts` and `search-domain`. Authorization, readback and durable storage arrive through vendor-neutral ports.

## Size

Target `src/` ≤ 6,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
