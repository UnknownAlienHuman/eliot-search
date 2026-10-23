# Provider client — one serviced session, not another protocol platform

[Protocol](../search-provider-protocol/FUNCTIONS.md) owns framing, control scheduling and deadlines.
[CLI](../../bins/eliot-search/FUNCTIONS.md) owns invocation and presentation. This file identifies what to reuse
and replace in the client. Names below are source seams, not a catalogue of APIs to implement.

## Inspected at ff59f930 (R022)

The current clients are still different paths:

| Source | Actual responsibility / limitation |
|---|---|
| `src/typed.rs` | Pairing/profile/grant session; exposes synchronous send/receive and incremental `poll_receive`; an operation error drops its state. |
| `src/typed/io.rs` | Incremental bounded input with a retained deadline, but `write_parts` retries all bytes and flush until completion/deadline in one call. |
| `src/typed/state.rs` | Live request correlation plus a separate `SessionMachine`; sends and cancels borrow the same state. Cancel send time is capped by the oldest pending deadline. |
| `../search-provider-protocol/src/session.rs` | Retains every admitted request/control ID in a BTreeSet. Terminal completion does not remove history. Requests and cancellation share its capacity check. |
| `../search-provider-protocol/src/config.rs` | Default replay capacity is 4096 identities, not 4096 concurrent requests. |
| `bins/eliot-search/src/provider_client.rs` | Reexports the typed client, but its separate `ProviderSession` delegates to the legacy core; indexed requests become marked legacy query payloads. |
| `bins/eliot-search/src/app/core.rs` | Additional process invocation and endpoint-command path; not an implementation of this target session. |

Two counterexamples follow from those branches, without claiming a reproduced product incident:

- once historical IDs fill the replay capacity, cancellation of an otherwise live request is rejected too;
- after the oldest data deadline has expired, `send_cancel` cannot use a new caller-supplied timeout to send
  cleanup control: its effective send deadline is already expired, and the error closes the session.

`poll_receive` is useful, but it does not make the whole API responsive: a caller inside synchronous write/flush
cannot poll input or observe a new local cancel through that same mutable session until the call returns. Do not
copy the legacy transport wholesale merely because it already uses bounded records.

## Replacement state and ownership

For the sequential local baseline keep the connection, limits, a non-reused session-local request counter,
one active data request and bounded control state. The protocol recipe owns the partial read/write state.
Remove terminal request data when it is handled. No completed-request set, grant history or replay database.
Counter exhaustion closes explicitly; it never wraps and aliases an old request. This is local correlation,
not authentication for a future remote service.

One concrete session is used by one-shot and interactive CLI modes. A synchronous `invoke` can wrap that same
serviced runtime, but cannot be a write-all/read-until-terminal loop that prevents controls. Do not hold a
session-wide mutex across a blocking operation and then attempt to cancel through that mutex.

The owned-child constructor transfers child/stdin/stdout and any captured stderr to one cleanup guard in this
session. CLI code does not separately wait on the same process. The selected platform adapter supplies actual
interruptible I/O or the explicitly limited owned-child termination path; a trait named bounded is insufficient.
An already-open independently owned pipe has different process ownership and must not be killed on close.

## What to preserve

- Prefix validation before body allocation and incremental reads retaining one deadline across polls.
- Response/request correlation, no duplicate terminal, and complete typed decoding before exposing a result.
- Fail-stop behavior after partial writes, malformed input, EOF before terminal or uncertain stream alignment.
- Server partial/gap results remain results; transport failure is not a fabricated successful empty result.
- Error classes useful to the CLI, without embedding source text, credentials or raw frames in default output.
- Native pipe `flush` does not call blocking `FlushFileBuffers` or claim peer acknowledgement. Its existing
  nonblocking adapter is a useful seam, not proof of a new child-stdio implementation.

Cancellation of active work is mandatory; reporting progress is optional. The protocol recipe specifies how
control survives data backpressure and expiry without corrupting a partial frame. The CLI recipe specifies
cleanup and bounded diagnostic capture. Do not copy those algorithms into another client wrapper.

## Remove with the real caller cutover

Remove the old pairing/profile/grant constructors, binding expectations, sequence/MAC compatibility, replay
ledger, TCP/token/endpoint feature and duplicate CLI core. A reexport is not evidence that the CLI uses the typed
path. Trace entrypoint, actual constructor, read/write loop and terminal consumer together before deleting a
shared module. Keep the current native registration/credential overlap explicitly unclassified until its callers
are inspected; this pass is not permission to delete every native file.

After the production change, use minimal pinned compiler/Clippy for the actual client and CLI binary. Later
behavior checks cover long sequential use without history growth, cancellation at data-capacity/expiry,
backpressure in both directions, split input, partial output, complete terminal decoding, and one owned-child
cleanup on every exit. No R022 runtime or compilation result is asserted.
