# Agent contract — search-source-admission

Own only `crates/search-source/search-source-admission/`. Do not edit the root workspace, shared
contracts, architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-source-admission.md`.

## Ownership

- pure canonical `SourceAdmissionPolicy` evaluation
- deny-by-default source/path/metadata/format/sensitivity rules
- deterministic decisions and legacy v1 receipts bound to policy and observation digests
- v2 receipt issuance/verification belongs to the permanent `search-source-admission-receipt` leaf; do not restore a forwarding module or reverse dependency

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

## #427 bounded batch size acceptance

The manager reviewed the complete admission-owned batch after the real permanent
receipt-owner split. This exact slice has a whole handwritten Rust ceiling of9,600
physical lines, including all inline/external fixtures; projected9533 is reviewed
before source edits. The10,000 hard stop stays unchanged. No prior receipt/contracts
allowance transfers and no future growth is authorized. New growth needs review.

Keep the1024-occurrence multiset contract, all finite metering dimensions and
causal fixtures. Reuse the sole evaluator and measured shared producer; no sizing
formula, second encode, duplicate evaluator, forwarding split or partial-success
fallback. Original probes and all-refusal cleanup stay with the actual owner.
