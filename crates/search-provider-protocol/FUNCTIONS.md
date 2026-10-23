# Local protocol — one framed session and one commit boundary

The [implementation guide](../../docs/implementation/README.md) owns composition. This file owns the local
wire, request/session lifetime, cancellation and transport failure. Client and CLI documents link here.

## Inspected current source

At `44d7416c` the active DIRECT path is a synchronous newline protocol:

```text
read one command line
→ execute the whole handler
→ emit several JSON lines, flushing each
→ read the next command
```

The service creates a fresh `Arc<AtomicBool>` cancellation value for a command, but the inspected loop has no
reader/control path that can set it while the handler runs. The interactive client likewise writes a command,
blocks draining output until a string-matched terminal line, and only then reads user input again. Cancel/EOF
therefore cannot be observed during active work in this path.

A search page is emitted as `started`, zero or more gap/match lines, then `complete`; every line has a 64-KiB
ceiling and an immediate flush. The client prints each line before seeing the terminal. Consequently, server-
side "do not commit on partial output" does not retract already displayed uncommitted cards/handles. A per-line
ceiling also does not bound the aggregate page response, and the manual `format!`/JSON strings are allocated
before `write_line` checks their size.

The existing deadline adapter checks the clock before each write/flush but explicitly cannot interrupt a write
already blocked. Keep its honest limitation; do not rename it into a cancellation guarantee.

## Wire

Use one typed wire for one-shot and live-session modes:

```text
u32 little-endian body length
+ one bounded UTF-8 Serde JSON message
```

Read the prefix incrementally. Reject zero/oversize before body allocation, read exactly one bounded body,
decode a current DTO and validate field/collection ceilings before dispatch. A frame limit does not bound every
downstream allocation; the request context owns those counters.

Serialize a response with `serde_json::to_writer` into a capped local `Write` sink that rejects before exceeding
remaining capacity. Do not use `to_vec` or `format!` and check afterward. Only after the complete body exists,
write prefix/body and flush. Any partial transport failure closes the session and commits no window progress.

One terminal frame contains the cards, bounded gaps, request-stop/truncation state and any usable session-local
window/item locators. Optional progress frames are finite and contain no cards, locators or facts whose
visibility would require a later cursor commit. The client does not display a terminal result until the full
frame has decoded and validated.

On a healthy connection each request receives at most one terminal. A cancelled or dropped connection may
receive none. Local flush is not peer acknowledgement and implies no exactly-once/resume protocol.

## Messages and dispatch

Use only operations with connected handlers: initially Search, Exact, Continue, Expand and selected direct
administration. Version, request ID, finite duration and operation form the request envelope. DTO names are
illustrative, not an API-generation checklist.

Owning-account child stdio needs no pairing key, per-frame MAC, grant or capability receipt. The daemon still
resolves selected source scope. A future restricted-client adapter authenticates outside this wire and narrows
its actual session; do not prebuild it.

One-shot and session use the same codec:

- **one-shot:** one request/terminal, no reusable locator, close/reap the owned child;
- **session:** request terminal ends only that request. The same child/streams carry Continue/Expand. Explicit
  end, EOF, failure or policy expiry drops windows and closes/reaps the child.

A new process cannot reuse an old process-local window. Persistent shared-daemon transport is separate scope.

## Cancellation, EOF and idle work

One active data request is sufficient, but the connection reader/control path remains live while it runs. It
accepts only bounded control messages such as Cancel/end and does not dispatch a second data request. Use the
existing runtime/event loop; do not create a general scheduler or unbounded command/progress queues.

The active request owns one cancellation token visible to all backend/source/formatting work. EOF/session close
sets it and begins cleanup. Idle turns also expire session windows. Removing a window prevents new acquisition;
active operations release their guards when they actually stop.

A deadline check before blocking I/O cannot interrupt an already-blocked call. A started blocking worker is not
made safe by dropping its join handle. Select an actually interruptible platform path, or rely on the owned
child's bounded close/termination boundary and state the limitation. Do not spawn a detached blocking task per
timeout. Silent input, blocked output and shutdown remain runtime evidence requirements.

## Clocks and output commit

Keep three clocks distinct:

- session idle/lifetime policy;
- a fresh monotonic deadline for each request, with queue/framing time charged where observable;
- a QueryWindow's fixed absolute expiry.

Continue/Expand use the earlier of their new request deadline and window expiry. Nested calls do not renew a
budget. A small cleanup allowance releases/terminates owned resources; it cannot resume application work.

The query owner stages the exact handled prefix and item indexes. Only complete terminal frame write/flush
inserts or advances the session window. A partial frame, cancellation before terminal or output encoding error
commits nothing and closes the session. Do not hold a global view/window lock during source or transport I/O.

## Cutover and checks

Change DTO, codec, dispatcher, client parser/presentation and actual handlers together. Remove manual JSON/
line/tab/hex paths, pairing/binding/grant/recipe/replay stacks and compatibility families after callers move.
A synchronous public API may wrap the same serviced runtime; it must not create another I/O engine.

After implementation and minimal pinned compiler/Clippy, exercise split/oversize input, aggregate capped output,
terminal decode before display, partial page, two requests in one session, same-session Continue/Expand, Cancel
while retrieval/read/output is active, silent input, blocked output, EOF, idle expiry and bounded owned-child
cleanup. This document establishes no platform/runtime result.
