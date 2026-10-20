# eliot-search CLI — current source and replacement map

The CLI should own one captured invocation, owned local connection, current typed request, bounded
presentation and exit status. It never owns query/access policy, redb/object/Qdrant state or grants.

## Current source has four directions

| Path | Actual state |
|---|---|
| one-shot forwarding in `app/core.rs` | Starts the daemon command for scan-file/stdin; this is the only simple working search route. |
| `public_client.rs` | Intercepts `serve-data-root`, spawns a child, waits for `data_root_ready`, then uses newline/tab/hex commands, session continuation strings and opaque source handles. The daemon entrypoint shadows this argv before the service handler, so startup fails. |
| default provider stack | `search-provider-client`, protocol/config/ports, native registration and Windows pairing credential/pipe dependencies compile even though `app.rs` rejects provider search/status/refresh/indexed commands by default. |
| legacy-loopback harness | Re-enables address/token/endpoint descriptors and a marked `IndexedQuery`; fixture-only and not a fallback. |

`UnsignedRequest::IndexedQuery` still embeds a strict marker and converts to legacy `op\tquery\t<hex>`.
Expansion can carry an opaque handle plus caller-selected absolute revision offsets. Machine output is partly
constructed/substr-parsed manually. These are replacement targets, not the wire to preserve.

## One invocation

Capture `args_os()` and selected configuration once. Preserve non-UTF-8 paths. Produce one closed enum:

```text
Search { selected_sources, query, limit, page_size, timeout, output }
Exact { selected_sources, predicate, limits, timeout, output }
Continue { window_id, page_size, timeout, output }
Expand { window_item, relative_context, timeout, output }
Health | Status | Version | ShutdownOwnedChild
```

Optional operations exist only with real handlers. Unknown input fails before starting a child. Help/version
create no daemon/root effects. Do not retain separate Query/IndexedQuery, recipe IDs, marked payloads or raw
provider filters.

## One connection

```text
CLI
→ spawn/own daemon child
→ inherited stdin/stdout
→ one LocalClient
→ u32-le length + bounded Serde JSON
→ current dispatcher
```

Retain exact child/stdin/stdout handles. On terminal/error/cancel: close input, bounded wait, then kill only
the child created by this invocation if cleanup exceeds its bound. A persistent daemon is never killed by an
ordinary client.

One original absolute deadline covers write/read/cancel. Partial I/O, EOF or protocol corruption makes the
session unusable; do not blindly reconnect/replay. Trusted child stdio uses no pairing/grants/binding key.

## Output and windows

Render one typed response in human or JSON/NDJSON form:

- final rank unchanged;
- exact source/revision/representation/location;
- verified excerpt;
- truthful partial/gap/coverage metadata;
- optional window/item locator and expiry;
- no denominator unless Exact produced it.

Use Serde and bound encoded records before write. `Continue` sends one `WindowId`. `Expand` sends one
`WindowItemRef` and finite relative context; it cannot request arbitrary bytes from the retained revision.
Cursor/window state advances only after complete output delivery.

## Remove after cutover

- `provider_client/core.rs`, endpoint/TCP/token/descriptor and line-protocol modules;
- `UnsignedRequest`, `to_core`, marked indexed query and active legacy session wrapper;
- old typed pairing/profile/grant path for trusted local use;
- Windows provider transport and pairing credential consumers unless a selected restricted client needs a
  new concrete adapter;
- `public_client` tab/hex session and opaque range expansion;
- `legacy-loopback-harness`, remote/indexed fixture entries and token files;
- duplicate parser/output/profile/grant fixtures and forwarding facades.

Retain child ownership/cleanup, args_os correctness, current framed client, cancellation and bounded output.

## Verification

Exercise the actual default binary: help/version no side effects; child Search to Qdrant/retained evidence;
explicit/empty/unknown source selection; human and JSON/NDJSON escaping/limits; Continue/Expand; Ctrl-C,
timeout, EOF and partial output; owned-child cleanup; non-UTF-8 path invocation. Verify that no default command
can enter a legacy endpoint/pairing/tab-hex route.

Run minimal Rust 1.98 compiler/Clippy for changed CLI/client/protocol/daemon targets after implementation.
