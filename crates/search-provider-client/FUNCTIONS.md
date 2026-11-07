# One native owned-child query

## Public consumer

`QueryClient::new()` constructs one Tokio 1.53.1 current-thread I/O runtime outside an async context.
`query_owned_child(&mut self, executable: &Path, args: &[OsString], request: SearchRequest,
deadline: Instant, cancelled: &AtomicBool)` returns QueryCompletion or content-free ClientError.
raw_exact_owned_child has the same ownership/deadline arguments with RawExactRequest and returns the same
QueryCompletion. Both methods use the serviced child I/O/reap owner; the raw result is additionally validated
against its literal, published-source selection and output limit. Raw matches stay in bytes_hex form.
The CLI supplies native invocation settings, starts the original deadline before setup and owns safe rendering.

The CLI explicitly calls enable_ctrl_c() before its query. Other callers leave signal handling disabled.
The maintained Tokio listener is first polled and registered before child spawn, then the same pinned future
remains in the existing I/O loop. A separate child process group routes terminal Ctrl-C through the parent
and its correlated Cancel/cleanup/reap path. Tokio's opt-in signal disposition lasts for the CLI process.

QueryCompletion carries the complete validated Terminal, retained diagnostics and diagnostics_truncated.
No child or pipe escapes. Complete success requires a successful observed child exit. Application Failed and
Cancelled remain typed terminals. A spawn/transport/truncation/deadline/reap failure never becomes an empty result.

## Actual I/O and cleanup

The client owns one Tokio child with piped stdin/stdout/stderr and kill_on_drop as unwind fallback. One select loop
services cancellation-safe reads/writes, explicit outbound byte positions, incremental FrameBuffer state,
child wait, a fixed deadline and cancellation checks. No timeout worker or query worker is spawned here.

Remaining milliseconds are sampled again while no request byte has been written. Once a write advances,
that exact frame/cursor remains. Cancellation during a partial Query closes the connection and reaps its child;
after a complete Query write it sends one correlated Cancel without inserting control into another frame.
At local deadline a finite separate cleanup allowance may send control but never renews query work.
A complete terminal already in flight retains its meaning; one-shot closure ends the input reader.

Stderr is drained alongside stdout and child waiting. At 64 KiB retained bytes the loop keeps draining/discarding
excess, avoiding diagnostic backpressure. Captured bytes are untrusted and must pass the CLI display boundary.

On full terminal validation the client closes its stdin and waits for stdout/stderr EOF and child exit.
Unexpected trailing stdout fails. Every ordinary return path performs an explicit observed wait; failure
requests child termination then awaits reap. A kill request or kill_on_drop alone supplies no exit evidence.
The two-second cleanup allowance bounds graceful servicing; the final OS wait is observed rather than detached
or described as a hard OS timing guarantee. Independent Qdrant/server processes are never managed by this client.

## Removal and checks

The former typed pairing/profile/grant/MAC/replay/TCP/native endpoint client and its exclusive example/fixtures
are physically retired, along with the Windows named-pipe adapter package. Generic local framing remains at
the protocol owner with its real daemon/CLI consumers. Root/S1/P1/Q1 argv operations retain their existing route.

The designated verifier owns one lock resolution and minimal locked protocol/client plus both-binary
check/strict Clippy feedback. Native qualification uses the actual search consumer; no dummy provider is added.
