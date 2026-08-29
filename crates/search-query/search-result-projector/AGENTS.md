# Agent contract — search-result-projector

Own only `crates/search-query/search-result-projector/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-result-projector.md`.

## Ownership

- compact response/coverage/budget projection
- deterministic handle-subject selection and creation requests

## Forbidden ownership

- raw dumps, vendor metadata or client admission
- mutable handle storage, authorization, expansion or revocation

## Dependencies

`search-contracts`, `search-domain`, `search-candidate-validator`, `search-handles`.

## Size

Target `src/` ≤ 7,000 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
