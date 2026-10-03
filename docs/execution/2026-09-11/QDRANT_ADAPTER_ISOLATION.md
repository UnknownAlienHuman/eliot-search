# Qdrant adapter isolation

Issue: #186

Normative references:

- `AGENTS.md` — global invariants 1 and 17, layer ownership
- `docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`
- `crates/search-index-qdrant/search-qdrant-bridge/AGENTS.md`
- `docs/execution/2026-09-05/tasks/T24.md`

## Current baseline

`main` already contains the first isolation commits:

- `e99b2957` — one workspace pin and explicit adapter boundary
- `9ce96b9f` — validator self-match correction
- `fb5be235` — `real.rs` split by private transport responsibility

These commits are implementation, not executed acceptance evidence.

## Complete the task

1. Keep `qdrant-client`, protobuf messages, collection names, request builders,
   status mapping and compatibility shims private to `search-qdrant-bridge`.
2. Keep the bridge public API entirely Eliot-owned. Daemon, publication,
   retrieval, rebuild, retention and control code must not import vendor types.
3. Keep Qdrant process ownership in `search-qdrant-supervisor`; bridge
   construction consumes a qualified endpoint/gate rather than starting or
   discovering a process.
4. Finish splitting `live.rs` and remaining large bridge modules without
   creating forwarding-only crates or a second adapter owner.
5. Treat an SDK/protobuf change as private translation work. A required change
   outside the bridge solely because a vendor type or method changed is a
   boundary defect.

## Acceptance

- only `search-qdrant-bridge` inherits `qdrant-client`;
- no `qdrant_client::*` type crosses a public signature or package boundary;
- exact server/client/artifact qualification remains fail-closed;
- no automatic download, floating version, fallback backend or mock-as-live;
- the documented upgrade procedure changes only the dependency pin, private
  adapter translation and qualification identities/evidence;
- line budgets are restored without changing data-plane behavior.

Tests and live qualification are deliberately deferred until the structural
refactor is complete; absence of execution must remain explicit.