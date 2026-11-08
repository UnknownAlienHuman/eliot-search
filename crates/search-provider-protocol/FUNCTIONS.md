# One child query wire

## Actual API

`SearchRequest { query, limit }` is shared by CLI and daemon. `ClientFrame` has JSON tag `type`:
Query, RawExact and DecodedUtf8 carry `request_id = 1`, `remaining_ms`, `request` and optional include_summary /
include_source_info. Cancel carries that same ID. Both flags default false and are omitted when false;
include_source_info=false/omitted preserves the legacy result shape without source_info fields.
Query UTF-8 is nonempty, at most 4096 bytes, without NUL; limit is 1..=64. Remaining duration is
1..=120000 milliseconds. The parent samples it at send and independently enforces its original deadline.
The receiver charges observed partial-frame time to the duration; processes never exchange Instant values.

`Terminal { request_id, outcome, summary? }` has one `QueryOutcome` tagged `status`: Complete contains
generation, ordered items and stop (complete/output_limit); RawExact/DecodedUtf8 contain their separately validated results;
Cancelled contains no result; Failed carries
a validated 1..=64 byte uppercase/digit/underscore FailureCode. Unknown and duplicate DTO fields fail.

`SearchItem` is tagged `kind = hit|gap`. Both carry original contiguous one-based rank, finite f32 score
(any sign) and EvidenceMetadata. Hit adds the exact full UTF-8 unit_text, at most 65536 bytes.
Gap adds only SOURCE_OBJECT_MISSING or PREPARED_OBJECT_MISSING. Corruption/routing/range failures
are Failed outcomes, never missing-object gaps or fabricated empty results.

EvidenceMetadata contains generation, lowercase point_id (32 hex), canonical source_namespace_id/source_id
UUIDs, input/prepared ObjectRefDto, unit, start/end, line_start/line_end and both line-boundary flags.
ObjectRefDto is version/kind/digest/bytes, with version 1 and kinds source/prepared. Source bytes are at
most 8 MiB; a prepared object is nonempty and at most 64 MiB. The point ID binds the terminal generation.
Rank and score remain outside the immutable retained citation; no live-path claim or revision ledger is sent.

The daemon resolves evidence and its captured source/preparation from the same published read snapshot,
verifies both retained objects and decoder/range/line bindings, and supplies the full unit. The wire's
structural checks do not perform those storage/backend checks.

## Saved captured-source annotations

include_source_info opts into optional source_info on Search/Raw/Decoded items and session evidence. Available
contains saved_path_hint and live_status: not_checked. The hint is decoded from that pinned CapturedSource's
saved native locator, not latest current-source state; it makes no live stat/open, existence, content or freshness
claim. SourceId/ObjectRefs/ranges remain the evidence identity. There is no public source_info RPC.

Unavailable also has live_status: not_checked and a closed reason: invalid_locator, unsupported_platform,
non_unicode_locator, allocation_failed or output_limit. Annotation unavailability does not turn valid evidence
into a gap; actual retained evidence corruption still fails. Requested presence and bounded string shape are
validated; false requests omit the field rather than serializing null. Session Open uses the same default false.

Annotation bytes are charged within the unchanged 8 MiB TOTAL owned-child frame cap. Exact fitting may replace
optional path hints with typed Unavailable/output_limit while preserving match items/ranges and scan accounting;
ranked/window output retains whole-card/page bounds. These are private SDK frame rules, not a blanket size or
allocation guarantee for the new [MCP transport](../../bins/eliot-search-mcp/FUNCTIONS.md). Candidate3 has MCP
SOURCE PASS; its selected native results and pending client qualification are scoped separately in
[AD1](../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery).

## Query scope summary

A successful opted-in terminal carries QuerySummary; old-wire/false requests omit summary. It contains
published_wanted_revision, published_sources, published_preparation_gaps, selected_sources and
selected_preparation_gaps. ControlObservation is Known { wanted_revision, known_control_lag } or Unavailable
with the closed reason Unavailable, UnsupportedSchema or Corrupt. Known lag is exactly current wanted > published
wanted; neither branch claims filesystem freshness. Counts describe published preparation outcomes, not emitted
cards or Raw scan completeness. A preparation gap alone does not invalidate a complete Raw negative.

Validation enforces the existing 4096-source bound, subset/gap counts and selected non-gap <= published non-gap,
plus request selection and revision/lag consistency. Requested successful summary presence is checked; metadata-
only Unavailable is a valid header, not an empty result or suppressed evidence failure. Cancelled/Failed omit it.
Session Open has the same default-false include_summary; WindowSummary.corpus caches the original header for
every page. Existing total frame limits include the encoded summary; no quota is raised.

