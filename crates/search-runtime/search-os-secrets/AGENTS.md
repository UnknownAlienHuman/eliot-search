# Agent contract — search-os-secrets

Own only `crates/search-runtime/search-os-secrets/`. Do not edit the root workspace, shared contracts,
architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-os-secrets.md`.

## Ownership

- opaque `SecretRef` lifecycle and purpose separation
- OS-user/installation/incarnation binding
- guarded short-lived plaintext access inside the adapter boundary
- creation, rotation and deletion receipts without secret material

## Forbidden ownership

- Qdrant/provider process supervision
- source grants, sessions or policy decisions
- plaintext through public serialization, Debug, logs, argv, config or telemetry
- cross-user or cross-incarnation reuse
- unproved hardware-backed or secure-erasure claims

## Dependencies

Only `search-contracts` and `search-domain`. Platform credential dependencies require exact version,
license review and Windows qualification.

## Size

Target `src/` ≤ 3,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
