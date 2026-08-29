# Agent contract — search-retrieval-executor

Own only `crates/search-query/search-retrieval-executor/`. Do not edit another package, root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-retrieval-executor.md`.

## Ownership

- bounded leg scheduling/dispatch/cancellation
- deterministic safe-leg fusion and partial coverage

## Forbidden ownership

- validation/admission or durable query history
- raw-score comparison across populations
- concrete Qdrant/redb/process dependencies

## Dependencies

`search-contracts`, `search-domain`, `search-query-planner`, `search-lexical`, `search-epoch-pins`, `search-access`.

## Size

Target `src/` ≤ 7,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
