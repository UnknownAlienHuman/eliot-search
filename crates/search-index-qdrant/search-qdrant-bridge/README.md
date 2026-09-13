# search-qdrant-bridge

**C15 — Qdrant data-plane bridge.**

**Status:** implemented vendor-neutral oracle, exact qualification gate and
live/real Qdrant transport. Qualification remains receipt-gated; no automatic
upgrade or fallback is permitted.

Own qualified Qdrant collection, point and query operations behind
vendor-neutral Eliot types.

## Owns

- capability and collection-schema probes
- strict-mode indexes and filter translation
- exact point mutation/readback/delete transport
- filtered query/count operations
- private vendor-type translation
- client/server qualification identity checks

## Must not own

- executable/process/ACL/Job Object lifecycle
- secret storage
- recipe, access, publication or result semantics
- vendor types in public ports
- automatic download, upgrade or silent fallback

The live T22 disposable-server harness is qualification-only. Its internals are
split into exact artifact measurement, loopback endpoint/client construction,
bounded diagnostics and child orchestration. Product process ownership remains
in `search-qdrant-supervisor`; daemon composition consumes a qualified endpoint
rather than adopting the fixture lifecycle.

Startup diagnostics read at most the final 1 KiB from each captured Qdrant log.
They never load an unbounded log and use lossy UTF-8 only for failure evidence.

The dependency and upgrade boundary is defined in
[`docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).

Validate it from the repository root:

```powershell
cargo run --locked -p xtask -- validate qdrant-boundary --json
cargo test --locked -p search-qdrant-bridge --test live_server_module_ownership
```

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
