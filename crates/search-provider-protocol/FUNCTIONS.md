# Local protocol — one framed session and one commit boundary

This file owns framing, request/session lifetime, cancellation and transport failure.
[Windowing](../search-query/search-continuation/FUNCTIONS.md) owns cursor/item lifetime;
[client](../search-provider-client/FUNCTIONS.md) selects concrete I/O mechanisms;
[CLI](../../bins/eliot-search/FUNCTIONS.md) owns invocation and presentation.
This is the replacement recipe, not a connected-implementation claim.

## Inspected source

R013 found DIRECT's synchronous read/execute/read loop and result display before terminal. R022 found
completed-ID history blocking Cancel, synchronous output starving controls and expired data budgets preventing
cleanup transmission. R023 separated the native declarations, obsolete bootstrap and actual legacy CLI route.

R024 at `676135f4` read the remaining CLI transport/response/cancellation helpers and the historical daemon
canonical I/O/serving path. Its input timer begins when poll_record is first called, before any bytes arrive;
that old timestamp is later used to charge the incoming request. Its 25-ms task quantum does not bound a
whole-record output call. Preserve the old helper's useful partial-frame/accounting continuity, not these
lifetime rules, strict JSON field order, MACs or multi-record acknowledgements. Exact evidence is in the registry.

## Wire and terminal visibility

Use one wire for one-shot and live-session modes:

```text
u32 little-endian body length + one bounded UTF-8 Serde JSON message
```

Reject zero/oversize length before body allocation. Decode a current DTO and validate fields/collections before
dispatch. JSON member order and insignificant whitespace are not protocol identity. Required typed fields and
correlation still matter; a syntactically valid object is not automatically a valid result. Frame size does not
bound downstream allocations or work.

Encode with serde_json::to_writer into a capped local Write sink before any frame byte. One terminal contains
bounded cards/gaps, stop/truncation state and usable session locators. The client exposes it only after the entire
frame decodes and validates. Progress is optional: omit it initially; when needed coalesce the latest update and
cap cumulative events/bytes. It contains no cards or locators dependent on a later commit.

A handled request has at most one terminal. Transport failure may deliver none. Local flush is not peer
acknowledgement, exactly-once delivery or proof of rollback.

## One active request and live control

Dispatch only constructed operations. Version, a non-reused session-local request ID, finite remaining duration
and operation form the envelope. Trusted owned-child stdio needs no pairing, MAC, grant or capability receipt;
source selection still belongs to the daemon. [Local scope](../../docs/architecture/LOCAL_SCOPE.md) excludes
remote/restricted-user authentication; do not add that protocol as a future placeholder.

One active data request is sufficient. Cancel names that request and consumes neither another work slot nor
completed-ID history. A late Cancel cannot cancel the next ID. Counter exhaustion closes instead of wrapping.
Require increasing data-request IDs and retain only the last admitted ID. Reject a reused or older data ID;
Cancel/Close do not advance that counter. Coalesce redundant control signals; do not build a replay ledger or
a general work scheduler.

Service reads, writes, control and deadlines while work is active. A synchronous public call may wrap this same
serviced runtime. Never hold the only session owner inside a full write/read loop or block the event-loop thread
with a synchronous handler and call the result cancellable.

Retain one outbound buffer/cursor and bounded inbound prefix/body state across turns. A partial frame cannot
contain an inserted Cancel. Finish within its existing bound or close. Under backpressure the other I/O direction
must still progress. Changing from data work to cleanup does not reset counters or discard an already consumed
prefix. The client recipe specifies cancellation-safe donor calls; async syntax alone supplies none of this.

## Distinct clock origins

Keep idle policy, incomplete-frame timeout, request deadline and window expiry separate:

- Caller: start the deadline before request-specific setup/spawn/encoding. New requests get new deadlines;
  nested operations and polling turns do not renew them.
- Wire: send the remaining duration sampled when preparing the frame, not the original timeout after setup.
  Continue enforcing the caller's own deadline during send/receive.
