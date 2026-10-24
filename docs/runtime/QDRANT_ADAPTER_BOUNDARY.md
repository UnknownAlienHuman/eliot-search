# Qdrant adapter boundary

Qdrant is the only indexed retrieval backend. `search-qdrant-bridge` is the sole package allowed to depend
directly on `qdrant-client`; vendor request/response/filter/schema types remain private.

## Selected runtime boundary

Daemon composition supplies one configured endpoint, optional actual credential, transport settings and the
selected physical generation/schema. Publication and query use the same concrete adapter. Startup observes the
live server capabilities required by the selected profile; past probes, build strings and artifact checksums are
development/release evidence, not constructor credentials.

Mutation uncertainty returns to publication for reconciliation. A local timeout, reconnect, client drop or
matching readback does not prove that every earlier dispatched write is terminal. Reads report ordinary failures
and never manufacture a commit receipt.

## Current code debt

The former `QualifiedGate` constructor input was removed at `7d92fb8b`; `qualified/gate.rs` now returns `()` for
test/diagnostic validation. Do not recreate that token.

Current `RealDataPlane::connect` still requires supervisor binding and an API-key lease, checks an exact server
build and uses fixed nested transport timeouts. Other `qualified/**`, auth/binding checks, a process-local
operation receipt map and duplicated point/payload models remain. Normal daemon publication/query is still not
connected to this runtime.

The #263 cutover constructs the bridge directly from:

```text
selected endpoint
+ actual optional credential
+ one absolute attempt deadline/cancellation
+ current capability and exact selected-schema observation
```

Preserve real auth, transport, schema, mutation/readback and bounded-response behavior. Remove historical
qualification/binding/receipt authority and only the unconsumed SDK wrappers after tracing real callers.

## Optional process owner

`search-qdrant-supervisor` has no connected product caller. External endpoint mode is the baseline. A future
managed-child profile may own an actual process handle, private config/resources, real platform child-tree
containment and bounded startup/shutdown. It must use the same bridge for authenticated readiness and never
block external-endpoint Search.

## Verification

Run 39872058276 passed Rust 1.98 locked check/strict Clippy for `search-qdrant-bridge --lib` at `7d92fb8b` only.
It does not cover current consumers, daemon composition, Windows, live Qdrant or retrieval behavior.

For each bridge change, compile the bridge and real consumers, then execute the affected schema/query/mutation/
recovery case against the selected server. Record raw result boundaries in the registry/PR; do not store them as
credentials required by ordinary startup.
