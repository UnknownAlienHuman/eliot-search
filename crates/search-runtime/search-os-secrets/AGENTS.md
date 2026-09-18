# Agent contract — search-os-secrets

Own only `crates/search-runtime/search-os-secrets/`. Do not edit the root workspace, shared contracts,
architecture or another package. Missing fields use the contract-change process.

The bounded implementation packet is `swarm/assignments/search-os-secrets.md`.

## Ownership

- opaque `SecretRef` lifecycle and purpose separation
- OS-user/installation/incarnation binding
- guarded short-lived plaintext access inside the adapter boundary
- creation, rotation and deletion receipts without secret material
- pure finite byte-layout and binding validation for the legacy protected-revision compatibility envelope

## Forbidden ownership

- Qdrant/provider process supervision
- source grants, sessions or policy decisions
- filesystem, Credential Manager, DPAPI, RNG or other platform I/O
- source identity, catalog currentness or plaintext-admission authority
- plaintext through public serialization, Debug, logs, argv, config or telemetry
- cross-user or cross-incarnation reuse
- unproved hardware-backed or secure-erasure claims

## Dependencies

Only `search-contracts`, `search-domain`, `search-ports` and `search-config`. Platform credential
dependencies remain in qualified adapters and require exact version, license review and Windows
qualification.

## Size

Target `src/` ≤ 3,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
The public `lib.rs` is a thin assembly facade; lifecycle and legacy compatibility contracts remain
separate owners inside this package.
