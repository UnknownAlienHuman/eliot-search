# Agent contract — search-exact

Own only `crates/search-query/search-exact/`. Do not edit another package, root workspace, shared
contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-exact.md`.

## Ownership

- frozen-denominator exact-plan compilation/execution
- exact predicate profiles and truthful completeness reports

## Forbidden ownership

- indexed top-k denominators or semantic absence claims
- unbounded regex
- concrete source/storage/index adapters

## Dependencies

`search-contracts`, `search-domain`, `search-access`. Inventory/readback arrive through ports.

## Size

Target `src/` ≤ 7,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
