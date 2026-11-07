# Exact search over retained evidence

The disconnected Exact Rust package, proof/spine APIs, manifest and exclusive fixtures are removed.
The current Search command is ranked text retrieval; it does not claim exhaustive exact matching.
FIRST RAW is connected through the actual CLI and daemon; the remaining X1 modes/windows are not complete.

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

DecodedUtf8 is next, using the existing retained UTF-8 Prepared representation and identity mapping. No decoded
or extracted route is implemented here; extracted has no current producer after the old parsers were removed.
Continue/Expand/expiry and held-output session/window qualification remain deferred. X1 is PARTIAL.

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
