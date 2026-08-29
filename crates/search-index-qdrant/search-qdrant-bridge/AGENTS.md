# Agent contract — search-qdrant-bridge

Own only `crates/search-index-qdrant/search-qdrant-bridge/`. Do not edit another package, root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The bounded packet is `swarm/assignments/search-qdrant-bridge.md`.

## Ownership

- Qdrant schema/capability/data-plane translation
- strict filters and exact acknowledged mutations/readback
- private vendor types

## Forbidden ownership

- executable/process/ACL/Job Object lifecycle or secret storage
- recipe/access/publication/result semantics
- vendor types in public ports
- automatic upgrades

## Dependencies

Only `search-contracts` and `search-domain`; the daemon supplies a qualified endpoint/auth lease.

## Size

Target `src/` ≤ 7,000 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
