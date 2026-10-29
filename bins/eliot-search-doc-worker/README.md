# eliot-search-doc-worker — replace or remove the refusal stub

The current workspace binary is not a document capability:

- [`main.rs`](src/main.rs) is a 1,073-byte unconditional unavailable exit which ignores input and exits 2;
- [`sha256.rs`](src/sha256.rs) is a 4,756-byte hand-written SHA-256 implementation not linked from `main.rs`;
- `Cargo.toml` declares four internal dependencies unused by the binary source;
- there is no request loop, parser, daemon caller or searchable document result.

These facts were rechecked on the R039 branch. Do not harden, qualify or preserve the scaffold. If #227 is
deferred, remove the package/workspace member. If #227 is implemented, replace the package atomically with the
real retained-PDF path and remove the stub, custom SHA-256 and unused dependencies.

## Current donor state

`pdfium-render` and PDFium are absent from the current ELIOT lock/runtime. They are a selected **candidate
boundary**, not an installed dependency, frozen crate version or packaged native artifact.

At implementation time #227 must:

1. inspect the then-current official crate API/releases, MSRV, advisories, features and transitive closure;
2. pin one exact crate version together with one explicitly packaged matching PDFium native artifact;
3. verify the binding/version relationship on every selected platform;
4. disable rendering/image/network or other unused features where the current API permits it;
5. record artifact source, license, hashes and packaging/runtime checks in the donor register and owner docs.

The Rust crate does not itself provide PDFium. Do not bind an arbitrary system library, auto-download a binary
at runtime or document an inspected API as though it were installed.

## Selected first capability

```text
retained PDF ObjectRef
→ one owned PDFium child when FFI isolation is selected
→ native page text + page/region coordinates + explicit gaps
→ one derived document representation
→ structure-first units
→ normal Qdrant publication/query
→ excerpt/location bound to the same retained PDF revision
```

The parent supplies exact retained bytes or a read-only input derived from that object. The worker never reopens
a mutable source path. The parent owns source/revision identity; the worker does not hash the PDF again or mint a
second identity.

Parent preparation owns extraction profile/settings, one absolute deadline/cancellation, finite input/output/
page/text limits, process/resource containment, terminate/wait, temporary cleanup, validation and persistence of
the derived representation.

The child processes one document in flight initially and returns bounded typed page text, page number/size,
available regions/coordinates and per-page `complete | partial | no-native-text | failed` status. It writes no
redb/object/Qdrant state and mints no receipt, capability or completeness claim detached from page results.

Extracted text is a mapped representation, not `ExactBytes` of the binary PDF. Preserve page boundaries,
coordinate provenance, reading-order limitations and explicit gaps. Query-time reads reuse the prepared artifact
and never rerun PDFium.

## Keep the capability small

Native text is the first profile. OCR is a separate later profile for pages without usable native text and must
state language/settings/gaps. Layout/table models, GROBID, Xberg or Docling are alternatives only for a selected
corpus. No automatic fallback chain or simultaneous installation of every donor.

Use a child only when native/FFI/JVM/Python/OCR isolation is selected. Safe bounded Rust parsers may run directly
in preparation.

## Removal and completion

Remove the refusal main, unlinked SHA-256, four unused internal dependencies, generic qualification/echo
fixtures and any second source identity/database/publication path in the same slice.

Completion is a retained PDF searchable through the real application with honest page/region evidence and gaps,
including malformed/oversized/cancelled input, child failure/cleanup, restart/reuse and correct source revision.
A constructor, checksum, refusal message or candidate donor description is not document support.

[Implementation map](FUNCTIONS.md) · [Document portfolio #216](https://github.com/UnknownAlienHuman/eliot-search/issues/216) · [PDF #227](https://github.com/UnknownAlienHuman/eliot-search/issues/227)
