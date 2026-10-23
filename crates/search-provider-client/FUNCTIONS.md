# Provider client — one serviced session, not another protocol platform

[Protocol](../search-provider-protocol/FUNCTIONS.md) owns framing, cancellation, clock origins and terminal races.
[CLI](../../bins/eliot-search/FUNCTIONS.md) owns invocation/presentation. This file selects the client mechanisms;
names from old code are inspection seams, not an API catalogue.

## Inspected paths

R022 (`ff59f930`) and R023 (`305ccab1`) established:

| Source | Finding |
|---|---|
| typed.rs / typed/io.rs | Useful incremental input and fail-stop state, wrapped in pairing/profile/grants. Whole-record writes monopolize the mutable session. |
| typed/state.rs and protocol/session.rs | Completed IDs fill a replay set and can block Cancel. Cancel sending is capped by the oldest data deadline. |
| CLI provider_client.rs | Reexports the typed client but delegates its ProviderSession to a separate legacy core; constructs duplicate owned requests. |
| CLI native_registered.rs / typed/native.rs / state/grant.rs | Compatibility bootstrap checks installation coordinates, then stores no trusted installation; mandatory first grant rejects it. Anchored NativeClientExpectations is a different path. |
| CLI provider_client/local.rs | Pipe opener exists but is not called from the inspected CLI dispatch. A declaration is not an installed handler. |
| CLI core verifier/exchange | Digest covers a synthetic status/request/sequence receipt, not displayed payload. Ordinary lines are printed before terminal/ack. |

R024 (`676135f4`) finishes the named CLI response/transport/cancellation helpers and native/grant source tails.
The line reader correctly retains a partial response, original counters and one cleanup allowance across a
diagnostic timeout. DeadlineCancellation applies only to Health/Version, not arbitrary Search work. The daemon's
canonical reader starts a request clock on an empty poll, and its serving quantum does not bound whole-record
writes. These are source findings, not exercised product behavior.

Do not preserve the old protocol merely to reuse those small correct mechanisms. Conversely, do not discard
partial-frame continuity, source scope, error meaning or child ownership when removing old ceremony.

## Selected replacement

Keep one owned connection, its finite buffers, one active data request, bounded control state and a non-reused
session-local ID counter. Remove completed-request data. No replay set, grant history, compatibility facade or
new scheduling framework. Counter exhaustion closes instead of aliasing an old request.

The current CLI and one-shot/session modes use this same client. A synchronous outward call may wait on it, but
cannot run write-all/read-until-terminal while preventing control. Do not hold a session-wide mutex across I/O.

Use already pinned **Tokio 1.53.1** for the parent's spawned-child I/O and process waiting. Add only the direct
features needed by that real caller during Cargo cutover; no version upgrade or second runtime per command.
Take ChildStdin/ChildStdout/optional ChildStderr into the one session owner. Service both directions and captured
diagnostics concurrently, with finite application buffers. Command::output/wait_with_output remain collecting
APIs, not bounded-capture shortcuts.

Keep prefix/body positions and outbound byte position in the session, outside disposable futures. In a select
loop use cancellation-safe read/write calls and advance by their returned count. Do not recreate read_exact or
write_all after another branch wins: their internal progress would be lost or resent. If the whole connection
is deliberately discarded, abandoning partial state is fine; it is not fine when reusing the stream. The
protocol recipe determines when to finish the frame and when to close.

A single event loop still stops if one branch blocks its thread. Do not put synchronous OS reads, large transforms
or backend waits directly into it and call the loop concurrent. Reuse bounded workers already required by the
real operation; retain ownership until they stop. No detached blocking task per timeout.

## Child and input lifetime

The client session owns the child and exactly one cleanup boundary. Enable kill_on_drop as a last-resort fallback,
not proof of reaping. Explicit cleanup closes owned stdin, observes a finite graceful wait, requests termination
when necessary and observes/reaps exit. Tokio start_kill requests termination; kill().await also waits. Neither
an issued kill nor dropping a handle is a hard timing/rollback guarantee. Do not kill an independent daemon.

Child pipe handles are not the CLI's console stdin or the daemon's inherited stdin. Tokio stdin uses a blocking
reader that cannot be cancelled; dropping its future can leave runtime shutdown waiting for input. Do not move
that problem into spawn_blocking or create a reader per request. For the standalone CLI, one process-lifetime
terminal reader with a bounded command handoff is sufficient: control can signal cancellation directly, the
reader owns no child/store guard, and process exit rather than an indefinite join ends a blocked console read.
A reusable library must not silently inherit that process-exit policy. In the child daemon, inherited input has
one process-scoped reader; parent EOF/close and owned-child termination are the explicit fallback for blocked I/O.
This does not certify platform timing, crash cleanup or server-side cancellation of external effects.

## Reuse and removal boundaries

Retain length-before-allocation, incremental framed input, typed correlation, complete terminal validation,
truthful partial/gap results and fail-stop damaged-stream handling. Native pipe flush must not become blocking
FlushFileBuffers or a delivery receipt. Do not globally discard live windows on an ordinary pre-write application
error; protocol distinguishes request outcome from failed transport.

Remove pairing/profile/grant constructors, synthetic registration producers, MAC/replay state, TCP/token/endpoint
features and duplicate request/client models with their real callers. Provider pairing credentials are not
revision encryption: shared OS-secret owners and unpublished #598 storage remain outside this deletion.
Validate borrowed fields, construct one current request and retain actual wire-input validation; no second
request-model or validation crate. Do not repair a retired bootstrap just to keep its constructor usable.

## Evidence and remaining cutover

Exact source read: tokio-rs/tokio tag tokio-1.53.1,
[stdin.rs](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/io/stdin.rs),
[read_exact.rs](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/io/util/read_exact.rs),
[write_all.rs](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/io/util/write_all.rs).
API references: [process](https://docs.rs/tokio/1.53.1/tokio/process/index.html),
[Child](https://docs.rs/tokio/1.53.1/tokio/process/struct.Child.html),
[select](https://docs.rs/tokio/1.53.1/tokio/macro.select.html), read 2026-10-23.
These identify mechanisms and limitations, not a working transport in this repository.

After the connected production cut, run minimal pinned compiler/Clippy on the client and actual CLI/daemon
binaries. Later cases cover persistent partial-frame state, idle then short request, long sessions without
history growth, simultaneous backpressure, cancellation/terminal races, diagnostic bounds and child cleanup.
The full daemon native/bootstrap/authority caller closure remains separate audit work; no broad deletion is
licensed by reading these helpers. R024 changes these instructions, not the legacy transport implementation.
