# Working on document extraction

Read the root architecture and implementation, then [FUNCTIONS.md](FUNCTIONS.md). The current binary
only reports unavailable; the old function catalogue is not an implemented provider.

Deliver one real format through retained input → parser → page/text coordinates → materializer/units →
searchable evidence. Change this binary and its immediate daemon/preparation consumers together.
An echo worker, provider descriptor or process supervisor alone does not complete document support.

Use the selected parser directly in this child process. Keep source retention and publication in the
daemon. Use owned stdio and concrete input/output types; reuse framing where useful. Do not build a
plugin registry, worker certificate protocol, per-phase receipts or another universal document model.

Start with native PDF text. Page and extracted-text coordinates are useful; they are not offsets into
the original PDF byte stream. OCR, layout models and scientific metadata are selected extensions.
Do not run all extractors on every document or reparse unchanged revisions on a query.

Keep input/output and process resources bounded, propagate one deadline, and reap cancelled or failed
children. Do not execute document actions or fetch linked resources. Use the donor parser rather than
writing a second parser to pre-inspect every PDF object. Select only runtime dependencies actually needed
by the format; a required OCR executable is different from executing indexed document code.

Verify useful text/location output, malformed input, cancellation and daemon integration. No P15/G6
receipt, provider ADR ceremony, source quota or legacy format compatibility is a coding prerequisite.
