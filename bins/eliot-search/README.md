# eliot-search

The intended standalone CLI owns invocation, one local connection, current typed requests, bounded output and
exit status. The current binary still contains several incompatible client directions and does not reach the
indexed product.

## Exact current behavior

- one-shot `scan-file`/`scan-stdin` commands forward to the daemon shell and work over current bytes;
- default builds reject provider `status/search/lexical/refresh/indexed` routes as
  `LEGACY_LOOPBACK_HARNESS_DISABLED`;
- nevertheless the default CLI still compiles the old provider client/protocol/config/ports stack and, on
  Windows, the named-pipe transport and Credential Manager owner;
- `public_client.rs` intercepts `serve-data-root`, starts a daemon child and expects `data_root_ready`, then
  speaks newline/tab/hex DIRECT commands with opaque handles and arbitrary byte ranges;
- the daemon's earlier direct-registry interceptor captures that same argv and exits with registry metadata,
  so the interactive session cannot start;
- the legacy-loopback feature exposes another endpoint/token/indexed fixture path, not a product fallback.

Help currently lists persistent commands whose names route to different or shadowed daemon applications. Do
not treat help output, a compiled provider dependency or the hard-coded daemon self-test flag as supported
Search.

## Target

```text
eliot-search
→ capture args_os/config once
→ Search | Exact | Continue | Expand | Health | Status | Version
→ spawn and own one eliot-searchd child
→ inherited stdin/stdout
→ u32-le + bounded Serde JSON LocalClient
→ current daemon dispatcher
→ typed source-backed response
```

Trusted child stdio needs no pairing key, grant, per-frame MAC, installation binding or endpoint/token file. A
future restricted-client adapter authenticates separately and enters the same dispatcher.

The CLI never opens redb/object/Qdrant state, mints access scope or supplies raw Qdrant filters/point IDs.
Continue uses one `WindowId`; Expand uses one emitted `WindowItemRef` and bounded relative context. Human and
JSON/NDJSON output preserve final order, exact source/revision/location/excerpt and truthful gaps.

Delete the old core/session/endpoint/native-registration/credential/harness routes with their callers. Keep
child ownership/cleanup, non-UTF-8 invocation handling, cancellation/deadline, finite output and useful exit
mapping.

Read [AGENTS.md](AGENTS.md) and [FUNCTIONS.md](FUNCTIONS.md). Issue
[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332) owns this cutover; #264 owns the complete
source → Qdrant → CLI result.
