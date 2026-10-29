# search-eliot-adapter — delete the disconnected session ledger

This optional package is not the MCP implementation. The established caller audit found no production
`search_eliot_adapter::` import or `EliotAdapter::new` construction outside the package. Its process-local
binding, replay, cancellation, terminal-receipt and drain/quarantine state duplicate responsibilities owned by
the current local client/protocol or by a future real restricted session.

That caller/size evidence is historical, not a permanent current-branch census. [#535](https://github.com/UnknownAlienHuman/eliot-search/issues/535)
requires one exact current Cargo/Rust/non-Rust caller search immediately before deletion. Do not connect,
version or harden this ledger merely because an optional MCP leaf may exist later.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), local [source map](FUNCTIONS.md),
[deletion issue #535](https://github.com/UnknownAlienHuman/eliot-search/issues/535) and optional
[MCP issue #219](https://github.com/UnknownAlienHuman/eliot-search/issues/219).

## Current installed and donor state

`rmcp` is absent from the current ELIOT `Cargo.lock`. No MCP SDK version, transitive dependency closure or
runtime is installed or reserved.

The latest stable protocol release inspected on 2026-10-24 remains MCP `2026-07-28`. The latest stable official
Rust SDK release is:

```text
rmcp:        3.7.0
release:     rmcp-v3.7.0, published 2026-10-22
tag object:  cdf95e06d7cc2a13f9ff97e3e5d6cf1226630ae9
commit:      5d6a11f2ae022f513aec0f6e0f10f89a57891a7f
MSRV:        Rust 1.88
license:     Apache-2.0
```

The 3.7.0 release changes auth/HTTP behavior: enterprise OIDC ID-token subjects, cross-origin resource metadata
and preservation of 401/403 status around JSON-RPC errors. None is a reason to enable auth or HTTP for the first
stdio-only leaf.

Official sources:

- [MCP 2026-07-28 release](https://github.com/modelcontextprotocol/modelcontextprotocol/releases/tag/2026-07-28)
- [rmcp 3.7.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.7.0)
- [rmcp 3.7.0 workspace/MSRV](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/Cargo.toml)
- [rmcp 3.7.0 feature manifest](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/crates/rmcp/Cargo.toml)

At that tag, the smallest relevant candidate feature closure remains:

```toml
rmcp = { version = "=3.7.0", default-features = false,
         features = ["server", "macros", "transport-io"] }
```

This is inspected candidate evidence, not an installed or permanently selected pin. When #219 has a real
consumer, recheck the then-current stable SDK, MSRV, advisories, features, transitive closure and required API,
then add the exact dependency with implementation.

## Optional target after baseline Search works

```text
MCP host stdio
→ official rmcp tools-only server
→ small typed tool mapping
→ same search-provider-client::LocalClient as the CLI
→ same current framed daemon dispatcher
→ working Search | Exact | Continue | Expand handlers
→ source-backed current result
```

The SDK owns MCP JSON-RPC, initialization/version negotiation, tool schemas, request IDs, cancellation and
stdio/async-RW transport. ELIOT must not implement another MCP protocol/session stack.

One optional binary is sufficient. It owns only MCP process lifetime, finite tool schemas and request/result
mapping. It owns no query planner, source reader, Qdrant/redb/object state, grant database, query window or
second session history.

Expose only standalone operations already working in the base product:

```text
eliot_search       → Search
eliot_exact_scan   → Exact
eliot_continue     → Continue
eliot_expand       → Expand
```

Add orientation/comparison only after their source-backed handlers exist. Do not expose raw filesystem,
Qdrant, redb, object-store, shell, network or arbitrary recipe/filter tools.

Do not enable HTTP, reqwest, OAuth/auth, client/child-process transports, resources, prompts, sampling,
elicitation, roots, Tasks, subscriptions or unrelated extensions in the first stdio leaf.

## Removal boundary

After the exact current caller recheck, remove with this package:

- `EliotAdapter`, `EliotBinding`, adapter protocol version and command enum;
- request payload digests and process-local replay/terminal history;
- terminal receipts, drain/close/quarantine and package-level cancellation authority;
- exact-eleven-recipe, capability pulse, fence/revocation/authority-receipt docs and fixtures;
- dependencies on old provider protocol/domain authority types.

If the future MCP mapping has one consumer, keep it in the optional binary instead of preserving a forwarding
crate.

## Completion

This package is not a baseline blocker. #219 closes only after standalone `Search`, `Exact`, `Continue` and
`Expand` work and an official-SDK stdio server maps them through the same `LocalClient` with finite
arguments/results, cancellation/EOF cleanup, source-backed semantics and no second authority/state system.
