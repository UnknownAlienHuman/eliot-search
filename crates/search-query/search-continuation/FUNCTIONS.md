# Same-session query windows

The old continuation crate/stores/manifest/exclusive fixtures remain removed. The real CLI/SDK/daemon window
consumer is qualified for the selected Windows/Release X1 scope on main 59061070, using source-equivalent
473447af binaries. The [registry](../../../docs/audit/ANALYSIS_REGISTRY.md#work-register) owns current stage
status; this window qualification does not establish completion of M1 or V1.

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

Open's optional include_summary defaults false for legacy SDK/wire callers; the current CLI enables it.
When requested, WindowSummary.corpus contains the original QuerySummary from the pinned RO snapshot. It is
cached unchanged across pages and offline Expand; Continue/Expand never refresh current CONTROL or live files
for this header. Metadata-only Unavailable preserves valid results. Real encoded summary bytes count toward
whole-page frame fitting under the same cap.

Open also records default-false include_source_info. Requested per-item/expansion metadata derives from the
original captured Source locator, with live_status: not_checked or typed Unavailable; it never refreshes the
live filesystem or latest source head. Annotation bytes count toward existing private frame/page budgets and
may be unavailable for output size. SourceId/ObjectRefs/ranges and scan facts remain authoritative.

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

## MCP consumer status

The [MCP owner](../../../bins/eliot-search-mcp/FUNCTIONS.md) maps seven Search/Exact/window tools onto this
existing SDK. Its public schemas define inputs; MCP windows use the returned opaque window_handle, not the
private numeric window_id in the CLI example above. Open's self-contained schema/description on main ed8ce5b4
preserves resolved validation; DTO/parser/handlers and the other six schemas are unchanged.

The verified [package-v4 and canonical Skill](../../../integrations/eliot-search/README.md) are current for all
five owned registrations. The Skill and CLI/daemon are unchanged. Candidate3 selected SDK native cleanup/
continuation PASS and four coding-client v3 model workflows retain their original provenance; no v4 replay is
claimed. Codex 0.161, Claude Code 2.1.278, AGY CLI 1.5.3 and OpenCode 2.0.32 have accepted recorded native
Skill/tool/context/answer scopes, including OpenCode's effective native 001/002/003 answer corrections.

Desktop 4.23118.0 / Opus 5.5 Extra separately passed actual v4 normal Chat via official local mcpServers config
and the native account Skill. Search/Exact and retained Open/Expand/valid-handle Close returned saved Rust
context and citations. A numeric-handle refusal recovered using the actual handle; the answer's larger block
is labelled abridged and the short literal signature/match scope are corrected natively without new Root calls.
[AD1](../../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery) is DONE after Sol DESKTOP_NATIVE_ACCEPTED.
MCPB is optional BUILT_NOT_INSTALLED; no Install approval gate or Cowork qualification remains.

The shared g1 inventory is the registered selection of 361 sources/gap0, not whole-repository/live-file coverage.
App/backend were left functional, with no service/autostart or across-runtime-exit availability claim.
Existing X1/A10 evidence below keeps its exact historical source scope and is not relabelled as MCP proof.

## Verification scope

Final Sol incremental X1 gate PASS covers the source-equivalent 473447af/main 59061070 Windows/Release
consumer: public Raw client-refusal/healthy continuation, server guard/late Cancel; Decoded stable Source/
Prepared refs and original bytes after live-input mutation; ranked generation-2 offline Continue/Expand,
Busy Index and reopen after Close; public TTL/no-renewal/EOF/early TTL; calibrated held deferred Cancel and
held-expiry finite partial exit 2; empty/unknown selection, gaps without false negative and 64-prefix end.

The real 4 MiB UTF-8/64 hard-64 KiB-line run cut a requested page into 63 whole items then one: committed
cursor 63 then 64. All 64 texts/ranges were byte-exact at 65536 bytes each. Frames were 8300716/132042/50
bytes, each <=8388608. Sol independently verified the v4 raw stdout/hash. Session wait, both EOFs and OS
absence were observed; query CONTROL/objects remained unchanged.

A10's separate fixed9cfa native scope verified offline Expand over exact 183-byte [0,183) original data/refs,
then a page with the unchanged cached header: published wanted 1, current wanted 2, known lag true and published
sources/gaps 4/1. The earlier out-of-range +8 context request correctly returned WINDOW_RANGE_INVALID; it is
not failed valid expansion proof. TTL and 8 MiB boundary cases were not requalified for A10; prior scopes above
remain distinct. See the [candidate checkpoint](../../../docs/audit/ANALYSIS_REGISTRY.md#a10-v1-candidate).

Exact pre-exit worker/root release timing remains UNKNOWN; SDK-only unread remains NOT_PROVEN and is not
a separate selected gate. The earlier v1 Gridstore os3 cause remains UNKNOWN; short-path PASS does not
establish its cause. Setup init/one-shot daemon observer remains UNKNOWN wherever not actually observed.
These scopes do not qualify the whole project, other platforms or an installed final-head baseline.

The current prepared-representation producer is saved UTF-8 identity. New extracted/parser/nonidentity routes
are not selected gates before M1, and historical Markdown/JATS are not restored. Unavailable representations
remain explicit. The [stopped-root owner](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection)
owns the delivered selected GC/export/import scopes; its exact evidence stays in that recipe and the registry.
Window qualification does not establish completion of V1 or the whole product.
