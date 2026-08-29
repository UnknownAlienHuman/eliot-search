# Agent contract — search-candidate-validator

Own only `crates/search-query/search-candidate-validator/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-candidate-validator.md`.

## Ownership

- live-security/membership/overlay validation
- exact source readback orchestration and anchor verification
- stale rejection/replan signaling

## Forbidden ownership

- vendor payload evidence, post-filter-only security or client admission
- concrete revision-store/redb/Qdrant/process dependencies

## Dependencies

`search-contracts`, `search-domain`, `search-access`; source bytes arrive through a readback port.

## Size

Target `src/` ≤ 7,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