## Raw Exact request and result

RawExactRequest { literal, source_ids, limit } uses a nonempty UTF-8 String of at most 4096 bytes as a literal
byte needle over undecoded retained input. Arbitrary non-UTF-8 needle encoding is not implemented. The CLI
supports one optional --source-id; the DTO uses canonical unique UUIDs, omitted = all published sources and
explicit empty = none. Limit is 1..=64 whole match/gap items, independent of input scan completeness.

RawSourceRef binds namespace/source UUIDs, row/registration revisions and the Source ObjectRef. A raw Match
contains source/start/end/bytes_hex; a raw Gap contains source and only SOURCE_OBJECT_MISSING. No PreparedRef,
unit/line coordinate or ranking score is invented. Corruption produces Failed, not a missing-object Gap.
RawExactResult carries generation, selected_sources/scanned_sources/missing_sources, scanned_bytes, total
matches, items, scan_complete, complete_negative and stop. Complete negative requires every selected source
verified/scanned and zero matches/missing input. OutputLimit describes an emitted prefix, not incomplete input.
The client validates literal bytes, selection and limit against the complete correlated result.

prepare_raw_exact encodes the producer's already bounded whole-item prefix under the same 8 MiB total frame
cap, including its optional summary; it clips no match/range and rejects an oversized frame. Ranked prepare_complete below retains its own
whole-card sizing behavior. Both operation variants use the existing Cancel/terminal/EOF/reap path.

## Retained DecodedUtf8 request and result

DecodedUtf8Request has the same literal/source_ids/limit bounds as RawExactRequest and a distinct operation tag.
DecodedUtf8Match carries RawSourceRef/input, Prepared ObjectRef, representation_start/end, source_start/end and
exact text. The source and representation ranges must match the verified retained UTF-8 identity map; text byte
length equals the range and request literal. The wire validates structure/request binding; the daemon verifies
the actual stored objects, saved representation and source equality before producing the match.

DecodedUtf8Gap has prepared=None only for PREPARATION_GAP; SOURCE_OBJECT_MISSING and PREPARED_OBJECT_MISSING
retain Some(selected Ready ObjectRef). Integrity failure is a Failed terminal. Result counters include
selected_sources/scanned_sources/gap_sources/scanned_bytes/matches plus items, scan_complete/complete_negative
and stop. A presentation prefix is separate from full representation scanning. Any representation gap prevents
complete negative. prepare_decoded_utf8 encodes the bounded result under the unchanged 8 MiB total cap.
The production codec's correlated Cancel and remaining-duration/QUERY_DEADLINE path is shared with other queries.

## Framing and bounded output

`encode_client_frame` / `decode_client_frame` and `encode_terminal_frame` / `decode_terminal_frame`
use complete u32-LE length-prefixed Serde JSON. MAX_FRAME_BYTES is the shared 8388608-byte total cap;
MAX_BODY_BYTES is four fewer. MAX_CLIENT_FRAME_BYTES is 28672, including the prefix.

`FrameBuffer` retains partial prefix/body positions and checks declared length before allocation. A push
consumes only the current frame, preserving visibility of extra/trailing bytes. Callers use cancellation-safe
read/write operations with explicit cursors; Cancel bytes never enter a partial data frame.

`prepare_complete(request_id, generation, items, summary)` validates all supplied items/header, then uses capped real-Serde
counting passes to select the largest whole rank prefix. It serializes the final fitting terminal before any
output byte. Omitted tail items produce OutputLimit. No 25 MiB body is allocated and no unit/frame is clipped.
A terminal is exposed only after complete decode, correlation, count, rank, finite-score, citation and text-byte
validation. One handled query emits at most one terminal; damaged/truncated/alignment-unknown streams close.

## Lifetime and verification

The client owns its child and all pipes through observed OS exit/reap. The daemon owns one joinable synchronous
Backend worker outside its I/O runtime and retains root/view through terminal flush. Existing argv root,
retention, preparation and publication operations remain separate consumers.

Native Rust tests cover split/oversized/truncated/extra frames, closed schema, UTF-8 byte binding, ranks,
correlation, gaps and real worst-case whole-unit OutputLimit. Compiler/Clippy and actual binary/native
qualification are recorded by the designated verifier; source readiness alone is not a passing gate.
