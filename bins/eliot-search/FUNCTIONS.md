# CLI — one owned child, explicit one-shot/session lifetime

Read the [implementation](../../docs/implementation/README.md). Wire and cancellation belong to the
[protocol owner](../../crates/search-provider-protocol/FUNCTIONS.md); paging belongs to the
[window owner](../../crates/search-query/search-continuation/FUNCTIONS.md).

## Inspected current client

At `44d7416c`, `public_client.rs` provides a useful persistent child session, but it still exposes the legacy
line/handle model:

- the client writes one command and synchronously drains daemon lines until a terminal string match; user input
  is not read during that period, so it cannot issue Cancel while work is active;
- each response line is printed immediately. If the daemon later closes before the terminal, partial uncommitted
  match/handle lines are already user-visible;
- event and token fields are located with string searches rather than decoded as typed messages;
- only the latest continuation and latest random source handle are remembered; expansion supplies arbitrary
  absolute raw-revision offsets;
- after spawning the child, many `?` error exits occur before the final `wait`, while `terminate_child` is used
  only for one not-ready branch. Normal shutdown waits without a finite fallback.

The function therefore needs one owned-child cleanup boundary for every return path. Do not add another daemon
manager merely to repair process ownership.

## Capture and dispatch

Capture `args_os()` and supported settings once; preserve OS paths as OS values. Validate help/version and
request shape before spawning. Construct the current typed operation directly. Do not pass through recipes,
grants, capability descriptors, legacy provider conversion or tab/hex commands.

The client decodes complete framed DTOs with Serde. It stages a terminal result until the whole frame validates,
then renders it. Optional progress may be shown separately but never contains cards or locators that depend on
a later window commit. Human and JSON output preserve final rank and truthful gaps.

## One-shot versus live session

These are two lifetimes over the same codec/runtime:

| Mode | Child lifetime | Result contract |
|---|---|---|
| One-shot | Spawn/own for one request, receive terminal, close/reap | Bounded cards/gaps; no reusable WindowId or item locator. |
| Live session | Keep the same child and streams across terminals until explicit end/EOF/expiry/failure | Search may return one window; Continue/Expand use that same connection. |

A terminal completes one request, not a live session. A new invocation cannot reuse an old process-local window.
A busy data root reports the actual owner conflict; the CLI does not start a hidden second copy or change
transports.

## Cancellation and input

The interactive/sync surface uses a serviced client runtime with:

- one active data request;
- one bounded control/input path that can send Cancel/end while the request runs;
- one response reader/writer contract from the protocol owner;
- no second concurrent data request, general scheduler or unbounded queue.

Ctrl-C, EOF or explicit cancel signal the active request and begin bounded cleanup. Each new request gets a
fresh deadline; Continue/Expand are additionally capped by the window's fixed expiry. The client does not
pretend that a clock check interrupts blocked OS I/O.

## Child ownership and cleanup

Wrap the spawned child, stdin and stdout in one guard immediately after `spawn`. Every success/error/panic-safe
normal return path performs the same sequence:

1. stop accepting new client commands and signal cancellation/session end;
2. close owned stdin;
3. wait for a finite graceful-shutdown allowance;
4. if still running, terminate only this invocation's owned child;
5. reap it and release the root handle.

Do not wait forever and do not kill a separately owned persistent daemon. Cleanup failure is reported without
claiming that uncertain storage/index effects rolled back. The guard is ordinary process ownership, not a
lease/receipt subsystem.

## Results and expansion

Render source/revision/location/representation/excerpt and bounded gaps from the typed terminal frame. Session
locators are `(window_id, item_index)`, never a separate random source-handle catalogue. `Expand` requests finite
relative context around the emitted item; it cannot choose arbitrary revision offsets or silently switch to the
latest source.

A partial/invalid frame produces no terminal result and closes the session. Local flush is not client
acknowledgement; lost sessions repeat Search rather than resuming through durable tokens.

## Removal and checks

Remove public line/string parsers, tab/hex conversion, latest-random-handle state, mandatory local pairing/
grants, unused endpoint/Windows registration paths and duplicate provider clients when the typed path is live.
Keep useful args/path handling, child ownership and presentation code.

After implementation and minimal pinned compiler/Clippy, check help/version without spawn, one-shot without dead
locators, two requests plus Continue/Expand in one child, full-frame-before-display, Cancel during active work,
EOF/read/write/decode errors on every post-spawn path, finite graceful shutdown, forced owned-child termination,
busy-root reporting and independent-invocation window rejection. No runtime outcome is asserted here.
