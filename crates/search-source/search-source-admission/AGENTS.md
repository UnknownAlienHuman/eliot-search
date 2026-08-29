# Agent contract — search-source-admission

Own only `crates/search-source/search-source-admission/`. Do not edit the root workspace, shared
contracts, architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-source-admission.md`.

## Ownership

- pure canonical `SourceAdmissionPolicy` evaluation
- deny-by-default source/path/metadata/format/sensitivity rules
- deterministic decision receipts bound to policy and observation digests

## Forbidden ownership

- opening files or Git objects
- root, identity or membership mutation
- post-admission authorization or ranking
- unknown-field allow behavior

## Dependencies

Only `search-contracts` and `search-domain`. The evaluator performs no filesystem, clock, database,
process or network I/O.

## Size

Target `src/` ≤ 4,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
