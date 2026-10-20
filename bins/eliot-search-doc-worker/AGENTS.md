# Working on the document worker

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [source map](FUNCTIONS.md),
[#216](https://github.com/UnknownAlienHuman/eliot-search/issues/216) and
[#227](https://github.com/UnknownAlienHuman/eliot-search/issues/227).

The current package is a refusal stub with an unlinked hand-written SHA-256 file and four unused internal
dependencies. Do not harden, qualify or preserve that scaffold. Replace it atomically with the selected PDF
path, or delete the workspace member until #227 implementation begins.

For #227, the parent supplies the exact retained PDF object and owns identity, deadline, cancellation,
process/resource limits, termination/wait and cleanup. The child binds one explicitly packaged matching
PDFium library, extracts finite page text/coordinates and returns bounded typed page results with explicit
gaps. It writes no redb/object/Qdrant state and never reopens a source pathname.

Delete `src/sha256.rs`; the worker does not mint source identity. Use the common object/artifact identity only
where the parent actually persists a derived representation. Remove unused `search-ports` and old provider
protocol dependencies; use one small private/current frame, not a generic worker/provider platform.

Extracted text is a mapped representation, not `ExactBytes` of the binary PDF. Change the real
materializer/unitizer consumer with the worker and persist one page/region map. Native text first; OCR/layout/
GROBID/other formats are separate selected profiles, never automatic fallbacks.

After code, run minimal Rust 1.98 compiler/Clippy for the worker and actual preparation/daemon consumers, then
exercise real retained PDFs through publication and source-backed query. A refusal stub, echo provider or
constructor test is not completion.