- Receiver: no request exists while waiting for its first byte. Use idle policy there. Start the frame clock
  on the first observed byte and retain it across partial reads. Charge observed framing/queue time to the
  request's remaining duration; cap by operation limits and window expiry. Do not reuse the timestamp of an
  earlier empty poll. Example: twenty seconds idle must not consume a later one-second operation budget.
- Cross-process: a duration does not reveal prior transport delay; no serialized Instant or identical process
  epoch is assumed. The caller can cancel/close at its own deadline, but external effects may remain uncertain.
- Cleanup: after data expiry a finite separate allowance may send best-effort control on an aligned stream,
  close and terminate the owned child. It cannot renew work or finish an expired partial data frame for reuse.

A pre-I/O clock check cannot interrupt a blocked call. Parent async child-pipe support does not make the daemon's
inherited stdin or the CLI console interruptible. A process-scoped reader may be used with the explicit parent-
close/owned-child-termination fallback; no detached worker per timeout and no hard OS timing guarantee from a
plain wait. Publication, not transport, resolves uncertain external writes.

## Cancellation versus terminal output

Decide on the one session owner, without a new commit protocol:

| State | Action |
|---|---|
| Before the first terminal byte | Cancel stops work. Discard uncommitted page state; a small cancelled/error terminal may use the still-valid output allowance. This ordinary outcome need not destroy a healthy aligned session. If no allowance remains, close. |
| A terminal frame has started | Keep that exact frame/meaning. Finish within its existing deadline or close; never replace its suffix or send a second terminal. Cancel cannot retrospectively rewrite it. |
| Complete write and local flush succeeded | Apply the already prepared local window update before dispatching the next data request. A late Cancel for the completed ID is a no-op. |
| Partial I/O, malformed framing or uncertain alignment | Close; commit no new window progress. Previously observed bytes and external effects are not rolled back. |

A cancelled/error terminal does not expose new cards/locators requiring a cursor commit. Valid server partial/gap
results are typed application outcomes, not transport errors. Never reinterpret a cancelled read as an absent
candidate. User-level errors before any write are not a reason to poison an otherwise healthy session.

## Session and window lifetime

One-shot returns no reusable locators and then closes/reaps its child. A live session keeps the same streams
across Search/Continue/Expand. A new process cannot revive an old window.

Before terminal output, reserve the pending window slot/metadata, validate cursor arithmetic and stage item
indexes. After successful local flush, commit is an infallible local assignment with no new allocation, source
read, policy requalification or backend call. Serialize the next data dispatch after it. This prevents an
avoidable failure after advertising a locator; it does not claim atomic delivery across a crashed connection.
The prepared result owns its pending window state and view guard; it retains no mutable window/map borrow or
mutex across I/O. [Window operation ownership](../search-query/search-continuation/FUNCTIONS.md#operation-ownership)
owns reservation, error restoration, spent-work accounting and expiry. Do not add a second window transaction.

EOF/end/failure signals active work and removes session windows. Idle turns service expiry; active guards remain
until their operations actually stop. No cleanup daemon or durable resume ledger is required.

## Cutover and evidence

Change DTO/codec/client/dispatcher and real handlers together. Remove line/tab/hex/manual JSON, pairing, grants,
recipes and replay families with their callers. Keep useful framing, error distinctions and partial-I/O behavior.
After code, minimal pinned compiler/Clippy precedes later behavior checks: idle then short request; split frame;
Cancel before/during/after terminal; duplex backpressure; complete terminal before display; no fallible post-output
cursor work; long sequential sessions; EOF/expiry and owned-child cleanup. No R024 transport run is claimed.

Donor: [Tokio 1.53.1 select cancellation safety](https://docs.rs/tokio/1.53.1/tokio/macro.select.html#cancellation-safety),
plus the exact-version sources linked by the client recipe. A library's API documentation is not execution evidence
for the application's transport or pinned compiler.
