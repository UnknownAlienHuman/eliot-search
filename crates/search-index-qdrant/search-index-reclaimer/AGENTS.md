# Agent contract — search-index-reclaimer

Own only `crates/search-index-qdrant/search-index-reclaimer/`. Do not edit the root workspace, shared
contracts, architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-index-reclaimer.md`.

## Ownership

- committed retired-manifest validation
- epoch/route watermark eligibility
- exact-ID deletion plans, receipts and resume cursors
- separation of ordinary reclamation from security purge

## Forbidden ownership

- publication visibility/retirement decisions
- pin acquisition
- broad-filter correctness deletion
- CAS deletion or purge acknowledgement
- direct vendor adapter dependency

## Dependencies

`search-contracts`, `search-domain`, `search-epoch-pins`. Deletion executes through an injected
vendor-neutral index-admin port.

## Size

Target `src/` ≤ 4,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
