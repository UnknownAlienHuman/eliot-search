# search-provider-client

Small synchronous client for the one current local framed protocol. It owns connection I/O, correlation,
cancellation and bounded response decoding. It does not own pairing, grants, capability comparison, query
semantics or daemon discovery.

Read the [protocol](../search-provider-protocol/README.md),
[implementation guide](../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source

The current package is about 70.8 KiB of production Rust plus large sequence/profile/grant fixtures. It
requires caller-supplied installation/incarnation/binding/pairing expectations, runs pairing/profile, issues a
standalone product grant, keeps a sequence/correlation ledger and preserves old wire bytes. The default CLI
still wraps the legacy `core::ProviderSession` for its active request path, so this state machine is neither
the only client nor the connected indexed product.

Do not make the old typed session the mandatory local constructor. Replace it with the actual current wire.

## Target API

Concrete names may differ, but the useful client surface is small:

```text
LocalClient::from_owned_stdio(child_stdin, child_stdout, limits)
LocalClient::from_owned_local_stream(stream, limits)       // optional persistent pipe

invoke(request, timeout) -> OperationResponse
start(request, timeout) -> RequestHandle                   // only if progress/cancel is retained
cancel(request_id)
receive_next() -> ServerMessage
close()
```

A simple CLI may use one sequential `invoke`. Bounded multiplexing remains internal only when a real caller
needs concurrent requests.

The client:

1. allocates a process-local request ID;
2. encodes one bounded current `ClientMessage` with Serde;
3. writes one length-prefixed frame under the original operation deadline;
4. reads bounded frames incrementally;
5. validates request ID/event order;
6. accepts optional finite progress and exactly one terminal response;
7. closes fail-stop after partial I/O/protocol corruption.

It never hashes the request, obtains a grant, compares capability/binding digests or translates to a recipe
or tab/hex line.

## Connection ownership

For child stdio, the CLI owns the child and exact pipe handles; EOF/close triggers bounded child cleanup. For
a persistent owner-only local pipe, the platform connector validates the real OS owner/ACL before handing an
owned stream to this package.

A restricted-client implementation authenticates before creating the common framed client or wraps the
transport with its selected security mechanism. Restricted identity/scope is not represented by fake local
`NativeClientExpectations`.

## Deadlines and cancellation

Convert a caller duration to one local absolute `Instant` before the first write. Framing, progress, terminal
read and optional cancel all consume that same deadline. No protocol phase renews it.

Cancellation is best effort and idempotent for one live request ID. A successful cancel response means the
daemon accepted cancellation, not that every effect rolled back. Query reads are ephemeral; publication
mutation uncertainty is not exposed through this generic client.

## Remove after cutover

- `NativeClientExpectations`, binding/pairing/profile comparison and mandatory `BindingKey`;
- `request_standalone_grant` and grant state;
- old sequence/replay/MAC transcript state beyond live request correlation;
- legacy-loopback compatibility and TCP/token/endpoints;
- exact old-byte/profile/golden fixtures;
- duplicate CLI `core::ProviderSession` and typed facade once the current client is connected;
- forwarding constructors kept solely to preserve unused native registration APIs.

Keep one concrete I/O engine, finite buffers, request correlation, cancellation, terminal ordering and
content-free transport/protocol errors.

## Completion

`eliot-search search ...` starts/opens one owned local connection, sends the current typed request, receives
source-backed cards, renders them and exits. No grant/pairing/capability/recipe/legacy-line object is created.
EOF, timeout, cancellation and partial output cleanly close the owned session and child.
