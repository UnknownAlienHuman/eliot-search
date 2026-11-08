# One native owned-child query

## Public consumer

`QueryClient::new()` constructs one Tokio 1.53.1 current-thread I/O runtime outside an async context.
`query_owned_child(&mut self, executable: &Path, args: &[OsString], request: SearchRequest,
deadline: Instant, cancelled: &AtomicBool)` returns QueryCompletion or content-free ClientError.
raw_exact_owned_child has the same ownership/deadline arguments with RawExactRequest and returns the same
QueryCompletion. Both methods use the serviced child I/O/reap owner; the raw result is additionally validated
against its literal, published-source selection and output limit. Raw matches stay in bytes_hex form.
decoded_utf8_owned_child takes DecodedUtf8Request with the same executable/args/deadline/cancellation ownership.
It validates the complete decoded terminal against literal, selection, retained references, identity ranges
and item bound before returning QueryCompletion. It performs no materialization or on-the-fly source decoding.
The CLI supplies native invocation settings, starts the original deadline before setup and owns safe rendering.

The three QueryClient owned-child methods send include_summary=true and validate requested summary presence,
selection/counts and Known revision/lag consistency along with the result. A typed Unavailable CONTROL observation
preserves an otherwise valid query result. It does not excuse evidence/transport/cancellation failures. Legacy
omitted/false wire requests retain their old shape; this is not a new CLI switch.

The CLI explicitly calls enable_ctrl_c() before its query. Other callers leave signal handling disabled.
The maintained Tokio listener is first polled and registered before child spawn, then the same pinned future
remains in the existing I/O loop. A separate child process group routes terminal Ctrl-C through the parent
and its correlated Cancel/cleanup/reap path. Tokio's opt-in signal disposition lasts for the CLI process.

QueryCompletion carries the complete validated Terminal, retained diagnostics and diagnostics_truncated.
No child or pipe escapes. Complete success requires a successful observed child exit. Application Failed and
Cancelled remain typed terminals. A spawn/transport/truncation/deadline/reap failure never becomes an empty result.

## Saved source-info opt-in

QueryClient::set_include_source_info(true) opts subsequent owned-child calls into captured saved-path annotations.
The default is false; existing CLI/SDK calls and omitted/false wire fields keep their earlier shape. There is no
new CLI launcher flag or source_info lookup RPC. The client validates source_info presence against the request
and its typed/bounded shape; it does not verify the live path. NotChecked and Unavailable are annotation state,
not permission to replace invalid evidence with live files or empty success.

The new [eliot-search-mcp consumer](../../bins/eliot-search-mcp/FUNCTIONS.md) uses the existing SDK and defaults
its tool request annotation choice on. Candidate3 has CORE/MCP SOURCE PASS with the reviewed transport/handle/
output P2 items closed and Sol-accepted SELECTED_NATIVE_PASS, including cleanup/continuation. Native operational
host model/context work remains pending in [AD1](../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery);
four updated coding-client MCP entries do not prove that runtime path.

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

## Session summary binding

OwnedQuerySession records Open's include_summary choice and requires WindowSummary.corpus presence to match it.
Open/Continue pages keep the exact original summary; the client rejects changed page summaries. The CLI enables
summary on Open before submission. Expand uses the original emitted item/representation without refreshing the
summary or scanning current source rows. Open also records include_source_info and validates per-item/expansion
annotations against that choice. Hints stay bound to original captured evidence; output fitting can return typed
Unavailable without asserting a live path change. Existing finish/wait/EOF ownership remains unchanged.

## Removal and checks

The former typed pairing/profile/grant/MAC/replay/TCP/native endpoint client and its exclusive example/fixtures
are physically retired, along with the Windows named-pipe adapter package. Generic local framing remains at
the protocol owner with its real daemon/CLI consumers. Root/S1/P1/Q1 argv operations retain their existing route.

The designated verifier owns one lock resolution and minimal locked protocol/client plus both-binary
check/strict Clippy feedback. Native qualification uses the actual search consumer; no dummy provider is added. The deliberate SDK negative
for a missing requested summary is NOT_RUN; source validation is not relabelled as that native case.
