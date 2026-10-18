# Working on exact scans

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [README](README.md) and
[current function map](FUNCTIONS.md).

Connect an explicitly requested scan to the real selected source inventory and retained revisions.
Use the existing source/readback owner and one matcher loop. Change these APIs and their daemon callers
together; no new inventory port, durable job platform or package split is required.

Reuse the Aho core in `literal.rs` and remove `proof::find_literal` with its last caller. Compile once
per predicate, carry state across chunks of one source and reset it between sources. Keep ASCII-only
case folding consistent and raw-byte versus decoded-text semantics explicit.

Read and verify each retained revision as work advances. Bound reads before allocation and release
buffers after scanning; a preloaded `Vec<ExactItemReadback>` cannot provide a bounded read path.
Use actual source identity, bytes, coordinates and selected client/scope restrictions. Supplied digest,
receipt or trust flags do not establish that a file was read correctly.

Keep the stated inventory and actual completed/failed/unread accounting. Preserve useful matches when
execution is incomplete, while reporting limits, cancellation, unsupported input and missing revisions.
Complete-no-match requires every item to be evaluated on its specified revision. Newer files at the
same paths, ranked candidates and empty symbol-index results cannot replace that work.

Remove duplicate denominator encodings, generic fence digests and unused checkpoint/receipt machinery
with their consumers. A simple interactive scan needs no proof lifecycle or compatibility platform.
Check the integrated behavior that can fail: chunk/source boundaries, overlaps, non-ASCII text, byte
limits before reads and incomplete inventory/readback outcomes.
