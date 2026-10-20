# search-eliot-adapter — delete the disconnected session ledger

This optional package has **no public-main Rust caller**. Its 19.9 KiB production source implements another
binding, request replay, cancellation, terminal-receipt and drain/quarantine state machine over
`search-contracts`, `search-domain` and the old provider protocol. Those responsibilities already belong to
the current local protocol/client or to a selected restricted session.

Do not connect or expand this package. The working Search application comes first. A later MCP integration is
a thin leaf binary using the same current `LocalClient` and source-backed request/result contracts.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [local protocol](../search-provider-protocol/README.md),
local [source map](FUNCTIONS.md) and optional issue [#219](https://github.com/UnknownAlienHuman/eliot-search/issues/219).

## Verified current source

`src/adapter.rs` (19,311 bytes) defines:

- a separate adapter protocol version;
- installation/incarnation/workspace/grant binding;
- closed Health/Search/Continue/ReadHandle/Cancel/Shutdown command set;
- request payload digests and byte counts;
- in-flight and terminal replay ledger;
- cancellation, drain, closed and quarantined session states;
- terminal response digests/receipts;
- bounded retained terminal history.

Repository searches found no `search_eliot_adapter::` consumer and no `EliotAdapter::new` caller outside the
package. The README/FUNCTIONS nevertheless describe capability pulses, exact eleven-recipe mapping, view
fences, revocation notices and authority receipts which the source does not implement.

This is not a useful leaf mapping; it is another unconnected provider session model.

## Current donor status — 2026-10-20

The stable MCP revision is `2026-07-28`. The official Rust SDK release inspected for this audit is:

```text
repository: modelcontextprotocol/rust-sdk
tag:        rmcp-v3.6.0
commit:     74d789515fdee63ef46a1635b801364aad6695e4
published:  2026-10-20
MSRV:       Rust 1.88
license:    Apache-2.0
```

Official sources:

- [MCP 2026-07-28 release](https://github.com/modelcontextprotocol/modelcontextprotocol/releases/tag/2026-07-28)
- [rmcp 3.6.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.6.0)
- [rmcp Cargo/features at the release commit](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/crates/rmcp/Cargo.toml)

The SDK already owns MCP protocol negotiation, JSON-RPC, tool schemas, cancellation and stdio/async-RW
transport. Release 3.6.0 adds a non-blocking stdio-pipes transport and bounds the incoming async-RW line
buffer. Eliot should not implement another MCP protocol/session stack.

## Optional target after baseline Search works

One optional binary is sufficient:

```text
bins/eliot-search-mcp/
  main.rs       rmcp stdio server and lifetime
  tools.rs      closed tools and direct mapping
  client.rs     reuse current search-provider-client LocalClient
  errors.rs     small stable MCP mapping
  limits.rs     finite argument/result/in-flight limits
```

Candidate dependency profile, after final lock/advisory review:

```toml
rmcp = { version = "=3.6.0", default-features = false,
         features = ["server", "macros", "transport-io"] }
```

Do not enable HTTP, reqwest, OAuth/auth, client/child-process transport, Tasks, resources, prompts, sampling,
elicitation, roots, subscriptions or other extensions for the initial stdio tools-only leaf.

The MCP host starts the leaf process. The leaf uses the same current `LocalClient`/framed daemon request path as
the CLI; it does not copy provider framing, query planning, source reading or authentication.

## Closed first tool surface

Expose only operations which are already working in the standalone product:

```text
eliot_search            → Search
eliot_exact_scan        → Exact
eliot_continue          → Continue
eliot_expand            → Expand
```

Add orientation/comparison only after their real source-backed handlers exist. Do not expose raw filesystem,
Qdrant, redb, object-store, shell, network or arbitrary recipe/filter tools.

Each tool accepts the current bounded Search contract fields and returns current source-backed cards/gaps/
window locators. MCP IDs and cancellation map to one live LocalClient request; they never become authority.

## Authentication and scope

Stdio process ownership does not widen Search scope. The daemon resolves the explicit selected sources against
the pinned `PublishedView`, exactly as for the CLI. The leaf cannot mint grants, source memberships or
capabilities.

If an external restricted MCP profile is ever selected, authenticate it outside the mapping layer and
intersect its real allowed subset/session with the same daemon request. Do not revive `EliotBinding`,
`GrantId`, payload digests or retained terminal receipts.

## Removal boundary

Now or with the MCP implementation:

- delete `EliotAdapter`, `EliotBinding`, adapter protocol version and command enum;
- delete request payload digests, replay/terminal ledger and terminal receipts;
- delete drain/quarantine state and package-level cancellation authority;
- delete exact-eleven-recipe, capability pulse, fence/revocation/authority-receipt documentation;
- delete the package if mapping has only one MCP consumer;
- delete fixture/golden/profile requirements protecting the disconnected API.

The optional leaf may contain small pure `SearchRequest ↔ MCP tool` mapping directly. Do not retain a crate
merely to preserve a planned package graph.

## Completion

This package is not a baseline blocker. Optional MCP work closes only after standalone CLI Search/Exact/
Continue/Expand work and one stdio-only rmcp server maps those operations through the same LocalClient with
finite arguments/results, cancellation, one source-backed result contract and no second state/authority
system.
