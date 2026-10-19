# eliot-search CLI — current source and replacement map

The CLI owns one captured invocation, owned local connection, current typed request, bounded presentation and
exit status. It never opens redb/object/Qdrant state or invents query/access authority.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md),
[local protocol](../../crates/search-provider-protocol/README.md) and
[client map](../../crates/search-provider-client/FUNCTIONS.md).

## Verified current source

The repository already has three client directions:

| Path | Actual state |
|---|---|
| `public_client.rs` | Starts an owned daemon child through stdin/stdout for DIRECT. This is the smallest useful transport owner. |
| `search-provider-client` / `provider_client/local.rs` | Implements native typed pairing/profile/grant connection. Product caller for its trusted registration input/listener was not found. |
| `provider_client.rs` active facade | Reexports typed types, but `ProviderSession` wraps `core::ProviderSession`. `UnsignedRequest::IndexedQuery` calls `encode_indexed_query`, embeds a strict prefix, then maps to legacy `op\tquery\t<hex>`. `Expand` still carries opaque handle bytes and arbitrary absolute start/end. |

The old provider core/endpoint/harness candidate identified earlier is roughly 2,031 physical lines across
eight files, but replacing it with another wrapper is not progress. The current typed session and legacy core
must converge into one current client.

## One invocation

Capture `args_os()` and relevant environment/config once. Preserve non-UTF-8 paths. Produce one closed enum,
for example:

```text
CliCommand {
  Search { selected_sources, query, limit, page_size, timeout, output }
  Exact { selected_sources, predicate, limits, timeout, output }
  Continue { window_id, page_size, timeout, output }
  Expand { window_id, item_index, context, timeout, output }
  Status / Health / Version
  ShutdownOwnedChild
}
```

Optional `Orient`/`Compare` are parsed only when real handlers exist. Unknown command/field fails before
starting a child or opening a pipe. Help/version that need no daemon create nothing.

Do not retain separate `Query` versus `IndexedQuery`, marked strict prefixes, V1 recipes or legacy DIRECT
query syntax. The current `Search` operation selects the connected implementation.

## One connection

Baseline:

```text
CLI
→ spawn/own daemon child
→ inherited stdin/stdout
→ LocalClient
→ current framed protocol
```

This reuses the actual child ownership already present. A persistent owner-only OS pipe may be added when a
measured startup/use case needs it. Do not require both routes before useful search.

Child cleanup:

- retain exact child/stdin/stdout ownership;
- close input after terminal/explicit cancellation;
- bounded wait;
- kill only the child created by this invocation when cleanup exceeds its bound;
- report unknown cleanup honestly.

A persistent daemon is never killed by an ordinary query client. Explicit shutdown must be authorized by
that connection profile.

Trusted child stdio does not use pairing, grants, binding records, shared-key MAC or capability digests. A
selected restricted-client integration authenticates separately and can reuse the same current request/result
wire.

## Request execution

1. Build the current typed operation directly from the parsed invocation.
2. Validate finite query/scope/page/context/output limits.
3. Convert the configured request timeout once to a local absolute deadline.
4. Send one bounded framed request through `LocalClient`.
5. Render finite optional progress and exactly one terminal response.
6. On interrupt, send best-effort cancel for the live request ID and close after the remaining deadline.
7. On partial/protocol/EOF failure, fail the session; do not retry blindly or claim server rollback.

The daemon resolves selected source IDs against its pinned view. The CLI cannot mint/widen a grant or pass raw
Qdrant filters/routes/point IDs.

## Continue and expand

`Continue` carries one `WindowId` and finite page size. `Expand` carries one `WindowItemRef` plus a bounded
relative context request. The CLI never decodes the ID or supplies arbitrary absolute revision offsets.

A normal card already includes exact source/revision/representation/location and a short verified excerpt.
Expansion is optional context around that exact emitted item.

## Output

Human and JSON/NDJSON render the same typed response:

- final rank order unchanged;
- source/revision/representation/range/location;
- actual verified excerpt;
- truthful partial/gap/coverage metadata;
- optional next window/item locator and expiry;
- no denominator unless an explicit exhaustive operation produced it.

Use Serde for machine output. Bound the encoded record before write. NDJSON uses one escaped physical line
per record. Do not substring-parse daemon events or manually concatenate JSON.

Exit classes remain small and actionable:

```text
success
partial_or_degraded
usage
connection_or_protocol
unauthorized_or_scope_empty       // selected restricted profile only
cancelled_or_expired
request_failed
```

A material partial response is not silent success; machine output includes its status.

## Remove after cutover

- `provider_client/core.rs` and legacy line/TCP/token/endpoint modules;
- `UnsignedRequest` facade and `to_core()` translation;
- strict indexed query prefix and hex/tab operation grammar;
- active `core::ProviderSession` wrapper;
- `legacy-loopback-harness` product paths;
- uncalled native registration/pairing/credential constructors unless a real restricted consumer remains;
- opaque source-handle + arbitrary byte-range expansion;
- separate remote/indexed harness entry paths and old parser/output fixtures.

Retain child ownership/cleanup, `args_os` correctness, typed client call, bounded presentation and useful
exit mapping.

## Verification

After code, exercise the actual binary:

- help/version without installation side effects;
- real child-stdio Search to Qdrant/retained evidence;
- explicit/empty/unknown source selection;
- human and JSON/NDJSON escaping/limits;
- Continue/Expand under one query window;
- Ctrl-C/cancel, request timeout, EOF and daemon error;
- partial output fail-stop and no cursor advancement;
- owned child cleanup;
- non-UTF-8 path invocation where applicable.

Run minimal Rust 1.98 compiler/Clippy for changed CLI/client/protocol/daemon targets after implementation.
Restricted pairing/credential matrices apply only if that selected profile is changed.
