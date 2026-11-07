# Exact search over retained evidence

The disconnected Exact Rust package, proof/spine APIs, manifest and exclusive fixtures are removed.
The current Search command is ranked text retrieval; it does not claim exhaustive exact matching.
This is policy for the later real Exact consumer.

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
