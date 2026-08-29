# Agent contract — search-source-registry

Own only `crates/search-source/search-source-registry/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-source-registry.md`.

## Ownership

- roots, memberships, portfolios and coherent source/workspace views
- namespace owner/cutover state
- verified admission-receipt persistence

## Forbidden ownership

- source byte reads or identity derivation
- source-admission rule implementation
- access/ranking/Qdrant behavior
- concrete redb dependency

## Dependencies

`search-contracts`, `search-domain`, `search-source-identity`, `search-source-admission`.

## Size

Target `src/` ≤ 6,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
