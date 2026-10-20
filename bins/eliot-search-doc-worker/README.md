# eliot-search-doc-worker — replace the stub only with real PDF extraction

The current workspace binary is not a document capability:

- [`main.rs`](src/main.rs) is a 1,073-byte refusal stub which ignores every input and exits 2;
- [`sha256.rs`](src/sha256.rs) is a 4,756-byte hand-written SHA-256 implementation which is not even linked
  from `main.rs`;
- `Cargo.toml` declares four internal dependencies which the binary does not use;
- there is no request loop, parser, daemon caller or searchable document result.

Do not preserve this scaffold. If #227 is implemented now, replace the package atomically with the concrete
PDF path and remove `sha256.rs` plus unused dependencies. If #227 is deferred, delete the package/workspace
member until that implementation starts.

## Selected first capability

```text
retained PDF ObjectRef
→ one owned PDFium child when FFI isolation is selected
→ native page text + page/region coordinates + explicit gaps
→ one MaterializationProduct
→ structure-first units
→ normal Qdrant publication/query
→ excerpt/location bound to the same retained PDF revision
```

The parent supplies exact retained bytes or a read-only input derived from that object. The worker never
reopens a mutable source path. The parent already owns source/revision identity; the worker does not hash the
binary again or mint a second revision identity.

Use `pdfium-render` with one explicitly packaged matching PDFium library for #227. Disable rendering/image
features when unused. One document in flight is the baseline. Parent owns deadline, cancellation, child
termination/wait, resource limits and temporary-file cleanup.

Worker output is ordinary bounded typed data, not a provider receipt or source authority. Return page text,
page number/size, available regions/coordinates and per-page skipped/failed reasons. Extracted text is a
derived representation, not `ExactBytes` of the binary PDF. Partial output remains partial.

No generic document-provider registry, universal parser protocol, capability digest, worker supervisor or
fallback chain is required. Safe Rust parsers may run directly in preparation; native/FFI/JVM/Python/OCR
parsers get a child only when their selected profile needs isolation.

[Implementation map](FUNCTIONS.md) · [Document portfolio #216](https://github.com/UnknownAlienHuman/eliot-search/issues/216) · [PDF #227](https://github.com/UnknownAlienHuman/eliot-search/issues/227)
