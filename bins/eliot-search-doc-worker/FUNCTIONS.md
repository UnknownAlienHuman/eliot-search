# Document extraction — implementation map

The current [binary](src/main.rs) only exits unavailable. The following describes the selected
implementation, not existing function signatures. Use existing #227 for the real document path.

## Connect one PDF

1. The daemon selects an exact retained revision and supplies its bytes, or a read-only input derived
   from that same retained object. Never reopen the current source pathname as if it were that revision.
2. Start one owned child with bounded stdio and one document in flight. Bind explicitly to the packaged
   PDFium library; avoid accidental system-library fallback. Load the PDF, visit pages and extract text.
3. Return page text, extracted-text ranges, page number/size and available region coordinates, plus
   skipped/failed-page reasons. Keep the source revision and extraction settings alongside the artifact.
4. Parent preparation accepts the actual output once, builds units with those coordinates and publishes
   through the existing planner. Reopen results through the retained PDF and derived text artifact.

The current raw UnitSet path requires byte-identical UTF-8. PDF output cannot be fed into it with a fake
`ExactBytes` label. Change that real ingress together with materializer output; use existing coordinate
maps, and add only a concrete page/region representation that its consumer needs.

## Selected parser and alternatives

| Use | Donor and concrete API | Limit / decision |
|---|---|---|
| First native PDF text | [pdfium-render 0.9.4](https://github.com/ajrcarey/pdfium-render/blob/ea2961cb8ac777bb1a06dddb8efd272d34d74f18/README.md#binding-to-pdfium): `Pdfium::bind_to_library`, load retained bytes, `document.pages()`, `page.text()`, `PdfPageText::all` / `chars` | Select the matching PDFium ABI/library at packaging. Disable default `image_latest` when rendering is unused; retain appropriate thread serialization. PDFium is a native dependency, not included by the Rust crate. |
| Multiple selected formats in Rust | [Xberg source](https://github.com/xberg-io/xberg/blob/28c5311862e6f3466b4e762bf73a2ffe4b318dfa/crates/xberg/Cargo.toml), [Rust API](https://docs.xberg.io/guides/rust-core-api/) | An alternative when its format support removes more integration than the narrow PDF adapter. Use `ExtractInput::from_bytes`, `extract` and selected features; inspect per-input `errors` as well as `results`. Do not import its retrieval, agent, crawler or enrichment stack. Current `pdf` selects its native engine; it does not mean PDFium. |
| Existing Docling artifacts or selected layout/table extraction | [NativePdfPipeline](https://github.com/docling-project/docling/blob/0e54294231b69bc2cf7a39873235495788f76b57/docling/pipeline/native_pdf_pipeline.py), [options](https://docling-project.github.io/docling/usage/advanced_options/#extract-the-native-content-of-a-pdf) | Native pipeline needs no layout/OCR/table model but keeps Python/docling-parse and has parser order only. Full layout/table extraction is a separate cost. Import existing revision-bound output before adding another persistent service. |
| Scientific references/sections | [GROBID coordinates](https://grobid.readthedocs.io/en/latest/Coordinates-in-PDF/) | Selected `processFulltextDocument` + `teiCoordinates` gives TEI and native regions. Use only for the scientific corpus that needs this structure; it does not replace the lightweight PDF text path. |
| Scanned pages | [OCRmyPDF options](https://ocrmypdf.readthedocs.io/en/stable/cookbook.html#produce-pdf-and-text-file-containing-ocr-text) or the selected OCR engine | OCR only pages lacking usable text, with explicit language/settings and gaps. OCRmyPDF sidecar omits pages that already had text and skipped pages; never treat that file as complete document text. Merge page outputs or read the produced PDF. |

These sources were inspected on 2026-10-18. No new parser dependency is locked or runtime-qualified by
this Markdown change. Kreuzberg v4 LTS and current Xberg have different APIs; do not paste a v4 example
into an Xberg adapter. The first PDF choice need not wait for all optional formats or a comparison service.

## Useful coordinates and bounded work

PDF character order may differ from human reading order in columns or custom layouts; the
[text API](https://github.com/ajrcarey/pdfium-render/blob/ea2961cb8ac777bb1a06dddb8efd272d34d74f18/src/pdf/document/page/text.rs)
states this limitation. Keep page boundaries and represent extraction quality honestly. Extracted text
is not a byte slice of the binary PDF; a page citation remains useful without fabricated byte precision.
Add layout inference only when the selected corpus needs it. OCR changes recognition quality, not source identity.

Check input bytes and page/text counts before retaining outputs; iterate pages rather than building a
corpus-sized result. A page-size check cannot bound arbitrary allocation inside native parsing, so keep
the child memory/process limit and one total deadline. PDFium serializes calls rather than gaining CPU
parallelism from more Rust threads. Start with one worker; add a bounded process pool only for measured
throughput needs. No parser runs on ordinary query/readback.

The parent owns child termination/wait and temporary-file cleanup on success, error and cancellation.
Keep one terminal result with complete/partial/failed state. Partial page text may be useful but must carry
its missing-page gaps; a crash or malformed output cannot become a complete representation. Worker
output is ordinary typed data, not a receipt chain, credential exchange or independent source authority.

## Checks that matter

Exercise a text PDF, a multi-column page, a scan without native text, a malformed document and a cancelled
large document through the real daemon/preparation/result path. Verify native page coordinates and
retained revision identity, honest gaps, resource cleanup and that a normal query reuses the prepared
artifact. Do not accept an echo provider or a passing constructor as format support.
