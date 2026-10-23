# Local protocol — one framed session and one commit boundary

This file owns wire framing, request/session lifetime, cancellation and transport failure.
[Windowing](../search-query/search-continuation/FUNCTIONS.md) owns cursor/item lifetime;
[client](../search-provider-client/FUNCTIONS.md) and [CLI](../../bins/eliot-search/FUNCTIONS.md) identify their
actual source seams. This is the replacement recipe, not a description of a connected implementation.

## Source findings

R013 inspected the active DIRECT loop at `44d7416c`: read one line, execute a whole handler, flush several JSON
lines, then read again. Its per-request cancellation flag has no active control reader. The CLI prints those
lines before terminal, so failed output can expose cards/handles which the server never commits. The 64-KiB
per-line bound does not cap a whole result, and manual JSON is allocated before the check.

R022 at `ff59f930` also inspected the legacy typed client and `src/{session,config}.rs`. Its input polling retains
a finite deadline, but output retries a whole record before yielding. SessionMachine retains completed IDs;
its default 4096-entry replay ceiling applies to cancellation as well as data admission. Client cancellation
also inherits the earliest pending deadline, so a late cancellation call can fail before any control is written.
These are inspected old-code limits and replacement counterexamples, not a reproduced new-product incident.

## Wire and terminal visibility

Use one wire for one-shot and live-session modes:

```text
u32 little-endian body length + one bounded UTF-8 Serde JSON message
```

Read the prefix incrementally and reject zero/oversize before allocating the body. Decode a current DTO and
validate fields/collections before dispatch. A frame limit does not bound downstream work or every decoded
allocation; the request owns its actual work/storage counters.

Encode with `serde_json::to_writer` into a capped local Write sink before emitting any frame bytes. Do not
serialize an unbounded value and check afterward. One terminal contains bounded cards/gaps, stop/truncation
state and any session-local locators. The client exposes the terminal only after the whole frame validates.

Progress is optional; omit it until needed. If enabled, retain at most the latest bounded progress update,
limit cumulative progress bytes/events per request, and prioritize terminal/control over another update. A
progress denominator is not an event/byte budget. Progress contains no cards or locators requiring later commit.

A healthy handled request has one terminal. Transport failure may deliver none; never retry partial framing or
manufacture a second terminal after uncertain output. Local flush is not peer acknowledgement or exactly-once.

## One active request, live controls, no replay history

Dispatch only constructed operations: Search, Exact, Continue, Expand and selected administration. Version,
request ID, finite remaining duration and operation form the envelope; type names are not new subsystem requests.
Trusted owned-child stdio needs no pairing/MAC/grant/capability receipt. The daemon still resolves source scope.

Use a monotonically increasing, non-reused session-local data-request ID and one active request. Correlate replies
with that request; reject unsolicited/duplicate terminals. A Cancel names the active ID and is not another data
request. A cancel racing with an already completed target cannot cancel the next request. Do not retain a set
of every completed ID, acquire a work slot for Cancel, or require a grant/replay capacity to close the session.
Counter exhaustion closes explicitly rather than wrapping. Restricted remote authentication is separate scope.

The client and server must service input/control, output and deadlines while a handler runs. Use the selected
runtime's I/O support or a small per-connection polling loop, not a new general scheduler. A synchronous public
API may wait on that same serviced runtime. It must not hold the only mutable session/lock inside write-all or
read-until-terminal while cancellation needs the same owner.

Keep finite partial-frame state: one outbound frame/cursor and bounded pending control; coalesce redundant
Cancel signals. Continue servicing the other direction under output backpressure. On a byte stream, control
cannot be spliced into a partly written frame. Finish that frame within its bound, or fail/close the connection
and cancel server work through disconnect. A second data request is not dispatched while one is active.

A clock check before blocking I/O is not interruption. The platform adapter must supply real interruptibility,
or the design must rely on the owned child's bounded termination attempt and report its limitation. Do not
spawn a detached blocking task on every timeout. PIPE_NOWAIT polling is not overlapped I/O, and its availability
on a named pipe does not prove inherited child stdio has the same behavior.

## Deadlines across processes

Keep session idle policy, each request deadline and the QueryWindow's fixed expiry separate.

- The caller starts one monotonic request deadline before request-specific setup/spawn, encoding and I/O.
  Existing-session requests get a new deadline; nested calls and polling turns cannot renew it.
- The envelope carries a finite remaining duration sampled when preparing its outbound frame, not the original
  timeout copied after setup. Check the original caller deadline throughout actual sending and receiving.
- The receiver separately bounds incomplete prefix/body reception from the first observed byte; no per-read
  renewal. After decoding, its local deadline charges observed framing/queue time and is capped by configured
  operation limits and any window expiry. Do not serialize a Rust Instant or compare process-local epochs.
- A duration cannot reveal time spent before the receiver observed bytes. Do not claim identical cross-process
  absolute deadlines. The owning client still closes/cancels at its original deadline; unavoidable OS/backend
  tail work is an explicit limitation, not rollback. Publication owns uncertain external effects.
- After data expiry, a small independent cleanup allowance may send best-effort Cancel/end on an aligned stream,
  close it and terminate the owned child. It cannot admit new data or extend computation. If sending would require
  completing an already-expired partial frame, close instead. Do not cap every cleanup attempt by an already
  expired data deadline and then claim that cancellation was delivered.

## Session and window commit

One-shot receives a terminal without reusable locators, then closes/reaps its owned child. A live session keeps
the same child/streams across terminals; Continue/Expand use that connection and their fixed window expiry.
A new invocation cannot restore an old process-local window.

EOF/end/failure signals active work and drops session windows. Idle turns expire windows; an active operation's
guard remains live until the operation actually stops. No separate cleanup daemon is needed.

Stage the exact handled prefix/item indexes and complete terminal buffer before output. Only complete terminal
write/local flush permits the window owner to insert/advance that prefix. Encoding failure, partial output or
cancellation before terminal commits no progress and fails the session. Do not hold a global view/window lock
across source/backend/transport I/O. Lost sessions repeat Search rather than using a durable resume ledger.

## Cutover and evidence

Change DTO, codec, dispatcher, client and CLI together. Remove line/tab/hex/manual JSON, pairing/binding/grant/
recipe/replay compatibility through the actual callers. Keep useful incremental bounds, error distinctions and
partial-I/O failure behavior; these do not require the surrounding authentication platform.

After code and minimal pinned compiler/Clippy, exercise long sequential sessions without historical-ID growth;
Cancel at data-capacity/expiry; simultaneous read/write backpressure; partial prefix/body/write; aggregate output
and progress limits; complete terminal before display; two requests and Continue; silent input, EOF, idle expiry
and owned-child shutdown. No execution result is asserted here.

Donor references read 2026-10-23: [Rust process I/O](https://doc.rust-lang.org/std/process/) describes pipe
backpressure; [Instant](https://doc.rust-lang.org/std/time/struct.Instant.html) is opaque; Microsoft's
[named-pipe modes](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes)
distinguish blocking and nonblocking operation. Retrieved Rust pages identify std 1.99.1; they are API-reference
observations, not a Rust 1.98 build or a dependency/toolchain upgrade.
