# search-eliot-adapter — source audit, deletion and optional MCP map

This package is optional and disconnected. Standalone Search and its current local protocol must work before an
MCP/ELIOT leaf is added.

## Current package disposition

Established inspection found `src/adapter.rs` to be a process-local state machine for adapter version,
installation/incarnation/workspace/grant binding, request admission/replay, cancellation, terminal receipts,
drain/close/quarantine and retained terminal history. It does not call a real Search handler or transport.

Repository searches at the recorded checkpoint found no production `search_eliot_adapter::` import and no
`EliotAdapter::new` caller outside the package. Documentation also promised capability pulses, exact eleven-
recipe mapping, fences and revocation/authority records absent from source.

These are historical findings. Before deleting under [#535](https://github.com/UnknownAlienHuman/eliot-search/issues/535),
perform one exact current Cargo/Rust/non-Rust caller search. If no real consumer exists, remove the package,
workspace/dependency/lock wiring, source, fixtures and package instructions together. Do not preserve a facade,
feature-gated ledger or generic adapter registry.

## Current donor state — 2026-10-24

The current ELIOT lock contains no `rmcp` package. No MCP dependency or runtime is installed.

Latest stable official checkpoints inspected:

```text
MCP specification: 2026-07-28
rmcp:              3.7.0
release tag:       rmcp-v3.7.0
published:         2026-10-22
tag object:        cdf95e06d7cc2a13f9ff97e3e5d6cf1226630ae9
commit:            5d6a11f2ae022f513aec0f6e0f10f89a57891a7f
MSRV:              Rust 1.88
license:           Apache-2.0
```

Sources:

- [MCP 2026-07-28](https://github.com/modelcontextprotocol/modelcontextprotocol/releases/tag/2026-07-28)
- [rmcp 3.7.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.7.0)
- [workspace version/MSRV](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/Cargo.toml)
- [feature manifest](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/crates/rmcp/Cargo.toml)

The official SDK owns JSON-RPC, initialization/version negotiation, tool schemas, request IDs, cancellation,
stdio/async-RW transport and protocol errors. Do not build a second MCP protocol/session implementation.

The 3.7.0 release adds/fixes auth and HTTP behavior only. Those changes do not affect the selected first
stdio-only topology and do not justify enabling auth/HTTP features.

At the inspected tag, the smallest relevant candidate feature closure is:

```toml
rmcp = { version = "=3.7.0", default-features = false,
         features = ["server", "macros", "transport-io"] }
```

This line is not an instruction to modify Cargo now. When the optional binary has a real consumer, recheck the
then-current stable SDK, MSRV, advisories, exact transitive closure and API, then pin the implementation-time
selection. Exclude HTTP/reqwest/OAuth/auth, client/child-process transports, resources, prompts, sampling,
elicitation, roots, Tasks, subscriptions and unrelated extensions unless a later concrete profile needs them.

## Optional leaf after standalone completion

```text
MCP host stdio
→ official rmcp tools-only server
→ small argument/result mapping
→ same search-provider-client::LocalClient as CLI
→ same framed daemon protocol
→ working Search / Exact / Continue / Expand handlers
→ source-backed result
```

One optional binary is sufficient. It owns MCP process lifetime and mapping only. It does not own Search
session history, source scope, Qdrant, redb, object bytes, planning, ranking, windows or grants.

Suggested files, only when implemented:

```text
bins/eliot-search-mcp/
  main.rs
  tools.rs
  client.rs
  errors.rs
  limits.rs
```

A separate mapping crate is justified only when another real adapter consumes the same pure mapping.

## Closed first tool surface

Expose only working standalone operations:

```text
eliot_search
eliot_exact_scan
eliot_continue
eliot_expand
```

Mapping:

- `eliot_search` → current bounded `SearchRequest`/`SearchResponse`;
- `eliot_exact_scan` → current bounded `ExactRequest`;
- `eliot_continue` → current `WindowId` and page size;
- `eliot_expand` → current `WindowItemRef` and bounded relative context.

Do not expose generic recipe names, arbitrary JSON pass-through, filesystem tools, Qdrant filters/points,
redb/object/control internals, shell/network tools, model calls, arbitrary revision offsets or unsupported
orientation/comparison placeholders.

## Scope, lifetime and limits

MCP stdio reachability grants no Search scope. Each tool supplies explicit source IDs/options through the same
`LocalClient`; the daemon resolves them against one pinned published view. The leaf cannot mint memberships,
grants, capabilities, query windows or view fences.

The host owns the MCP process; the leaf owns one LocalClient connection. MCP cancellation maps to one live
Search request. EOF/shutdown cancels bounded work and closes the connection. Query windows remain daemon-owned.

Apply finite frame/JSON/tool-argument depth and bytes, active requests, selected source/query/page/context sizes,
result/card/gap/output bytes and one absolute deadline. Return structured current results with exact source,
revision, location, excerpt, truthful gaps/partial state and live window locators. Empty partial output never
becomes “nothing exists”.

## Remove with the package

- `EliotAdapter`, `EliotBinding`, `AdapterProtocolVersion` and `EliotCommand`;
- payload/request/response digests used only for adapter replay;
- process-local request/terminal history and replay classification;
- terminal receipts and drain/close/quarantine lifecycle;
- exact-eleven-recipe/capability/fence/revocation/authority docs and fixtures;
- dependencies on old provider protocol/domain authority types;
- package-status/wave/line-target obligations.

## Completion boundary

The current package may be deleted after the exact caller recheck. Optional #219 closes only after the standalone
CLI path works and an official-SDK stdio tools leaf maps the four implemented operations through the same
`LocalClient` with bounded schemas/results/cancellation and no second authority/state system.
