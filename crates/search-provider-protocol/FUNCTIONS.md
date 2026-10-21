# Local protocol — one wire and one I/O contract

The [implementation guide](../../docs/implementation/README.md) owns overall composition. This file owns
framing/session behavior; client and CLI documents link here rather than defining another protocol.

## Source findings

At `c8e38b6`, protocol/client/provider-composition/public-runtime code forms several parallel stacks:
manual JSON/base64, pairing/MAC, bindings, grants, recipes, replay records and a separate tab-line service.
The active legacy facade can turn IndexedQuery back into `op\tquery\t<hex>`. None is the target wire.

Retain the useful length-prefix parser and fail-stop output handling. The actual
[output_deadline helper](../../bins/eliot-searchd/src/public_runtime_service/kernel/output_deadline.rs)
explicitly says that it cannot interrupt an already-blocked writer. Its clock check is useful but not a
transport timeout implementation.

## Framing and allocation

```text
u32 little-endian body length + one UTF-8 Serde JSON message
```

Read the prefix incrementally, reject zero/oversize before body allocation, then read one bounded body.
Decode a current typed DTO and validate field/collection limits before dispatch. A frame bound is not a claim
of zero-allocation decoding or a bound on every downstream allocation; downstream work has its own counters.

For output, use `serde_json::to_writer` into a small capped in-memory Write sink which rejects a write before
its checked remaining capacity is exceeded. Bound the input DTO too. Only after complete encoding succeeds,
write the prefix/body and flush. `to_vec` followed by a size check is not allocation-bounded serialization.
Do not serialize directly into the transport, where an encoding/limit error would leave half a frame.

A resource failure before writing may produce one small bounded failure response. Any partial transport
write/flush failure closes the session; no appended second terminal/error frame and no cursor commit.

## Messages are data, not an API-generation checklist

Use only operations with actual handlers: Search first, then selected Exact/Continue/Expand and real admin
commands. Version, request ID, finite timeout and the operation form the request envelope. Response events
carry request ID and one terminal result; progress is optional and bounded. DTO names are illustrative.
Do not generate optional providers, progress registries, grants or recipes because the sketch names them.

On a healthy connection the server emits at most one terminal per request; a cancelled/dropped connection
may deliver none. No exactly-once receipt protocol is implied.

## Connection and process lifetime

The owning-account baseline uses owned child stdio. No pairing key, per-frame MAC or product grant is needed;
selected source scope is still resolved by the daemon.

One-shot: request, terminal, close/reap; return no live query locators.
Multi-request session: terminal ends a request but leaves the same child/streams alive. Continue/Expand refer
only to windows in that session. Explicit end/EOF/failure drops windows and closes/reaps the child. New
independent invocations cannot reuse them. Persistent cross-invocation transport is not baseline scope.

A selected restricted-client adapter authenticates outside this wire and narrows its actual session scope.
Do not build that adapter until selected.

## Cancellation and I/O

One active data request is sufficient, but the connection reader must keep servicing Cancel and EOF while
that handler runs. A synchronous `read request → execute handler → read again` loop cannot support this.
Use the existing runtime with one bounded control/read path, one active operation and one response writer.
Do not add a general scheduler or unbounded request/progress queues.

Deadline checks before blocking calls cannot bound an already-blocked call. Neither wrapping a blocking
operation in a timeout nor aborting a started `spawn_blocking` task proves that it stopped. Select an actual
interruptible platform I/O path, or use the owned child's finite close/termination boundary and explicitly
state the limitation. Do not leak a new detached blocking worker after each timeout. Platform support remains
unverified until silent input, blocked output and shutdown are exercised.

## Three different clocks

- Session idle/lifetime policy controls the connection, not each operation's work budget.
- Each new request establishes a fresh local monotonic deadline. Nested calls do not renew it. Charge queue
  and framing time where the chosen transport can observe them; serialize durations, not cross-process Instant.
- A QueryWindow has an absolute expiry fixed at creation. Continue/Expand use the earlier of their new request
  deadline and that expiry. They neither inherit an already expired Search deadline nor extend window TTL.

After an operation deadline, a separate small cleanup allowance may release/terminate owned resources; it
cannot resume backend work or report late results as timely success.

## Output and window commit

Keep a prepared page uncommitted until the whole frame has been written/flushed locally. The commit concerns
only the handled candidate prefix represented in that frame; output-limited candidates are not skipped.
Successful flush is not proof the peer received or processed the frame. A lost connection invalidates its
windows and the user repeats Search; there is no durable resume, acknowledgement ledger or exactly-once claim.
Do not hold a global view/window-manager lock across slow source/transport I/O; serialize only the active
connection/window work needed by this sequential session.

## Donor verification — 2026-10-21

- [serde_json 1.0.151 to_writer](https://docs.rs/serde_json/1.0.151/serde_json/fn.to_writer.html): accepts an io::Write sink; the application supplies its capped buffer.
- [Tokio 1.53.1 spawn_blocking](https://docs.rs/tokio/1.53.1/tokio/task/fn.spawn_blocking.html): started blocking closures cannot be aborted; a shutdown timeout stops waiting, not the work.
- [Tokio stdin](https://docs.rs/tokio/latest/tokio/io/fn.stdin.html): its blocking reader is not cancellable. Wrapping it does not establish bounded shutdown. This is a documentation check, not a tested platform adapter.

Use the workspace's selected versions during implementation; these references establish only the described
API semantics. No new dependency or native adapter is claimed to have been integrated.

## Cutover and behavior checks

Change the actual DTO, decoder, dispatcher, client and output consumer together; delete manual-codec,
pairing/grant/recipe and legacy line paths after callers move. A sync client API may wrap the same runtime;
do not create a second I/O engine.

After code/minimal Clippy, exercise split/oversize frames, capped serialization, same-session paging, a new
invocation's expired locator, Cancel during active work, silent input, blocked output, EOF and owned-child
cleanup. A complete source-backed round trip is still required; this document proves no runtime result.
