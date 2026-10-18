# eliot-search-doc-worker

Optional document extraction in an owned child process. The daemon supplies a retained revision and
receives text with page/region coordinates and explicit extraction gaps. Preparation, storage and
publication continue through the same application owners.

**Current source:** [main.rs](src/main.rs) ignores arguments, reports
`DOCUMENT_PROVIDER_NOT_QUALIFIED` and exits with code 2. No parser, request loop or daemon integration is
implemented here. The unreferenced `src/sha256.rs` is not a worker implementation.

**First implementation:** native PDF text through `pdfium-render`, with a selected local PDFium library;
no page rendering, OCR or model process for text already present in the PDF. This is a donor choice for
implementation, not a dependency already in Cargo.lock or a measured Windows result. The
[change map](FUNCTIONS.md) explains data flow, donor alternatives and real limits.

Reuse [root rules](../../AGENTS.md) and [local instructions](AGENTS.md). A real searchable document
replaces the old runtime-only/qualification ladder in #227; no generic framework must precede it.
