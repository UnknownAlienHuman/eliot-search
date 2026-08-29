# Agent contract — search-safe-reader

Own only `crates/search-source/search-safe-reader/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-safe-reader.md`.

## Ownership

- final-handle root containment
- stable pre/post metadata and digest reads
- no-execute filesystem/Git-object acquisition
- bounded retry/size/encoding observations

## Forbidden ownership

- source-admission rules
- root/identity/membership state
- parsing/materialization or durable retention
- hooks, filters, prompts, macros, builds or network execution

## Dependencies

Only `search-contracts` and `search-domain`.

## Size

Target `src/` ≤ 6,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
