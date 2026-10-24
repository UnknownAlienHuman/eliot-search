# eliot-search — current CLI status

The intended standalone CLI owns invocation capture, one local child/session, current typed requests, bounded
presentation and exit status. The current binary still contains incompatible client directions and does not
reach the selected indexed product.

R033 rechecked current package documentation and source seams after R032. No CLI/daemon Rust was changed or
executed. The route descriptions below are source observations, not passing runtime evidence.

## Current source-selected paths

- `scan-file` / `scan-stdin` command arms still forward to the daemon shell one-shot scanner. The current branch
  has not compiled or run the CLI/daemon, so these routes are **present but unverified**, not documented as
  working commands.
- default builds reject old provider `status/search/lexical/refresh/indexed` routes as
  `LEGACY_LOOPBACK_HARNESS_DISABLED`;
- the default CLI still compiles the old provider client/protocol/config/ports stack and, on Windows, the
  named-pipe/credential owners even though the primary dispatcher does not use the selected typed client;
- `public_client.rs` intercepts `serve-data-root`, starts a daemon child and waits for `data_root_ready`, then
  uses newline/tab/hex DIRECT commands with opaque handles and arbitrary byte ranges;
- the daemon's earlier direct-registry interceptor claims that same argv and exits through registry metadata,
  so the expected interactive DIRECT session cannot start;
- the removed daemon loopback server is not a fallback. Feature-only client code cannot make it reachable.

Help still contains persistent command names owned by different or shadowed daemon applications. Help text,
compiled dependencies and hard-coded status/self-test fields are not supported-capability evidence.

## Target

```text
eliot-search
→ capture args_os and selected settings once
→ help/version without spawning
→ Search | Exact | Continue | Expand | Health | Status
→ spawn and own one eliot-searchd child/session
→ one bounded typed framed exchange
→ current daemon dispatcher
→ source-backed typed response
```

Trusted owned-child stdio needs no pairing key, grant, per-frame MAC, installation binding or endpoint/token
file. A later restricted client authenticates separately and enters the same dispatcher.

The CLI never opens redb/object/Qdrant state, mints source scope or accepts raw backend filters/point IDs.
Continue uses one session-owned `WindowId`; Expand uses one emitted `WindowItemRef` and bounded relative context.
Human and JSON/NDJSON rendering preserve final order, source/revision/representation/location, verified excerpt
and truthful gaps.

## Removal boundary

Delete old line/tab/hex, endpoint, pairing/grant, native-registration, duplicate launch/session and legacy core
routes with their actual callers. Keep OS-native argument handling, one owned-child cleanup path, active
cancellation, finite input/diagnostic/output limits and useful exit mapping.

A one-shot result returns no reusable locator. A live session keeps the same child for Continue/Expand. Decode
and validate the complete terminal frame before displaying cards or committing window progress. Partial output
fails the session and advances no cursor.

## Work and evidence

Read [AGENTS.md](AGENTS.md), [FUNCTIONS.md](FUNCTIONS.md), the
[protocol owner](../../crates/search-provider-protocol/FUNCTIONS.md),
[client owner](../../crates/search-provider-client/FUNCTIONS.md) and
[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332). #264 owns the complete retained-source →
Qdrant → pinned-evidence → CLI result.

After code, run locked Rust 1.98 check/Clippy on the actual CLI, client, protocol and daemon consumers, then the
focused one-shot/session/cancellation/output/cleanup scenarios. No current CLI runtime success is claimed here.
