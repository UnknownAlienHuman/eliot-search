# search-provider-client — one serviced framed session

Target: one serviced framed session used by the real CLI and daemon. A synchronous public call may wrap it;
active cancellation is required, while progress and additional transport modes are optional.

Read the [protocol owner](../search-provider-protocol/FUNCTIONS.md), local [source map](FUNCTIONS.md),
[CLI owner](../../bins/eliot-search/FUNCTIONS.md) and [implementation guide](../../docs/implementation/README.md).

## Revision-specific evidence

At the inspected revision, client source still carried pairing/profile/grant/replay state and the CLI retained a
separate legacy core/session. Those are source observations at that revision, not permanent current branch facts.
Before deleting or replacing them, search current manifests, Rust/non-Rust callers, constructors, module
inclusion, features, targets, tests and workflows and move all surviving consumers in the same cut.

## Selected ownership

The client owns one live connection and services response reads, partial writes, cancellation and the original
absolute deadline while a request is active. Use one active data request and bounded control state. Completed
request history cannot gate cancellation or close. Never interleave a control frame into a partially written
frame; follow the protocol owner's fail-stop rule.

A child-stdio session owns exactly one child/stream cleanup guard. The CLI supplies invocation settings and
renders results; it does not create another process manager. A separately owned persistent pipe is not killed as
though it were this client's child.

Replace pairing/profile/grant authority, expectation records, per-record MACs, replay transcripts,
loopback/token/endpoint fallback and duplicate CLI core sessions with their actual consumers. Preserve useful
framing, fixed-deadline polling, typed result/error distinction and fail-stop cleanup. Do not translate current
requests through old recipe IDs or line/tab/hex messages.

## Completion

A real CLI request reaches the current daemon dispatcher over one bounded framed session, can cancel while I/O is
active, decodes one complete terminal response and cleans up its owned child/stream. After code, run minimal Rust
1.98 check/Clippy for actual CLI/client/protocol/daemon targets; construction or documentation alone is not
transport evidence.
