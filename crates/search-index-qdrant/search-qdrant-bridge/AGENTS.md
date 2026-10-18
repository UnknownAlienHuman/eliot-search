# Agent contract — search-qdrant-bridge

Own the Qdrant data-plane adapter. Follow [root rules](../../../AGENTS.md) and the
[execution protocol](../../../docs/product/EXECUTION_PROTOCOL.md); a named immediate consumer may
change with a replaced public API. Missing shared fields go to the existing contract owner.

## Ownership and reuse

- Consume shared S9.5 payload/schema/eligibility/epoch contracts and `search-point-identity::s11`.
- Keep vendor schema/value/filter conversion, exact acknowledged mutations and readback private.
- Bind actual operations to the exact admitted generation, profile, artifact and settings.
- Use [FUNCTIONS.md](FUNCTIONS.md) for the old-to-shared payload and identity disposition under #262.

Do not own process/ACL/Job Object lifecycle, secret storage, access/recipe policy, durable publication
or result authority. Do not expose vendor types, auto-download/upgrade, or add silent fallback.
An SDK rename alone must not force daemon/query/control semantic changes.

## Dependency and verification boundary

`qdrant-client` is the private exact workspace pin, with `default-features = false`.
The daemon supplies the qualified endpoint and bounded auth lease. Preserve the
[adapter boundary](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md) and its affected source guard:

```text
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

The root protocol selects production checks and defers test/live execution until integration.
Review mixed responsibilities and delete replaced tables/codecs; a line count does not justify a new
crate, public facade or a file split with identical ownership.
