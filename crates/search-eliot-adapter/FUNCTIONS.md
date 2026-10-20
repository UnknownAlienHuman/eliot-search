# search-eliot-adapter — source audit and removal map

This package is optional and disconnected. The standalone Search application and its current local protocol
must work before any MCP/ELIOT leaf is added.

## Current source and caller reality

| Source/API | Actual behavior |
|---|---|
| `src/adapter.rs` | One 19,311-byte state machine for adapter version, installation/incarnation/workspace/grant binding, request admission/replay, cancellation, terminal receipts, drain/close/quarantine and retained terminal history. |
| `EliotBinding` | Requires `InstallationId`, incarnation, workspace and `GrantId`; these duplicate the retired provider binding/grant model rather than map current Search data. |
| `EliotRequest` | Adds a caller payload digest and byte count to an already-decoded payload. No real transport/handler is called. |
| `Admission` / `TerminalReceipt` | Process-local replay and terminal receipt authority. It disappears on restart and is unnecessary for ephemeral query reads. |
| docs | Promise capability pulses, exact eleven-recipe mapping, source/workspace fences, revocation notices and authority-boundary receipts not implemented by the source. |
| callers | Repository search found no `search_eliot_adapter::` import and no `EliotAdapter::new` caller outside the package. |

The package depends on `search-contracts`, `search-domain` and the old provider protocol. Connecting it would
reintroduce the grant/recipe/session operating system being removed by #280/#332.

## Selected protocol donor

Current upstream inspected on 2026-10-20:

```text
MCP specification: 2026-07-28
official Rust SDK:  rmcp 3.6.0
release tag:        rmcp-v3.6.0
commit:             74d789515fdee63ef46a1635b801364aad6695e4
MSRV:               Rust 1.88
license:            Apache-2.0
```

Sources:

- [MCP 2026-07-28](https://github.com/modelcontextprotocol/modelcontextprotocol/releases/tag/2026-07-28)
- [rmcp 3.6.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.6.0)
- [rmcp feature manifest](https://github.com/modelcontextprotocol/rust-sdk/blob/74d789515fdee63ef46a1635b801364aad6695e4/crates/rmcp/Cargo.toml)

The official SDK owns JSON-RPC, initialization/version negotiation, tool schemas, request IDs, cancellation,
stdio/async-RW transport and protocol errors. Release 3.6.0 includes non-blocking stdio-pipes transport and a
bounded incoming async-RW line buffer. Do not build a second MCP protocol/session implementation.

Candidate initial feature closure after final lock/advisory review:

```toml
rmcp = { version = "=3.6.0", default-features = false,
         features = ["server", "macros", "transport-io"] }
```

Exclude HTTP/reqwest/OAuth/auth, client/child-process transports, resources, prompts, sampling, elicitation,
roots, Tasks, subscriptions and unrelated extensions.

## Optional leaf after standalone completion

```text
MCP host stdio
→ official rmcp tools-only server
→ small tool argument/result mapping
→ same search-provider-client LocalClient as CLI
→ same current framed daemon protocol
→ Search / Exact / Continue / Expand handlers
→ source-backed current result
```

One optional binary is sufficient. It owns MCP process lifetime and mapping only. It does not own Search
session state, source scope, Qdrant, redb, object bytes, planning, ranking or grants.

Suggested files if implemented:

```text
bins/eliot-search-mcp/
  main.rs
  server.rs
  tools.rs
  client.rs
  errors.rs
  limits.rs
```

A separate mapping crate is justified only if another real adapter consumes the exact same pure mapping.
Otherwise delete `search-eliot-adapter` and keep the small mapping in the leaf binary.

## Closed first tool surface

Expose only working standalone operations:

```text
eliot_search
eliot_exact_scan
eliot_continue
eliot_expand
```

Mapping:

- `eliot_search` → current `SearchRequest` and source-backed `SearchResponse`;
- `eliot_exact_scan` → current bounded `ExactRequest`;
- `eliot_continue` → `WindowId` + page size;
- `eliot_expand` → `WindowItemRef` + bounded relative context.

Do not expose:

- generic recipe names or arbitrary JSON pass-through;
- raw filesystem paths/read/list/glob;
- Qdrant filters/collections/points/vectors/scores;
- redb/object/control/internal manifest rows;
- shell/process/network tools;
- resources/prompts/sampling/model calls;
- arbitrary revision offsets or old source handles;
- unsupported orientation/comparison placeholders.

Add orientation/comparison only after their real source-backed handlers exist.

## Scope, authentication and lifetime

MCP stdio reachability does not grant Search scope. Each tool sends explicit selected source IDs/options over
the same LocalClient. The daemon resolves them against one pinned `PublishedView`. The leaf cannot mint a
grant, membership, capability or view fence.

The host owns the MCP process; the leaf owns one LocalClient connection. MCP cancellation maps to the one live
Search request. EOF/shutdown cancels bounded work and closes the LocalClient. Query windows stay in the daemon
and are addressed through the current `WindowId`/`WindowItemRef` types.

If a future remote/restricted profile is selected, its actual authentication/session scope terminates before
tool mapping and can only narrow the daemon request. Do not revive `EliotBinding`, `GrantId`, payload digests,
terminal receipts or durable revocation/influence records.

## Limits and result mapping

Apply finite:

- MCP frame/JSON/tool-argument bytes and depth;
- active request count;
- selected source/query/page/context sizes;
- result/card/gap/output bytes;
- one absolute request deadline and cancellation.

Return structured current Search results. A text rendering may be a bounded convenience field, not the only
result. Preserve exact source/revision/location/excerpt, truthful gaps/partial state and window locators.
Never convert empty partial output into “nothing exists”.

## Remove now or with leaf implementation

- `EliotAdapter`, `EliotBinding`, `AdapterProtocolVersion` and `EliotCommand`;
- payload/request/response digests used only for adapter replay;
- process-local request/terminal history and replay classification;
- drain/close/quarantine lifecycle;
- exact-eleven-recipe/capability/fence/revocation/authority-receipt docs and fixtures;
- dependencies on old provider protocol/domain authority types;
- package status/wave/line-target obligations.

Do not add another adapter manager, session registry, capability pulse or mapping receipt.

## Completion boundary

This package may be deleted immediately after confirming no caller. Optional #219 closes only after the
standalone CLI path works and one official-rmcp stdio tools-only leaf maps the four implemented operations
through the same LocalClient with bounded schemas/results/cancellation and no second authority/state system.
