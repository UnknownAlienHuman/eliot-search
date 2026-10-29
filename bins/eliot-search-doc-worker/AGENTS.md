# Working on the document worker

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), local [source map](FUNCTIONS.md),
[#216](https://github.com/UnknownAlienHuman/eliot-search/issues/216) and
[#227](https://github.com/UnknownAlienHuman/eliot-search/issues/227).

The current package is a refusal stub with an unlinked hand-written SHA-256 file and four unused internal
dependencies. Do not harden, qualify or preserve it. Replace it atomically with the selected retained-PDF path,
or delete the workspace member while #227 is deferred.

`pdfium-render`/PDFium is an uninstalled candidate boundary, not a current dependency or frozen API. When the
real implementation begins, recheck and pin the then-current crate API/version/MSRV/advisories/features together
with one explicitly packaged matching PDFium native artifact. Record artifact source, license, hashes and
platform binding checks. Do not bind an arbitrary system library or auto-download one at runtime.

The parent supplies the exact retained PDF object and owns identity, deadline, cancellation, process/resource
limits, termination/wait, cleanup, validation and persistence. The child reads only that retained input and
returns finite page text/coordinates plus explicit per-page gaps. It writes no redb/object/Qdrant state and never
reopens a mutable source pathname.

Delete `src/sha256.rs`; the worker does not mint source identity. Remove unused `search-ports` and old provider
protocol dependencies. Use one small private/current frame, not a generic worker/provider platform.

Extracted text is a mapped representation, not `ExactBytes` of the binary PDF. Change the actual
materializer/unitizer consumer with the worker and persist one page/region map. Native text is the first profile;
OCR/layout/GROBID/other formats are separate selected profiles, never automatic fallbacks.

After code, run minimal Rust 1.98 compiler/Clippy for the worker and actual preparation/daemon consumers, then
exercise retained PDFs through publication and source-backed query. A refusal stub, candidate donor, echo
provider or constructor test is not completion.
