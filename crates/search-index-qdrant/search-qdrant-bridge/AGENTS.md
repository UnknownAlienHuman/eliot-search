# Agent contract — search-qdrant-bridge

Own only `crates/search-index-qdrant/search-qdrant-bridge/`. Do not edit another
package, root workspace, shared contracts or architecture. Missing fields use
the contract-change process.

The bounded packet is `swarm/assignments/search-qdrant-bridge.md`.

## Ownership

- Qdrant schema/capability/data-plane translation
- strict filters and exact acknowledged mutations/readback
- private vendor types
- adapter-local handling of vendor API/protobuf changes

## Forbidden ownership

- executable/process/ACL/Job Object lifecycle or secret storage
- recipe/access/publication/result semantics
- vendor types in public ports
- automatic downloads, upgrades, version ranges or silent fallback
- requiring daemon/query/publication/control changes for an SDK rename

## Dependencies

Public behavior uses Eliot-owned contracts. `qdrant-client` is a private
workspace-pinned dependency inherited only by this package; the daemon supplies
a qualified endpoint/auth lease. Keep `default-features = false` in the root
workspace pin and run:

```powershell
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

See
[`docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).

## Size

Target `src/` ≤ 7,000 lines; split review before 8,500 total; hard stop at
10,000 hand-written Rust lines. Split vendor transport by operation family
instead of growing `real.rs` or `live.rs`.
