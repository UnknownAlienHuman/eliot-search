# CLI — invocation, one owned session and bounded presentation

[Protocol](../../crates/search-provider-protocol/FUNCTIONS.md) owns framing/control/deadline behavior;
[client](../../crates/search-provider-client/FUNCTIONS.md) owns the serviced session;
[windowing](../../crates/search-query/search-continuation/FUNCTIONS.md) owns paging and item lifetime.
This is the replacement recipe, not a claim that the indexed application currently works.

## Inspected source and deletion boundaries

R013 at `44d7416c` inspected `public_client.rs`: the persistent child uses line/string parsing, prints events
before terminal, remembers the latest random handle, and sends arbitrary revision offsets. User input is not
read while waiting for the response. Multiple post-spawn errors bypass the final wait; normal shutdown has no
finite fallback. Those defects remain recorded; R022 does not claim to have repaired that Rust.

R022 at `ff59f930` inspected `entry.rs`, `app.rs`, `provider_client.rs` and the invocation/dispatch portion of
`app/core.rs`:

- entry first tries the feature-gated remote path, then public client, then app; there is still no single
  invocation/session path;
- app gates old endpoint commands but retains another core parser; the provider wrapper reexports the typed
  client while its `ProviderSession` still delegates to the old core. Reexports are not integration;
- `core::invoke_stdio` owns another child/readiness/shutdown loop with fallible exits before reap;
- `self-test` calls `Command::output()`, then checks only `output.stdout.len()`. The API has already collected
  stdout and stderr into memory and waited for exit. That late stdout check is not a capture bound;
- ordinary forwarding uses `Command::status()` with inherited streams. It waits for exit and neither parses nor
  bounds the product response through the selected client;
- `core::run_main` replaces quotes in an error string before embedding it in JSON. Backslashes/control bytes are
  not thereby escaped. Preserve the error meaning through the selected serializer, not string replacement.

These are named source findings. Full remote/native-registration/credential and remaining feature consumers
are not yet classified. Do not delete shared native modules merely because the loopback server was removed.

## Capture once and use the real session

Capture `args_os()`/settings once, preserving OS paths. Validate help/version and request shape before spawning.
Construct current Search/Exact/Continue/Expand or connected administration directly. Do not pass through recipes,
grants, endpoint discovery, marked query payloads or tab/hex commands.

The CLI supplies the selected executable/settings to one client session. That session immediately owns the
spawned child and its streams in one cleanup guard. The CLI does not create a second guard/wait loop for health,
self-test, forwarding or interactive commands. Independently owned persistent daemons are never killed by this
invocation. A busy root reports the conflict; there is no hidden second installation or transport fallback.

The caller's deadline includes request-specific child setup and encoding. A synchronous public call still runs
the serviced client: user cancellation/EOF, input and output remain observable while work is active. Protocol
owns partial-frame handling and expiry; do not reproduce it in each command adapter.

## One-shot and session are lifetimes, not separate implementations

| Mode | Contract |
|---|---|
| One-shot | One terminal with bounded cards/gaps, no reusable window/item locator, then bounded owned-child cleanup. |
| Session | Keep the same child/streams across requests; Continue/Expand use that session's window. A terminal ends one request, not the session. |

A new invocation cannot reuse process-local locators. One active data request plus bounded control is enough;
no general scheduler or request-history archive is required.

## Output and diagnostic streams

Decode and validate the complete framed terminal before displaying its cards or locators. Render human/JSON/
NDJSON from typed values with the existing selected serializer. Preserve rank, source/revision/representation/
location and truthful partial/gaps. Never substring-parse events or repair JSON by replacing characters.
Progress is optional and cannot expose commit-dependent results before terminal.

Do not use `Command::output`/`wait_with_output` followed by a length check as bounded capture. If stderr is piped,
service it alongside stdout with finite retained diagnostic bytes; keep it outside the framed result parser.
An intentionally inherited/discarded diagnostic stream is a separate policy, not a claim of bounded capture.
Never leave a captured stderr pipe unread while waiting for stdout or process exit. A full pipe can prevent the
child from making progress. Parent output failure also ends the owned session; it must not leave child work running.

Do not advertise a raw `status()` passthrough as a validated/cancellable product exchange. Remove these parallel
launch loops when the selected session handles the real operation, rather than adding a different subprocess
framework for each command.

## Cleanup and expansion

Every success/error/interrupt path uses the session's same owner: stop new work, best-effort Cancel/end, close
owned stdin, allow finite graceful exit, terminate only this child when needed, and observe/reap its exit.
A kill call is not itself reap or rollback. Do not claim a hard OS timing bound from an unconditional wait;
report unresolved cleanup honestly and verify the chosen platform mechanism. No detached per-timeout workers.

Expand uses `(window_id, item_index)` and bounded relative context in the emitted item's representation. It
cannot select arbitrary raw offsets, revive an expired session or silently read a newer source revision.
Partial/invalid terminal input closes the session without presenting a result. A lost session repeats Search.

## Cutover and checks

Change CLI/client/protocol/dispatcher together. Delete old launch/parser/session/endpoint paths and their
exclusive fixtures with actual consumers, preserving useful OS-path handling and causal child/output cases.
After code, run minimal pinned compiler/Clippy on the actual CLI and daemon binaries and affected libraries.
Later checks cover split/partial responses, two requests and Continue/Expand, Ctrl-C/EOF during read and write,
stdout/stderr backpressure, oversized diagnostics, setup deadline, output failure, cleanup, busy root and
independent-invocation locator rejection. No R022 build/test/runtime result is claimed.

Donor references read 2026-10-23: [Command::output/status](https://doc.rust-lang.org/std/process/struct.Command.html)
and [Child lifecycle](https://doc.rust-lang.org/std/process/struct.Child.html). These document collection/wait/drop
semantics; they do not establish runtime behavior of this checkout or upgrade its pinned compiler.
