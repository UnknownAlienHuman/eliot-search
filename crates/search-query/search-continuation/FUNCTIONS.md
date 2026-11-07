# Same-session query windows

The old continuation crate/stores/manifest/exclusive fixtures remain removed. The connected candidate at
source 473447af uses the real CLI, public SDK and daemon; it does not introduce another window service.
Implemented policy and the limited executed native scope below are separate facts. X1 remains PARTIAL.

## Current consumer and usage

    eliot-search session --data-root ROOT [--qdrant-url URL] [--config-file PATH] [--daemon PATH]

[CLI search_command.rs](../../../bins/eliot-search/src/search_command.rs) reads one SessionClientFrame JSON
object per input line and emits SessionReply JSON records. This line-oriented CLI facade is implemented;
the [OwnedQuerySession SDK](../../search-provider-client/src/lib.rs) and daemon use the existing bounded
length-prefixed frame codec. The CLI launches --session-stdio, not the one-shot --search-stdio route.
SDK operations are open/submit/next_reply/request/finish on the caller's Tokio runtime; finish observes
child wait and stdout/stderr EOF. Dropping without finish is emergency cleanup, not successful completion.

Enter one record and consume its correlated reply before the next data operation. This Raw example assumes
Open emits item_index 0 in window_id 1; choose a literal present in the already published inventory:

    {"type":"open","request_id":1,"remaining_ms":120000,"query":{"mode":"raw","request":{"literal":"timeout","source_ids":null,"limit":8}},"page_size":2,"ttl_ms":120000}
    {"type":"expand","request_id":2,"remaining_ms":120000,"window_id":1,"item_index":0,"before_bytes":0,"after_bytes":0}
    {"type":"continue","request_id":3,"remaining_ms":120000,"window_id":1,"page_size":2}
    {"type":"close","request_id":4}

These fields come from [SessionClientFrame/WindowQuery](../../search-provider-protocol/src/dto.rs).
WindowQuery.mode is search, raw or decoded_utf8, with its existing request payload. Only search needs a
Qdrant endpoint. Open is accepted once per child; a new original query requires a new session. Data request
IDs are nonzero and increase; Cancel names correlated active work rather than creating another data request.

## Fixed window and paging

Open executes the original query once and retains its final-order prefix of at most 64 entries, its pinned
PublishedView, cursor and source bindings. Exact total found_matches/scanning facts remain separate from
retained_items; Continue never promises all Exact matches beyond this original prefix or rescans/repins.
Open/Continue page_size is 1..64, default 16. Pages expose window_id, start_index, next_index, original summary,
whole WindowEntry values with zero-based item_index, has_more and page_stop. Frame-size cuts retain whole items
under the existing 8 MiB total cap. has_more concerns stored entries, not unbounded matches.

The fixed TTL starts at accepted Open BEFORE query work/root acquisition/pinning; default and maximum are
120000 ms. Access, Continue and Expand never renew it. Each operation's remaining_ms is also bounded by the
original window expiry. Later source/preparation changes cannot replace the pinned inventory or representation.
Only completely flushed pages commit cursor/emitted indexes. One-shot results have no reusable locator.

## Expansion

Expand requires explicit before_bytes and after_bytes and an already emitted (window_id, item_index) in this
same child. The returned neighborhood includes the original span; its TOTAL is at most 65536 bytes.
Underflow/out-of-range or UTF-8 scalar-splitting endpoints are refused, not silently clipped/normalized.
Search/DecodedUtf8 context uses the actual saved UTF-8 representation and verified Source with the supported
identity map; Raw uses verified Source bytes and returns hex. Original refs/ranges/text must still match.
Gaps are not expandable matches. No arbitrary revision offsets, latest-source lookup or live fallback is added.

## Operation ownership

The I/O owner services data/control while the joinable worker owns the root/view/window. Prepare a page's
cursor update before output and apply it only after the complete daemon frame flush. Local flush is not peer
acknowledgement. Partial output commits no page progress and closes the session. No writer/view/window mutex
or mutable shared borrow spans reads, encoding or output.

Healthy pre-output failures preserve the prior cursor/window. SDK local CommandRejected writes no daemon
command and leaves the session usable. The CLI identifies such a record with origin:"client"; it is not a
daemon SessionReply. Any such failure makes aggregate CLI exit 2 even when later operations/Close succeed
and the daemon exits 0.

Expiry, EOF/disconnect and Close cancel/join active work, release the root/view and finish/reap the child;
the implemented expiry/shutdown path also services output backpressure. No detached timeout worker, durable
resume, renewal, grants or second state platform is added. Runtime qualification is limited as recorded below.

## Verification scope

Source review PASS at 473447af and focused locked check/strict Clippy/RELEASE builds passed. Actual native
proof supplied so far is Raw only: public Open -> client-origin refusal -> valid Expand -> Continue -> Close
(CLI PID 103172 exit 2, daemon 13692 exit 0, both EOF and OS absence). A direct-server guard/two late-Cancel/
three-page/Close run (helper 104060, daemon 23456, both exit 0) received replies 1..6 exactly once and preserved
CONTROL plus 16 objects.

Held-output deferred Cancel, ranked/Decoded windows, TTL/EOF, Busy release and paging-boundary qualification
remain pending. Do not promote source-reviewed lifecycle policy or these two Raw runs into those results.
X1 remains PARTIAL, M1 WAIT and V1 unfinished. The current prepared-representation producer is saved UTF-8 identity;
new extracted/parser/nonidentity routes are not selected gates before M1. Historical Markdown/JATS are not
restored, and unavailable representations remain explicit gaps rather than invented text.
