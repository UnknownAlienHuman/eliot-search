# CLI — one invocation, explicit session lifetime

Read the [implementation](../../docs/implementation/README.md). Wire/I/O semantics belong only to the
[protocol map](../../crates/search-provider-protocol/FUNCTIONS.md); paging belongs to the
[window map](../../crates/search-query/search-continuation/FUNCTIONS.md).

## Source findings at c8e38b6

- `app/core.rs` forwards the working one-shot scan-file/stdin path.
- `public_client.rs` starts a child and waits for `data_root_ready`; the daemon's earlier argv interceptor
  shadows the intended service. Presence of a child client does not prove indexed serving.
- The default provider dependencies compile although default commands reject provider operations.
- Legacy `IndexedQuery` still becomes a marked tab/hex command. Expansion carries an opaque handle and
  arbitrary absolute offsets. These paths are replaced, not adapted into another protocol version.

## Capture and dispatch

Capture `args_os()` and supported settings once. Keep OS paths as OS values. Validate input before spawning.
Help/version without daemon work creates nothing. Build the current typed operation directly; do not pass
through recipes, grants, capability descriptors or `UnsignedRequest::to_core()`.

## One-shot versus session

These are two lifetimes of the same client/dispatcher, not two transport stacks.

| Mode | Lifetime | Output contract |
|---|---|---|
| One-shot command | Own child for the one request, then close/reap it | Bounded cards and truthful truncation; no reusable WindowId/WindowItemRef after exit. |
| Multi-request session | Own child/streams until explicit session end, EOF, expiry policy or failure | Search may return a window; Continue/Expand are sent on this same live connection. |

The CLI session can be an interactive command loop or framed client mode; select one actual interface rather
than building both pre-emptively. A terminal event completes one request, not the session. Do not close stdin
or kill the child after each successful terminal in session mode.

A new independent invocation has no old process-local window. Return an expired/unavailable-window error;
do not persist tokens, resurrect a daemon or replay Search under the same locator. A future shared daemon is
a separately selected transport/use case, not a workaround for this mismatch.

The same root cannot be held by several independently spawned owner children. A busy root reports the actual
owner conflict; it does not start another storage copy or silently change transports.

## I/O and cleanup

Use the current client's request IDs and deadlines. Cancel/EOF must remain observable while a request runs;
a sync public `invoke` may wrap an internally serviced control/read path. The protocol map specifies the
blocking-I/O limitation and required finite shutdown behavior.

At session termination: close owned input, allow bounded child shutdown, then terminate/reap only the child
owned by this invocation if it does not stop. Do not kill a persistent daemon or claim an uncertain storage
mutation rolled back. A separate cleanup deadline never authorizes further query work.

## Results

Render the same typed cards in human/JSON output without changing rank or losing gaps. Encoded records use a
capped buffer before transport output. Session results may expose locators scoped to that session; one-shot
results cannot. Continue/Expand obtain a new request budget capped by the original window expiry.

Expand accepts a selected item plus finite relative context, not arbitrary revision offsets. Partial I/O
fails the session; do not print a second protocol error after a partial frame. A successful local flush is
not a client acknowledgement.

## Removal and checks

Remove legacy provider core/endpoint/tab-hex paths, strict indexed markers, mandatory local pairing/grants,
unused native registration/Windows transport and arbitrary handle-range expansion when real callers move.
Keep useful child ownership, args_os handling and presentation code.

Minimal changed-owner compiler/Clippy follows code. Focused behavior: one-shot emits no dead locator; two
requests and Continue work in the same live child; independent invocation cannot reuse that window; terminal
does not end a live session; EOF/cancel/timeout clean up only the owned child; actual default dispatch reaches
the intended handler. No runtime outcome is asserted by this document.
