# Agent contract — search-qdrant-supervisor

Own only `crates/search-index-qdrant/search-qdrant-supervisor/`. Do not edit the root workspace,
shared contracts, architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-qdrant-supervisor.md`.

## Ownership

- exact Qdrant artifact and process identity
- loopback/ACL/Job Object containment
- bounded restart, quarantine and controlled shutdown
- consumption of an opaque, bounded secret lease supplied by daemon composition

## Forbidden ownership

- collection/point/query data-plane operations
- recipe, access, ranking or publication meaning
- opening or implementing the OS secret store
- automatic download/upgrade
- plaintext secrets in argv, config or logs

## Dependencies

`search-contracts`, `search-domain`. The daemon obtains a secret lease through `SecretStorePort` and
passes only its vendor-neutral bounded capability. Platform process dependencies require exact
qualification and Windows evidence.

## Size

Target `src/` ≤ 5,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
