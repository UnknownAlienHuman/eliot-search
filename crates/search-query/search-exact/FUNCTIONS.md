# Exact search over retained evidence

The disconnected Exact Rust package, proof/spine APIs, manifest and exclusive fixtures are removed.
The current Search command is ranked text retrieval; it does not claim exhaustive exact matching.
RAW and retained DecodedUtf8 are connected through the actual CLI/daemon and the
[same-session window](../search-continuation/FUNCTIONS.md). The registry owns current delivery/qualification status.

## Implemented raw slice

    eliot-search exact --raw --literal TEXT --data-root ROOT [--source-id UUID] [--limit N]

[raw_exact.rs](../../../bins/eliot-searchd/src/raw_exact.rs) scans the UTF-8 literal's bytes over undecoded
retained Source objects, without Qdrant, live-file reads, preparation or publication. The literal is nonempty
and at most 4096 UTF-8 bytes; arbitrary non-UTF-8 needle input is not supported by this String CLI.
PublishedView::source_inventory supplies the selected committed sources, including Empty and PreparationGap
inputs without points. Latest current/unpublished source rows never replace the pinned revisions.

One private matcher preserves overlapping matches and state across bounded byte blocks, resetting per source.
Each Source ObjectRef is verified by length/BLAKE3 before its occurrences become evidence; one object buffer
is retained at a time. RawSourceRef carries namespace/source IDs, row/registration revisions and the Source
ObjectRef. Matches contain raw byte start/end and exact bytes_hex, not prepared-unit or line coordinates.

The complete result accounts selected/scanned/missing sources, scanned bytes and total matches. --limit N
bounds whole emitted match/gap items to 1..64; it does not end input scanning. OutputLimit and scan_complete
are separate facts. Missing Source objects are explicit SOURCE_OBJECT_MISSING gaps; corruption is fatal.
complete_negative is true only when every selected source was verified/scanned with zero matches and no missing
input. The shared total frame cap is 8 MiB; no range or matched bytes are clipped.

## Retained DecodedUtf8 identity mode

    eliot-search exact --decoded-utf8 --literal TEXT --data-root ROOT [--source-id UUID] [--limit N]

Exactly one of --raw and --decoded-utf8 is accepted. [decoded_exact.rs](../../../bins/eliot-searchd/src/decoded_exact.rs)
uses the selected committed inventory and its saved source/preparation/settings binding. For a Ready entry it
verifies both retained Source and Prepared ObjectRefs, decodes the SAVED artifact and verifies that its entire
UTF-8 representation bytes equal the retained Source. This proves the identity byte map; BOM and CRLF remain.
It does not decode a live/raw source on demand, materialize, normalize, reprepare or contact Qdrant.

The matcher scans the full saved representation, crossing unit descriptors as well as bounded scan blocks.
Each match carries RawSourceRef/input, Prepared ObjectRef, representation_start/end, source_start/end and
exact UTF-8 text. Source/representation byte ranges coincide only because this identity map was verified.
PreparationGap yields PREPARATION_GAP with prepared=None. Missing Ready input/artifact yields SOURCE_OBJECT_MISSING
or PREPARED_OBJECT_MISSING with Some(selected PreparedRef). Corruption is fatal, never a gap/negative fallback.
Raw remains independent of Prepared availability.

DecodedUtf8Result separates selected/scanned/gap sources, scanned bytes, total matches and the emitted prefix.
Existing 4096-byte UTF-8 literal, 64-item and 8 MiB total-frame bounds remain. OutputLimit does not stop scanning;
complete_negative requires every selected representation verified/scanned, zero matches and no representation gap.
Empty Ready can complete negatively; an unavailable representation cannot.

Same-session Continue/Expand/expiry use the connected window owner and original representation. Its
[verification scope](../search-continuation/FUNCTIONS.md#verification-scope) preserves the selected Windows/Release limits.
Extracted has no current real producer or implemented route. Arbitrary non-UTF-8 needle input is not implemented.

## Input semantics are part of the request

Raw bytes, explicitly decoded text and extracted document text are different requests.
Raw literal search scans the exact retained bytes without implicit decoding or normalization.
Decoded/extracted modes require their real retained representation and mapping reader; unsupported
input is a gap, never an empty successful scan. ASCII-insensitive matching is not Unicode case folding.
A no-match concerns the selected retained revisions, not the unobserved live files.

## One execution

Use one pinned source inventory and predicate. Validate finite pattern/input/output bounds even for
empty selection, compile the private matcher once, and read one retained object at a time.
Preserve overlapping matches and matcher state across byte blocks; reset at each source boundary.
Byte blocks need not be independently valid UTF-8. Check the original deadline/cancellation between
bounded blocks and before more reads/output. Charge actual consumed input and integrity work.

## Integrity, memory and output

Verify the retained object before emitting its matches as evidence. Account for any whole-object
buffer/hash requirement. Keep exact ranges in their own representation domain.
Presentation limits and aborted input scanning remain distinct. Complete negative requires every
selected item verified and scanned, zero matches and no missing/unsupported/limit/cancel gap.
Empty selection never widens. Paging may use the existing session window; it does not complete a scan.

## Regex and other predicates

Add another predicate only with an actual consumer and explicit engine/domain/bounds.
Do not infer regex cross-block or anchor behavior from independently evaluated chunks.
No compile-plan service, supplied-readback proof, durable job, permit or receipt API.
