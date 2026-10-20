# Document extraction — source audit and #227 implementation map

## Current package

| Current source | Actual state |
|---|---|
| `src/main.rs` | 1,073-byte unconditional unavailable exit. No argument parsing, request framing, parser or daemon integration. |
| `src/sha256.rs` | 4,756-byte hand-written SHA-256 implementation. It is not declared as a module and therefore is not part of the binary. |
| `Cargo.toml` | Declares `search-contracts`, `search-ports`, `search-provider-protocol` and `search-materializer`; none is used by `main.rs`. |
| package status | Workspace member and apparent executable despite no capability. |

The package must not remain as an unavailable placeholder. #227 either replaces it with the concrete parser
or removes it until implementation begins.

Delete the custom SHA-256. Source/revision identity comes from the retained `ObjectRef`; derived artifact
identity uses the shared selected content/artifact mechanism if needed. A document worker never creates an
independent source identity.

## One selected PDF path

```text
parent preparation:
  exact retained PDF ObjectRef
  + finite extraction profile/settings
  + absolute deadline/cancellation

owned child when selected:
  explicitly packaged PDFium
  → load exact retained bytes/read-only object input
  → iterate pages
  → native text and available character/region coordinates
  → bounded typed page results + explicit gaps

parent:
  validate result/object/profile binding
  → persist one derived representation through the common object writer
  → one MaterializationProduct / structure-first UnitSet
  → ordinary planner/publication/query
```

The worker is not a long-lived provider service. Start with one document in flight. Reuse one child across a
bounded batch only if startup cost is measured and cleanup remains simple. No document parser runs during an
ordinary query.

## Input and output

Input contains only what the concrete parser needs:

```text
request ID
retained object input/handle or bounded bytes
source revision/ObjectRef identity supplied by parent
selected PDF extraction profile/version
finite page/text/output/resource limits
```

Output:

```text
page number and page size
extracted page text
text offsets within the derived representation
available character/region coordinates
page status: complete | partial | no-native-text | failed
bounded warnings/gaps
```

Do not return source authority, Qdrant records, receipts, grants or completeness booleans detached from page
results. The parent validates page counts, bounds, UTF-8, coordinate ranges and profile/object binding.

Extracted text is not an exact byte range of the binary PDF. Use a mapped page/region representation. Preserve
page boundaries and honest reading-order limitations; do not fabricate binary byte offsets.

## Selected donor

For #227, use the inspected `pdfium-render` API with one matching packaged PDFium library:

```text
Pdfium::bind_to_library
load retained PDF bytes/input
Document::pages
page.text()
PdfPageText::all / character/region access as needed
```

The Rust crate does not ship PDFium. Bind the explicit packaged artifact; do not fall back silently to an
arbitrary system library. Disable image/rendering features when not used. PDFium serializes native access;
more Rust threads do not create useful parser parallelism.

Alternative donors remain alternatives, not parallel requirements:

- Xberg when one selected Rust integration removes more code across actual required formats;
- imported Docling artifacts or a selected layout/table profile when native page text is insufficient;
- GROBID only for a scholarly corpus requiring TEI/references/regions;
- OCR only for pages with no usable native text, with explicit language/settings and gaps.

No automatic fallback chain. OCRmyPDF sidecar alone is not complete text because pages with existing text or
skipped processing may be absent.

## Boundary and limits

Use the smallest private typed stdio frame owned by #227 or the current local frame helper after #280. Do not
depend on generic `search-ports`, the old provider pairing/grant protocol or a universal worker supervisor.

Parent owns:

- process creation and exact executable/library path;
- one absolute deadline and cancellation;
- input/output byte and page/text ceilings;
- OS memory/CPU/process containment when selected;
- kill/wait and temporary-file cleanup on every outcome;
- rejecting late, malformed or mismatched responses.

A page-size check cannot bound all native allocations. The child boundary plus practical process limits is
the reason to use a worker for PDFium. Safe bounded Rust parsers for other selected formats may run directly.

## Preparation cutover

Current raw UnitSet ingress requires byte-identical UTF-8. Do not label PDF-derived text `ExactBytes`.
Change materializer/unitizer consumers together:

```text
DerivedDocumentRepresentation {
  source ObjectRef/revision
  extraction profile/version
  ordered page text
  page/region coordinate map
  explicit page gaps
}
```

Persist/reload this artifact through the common object store. Unitize pages/structural blocks and return
citations to the retained PDF plus derived page/region location. Reuse the artifact on later queries and
unchanged revisions; no parser rerun at query time.

## Remove with implementation

- refusal-only `main.rs` and obsolete unavailable/qualification message;
- unlinked `src/sha256.rs`;
- unused `search-contracts`, `search-ports`, `search-provider-protocol`, `search-materializer` dependency edges;
- generic provider/runtime qualification documents and fixtures;
- echo parser/protocol demonstrations which do not produce searchable results;
- any second document database, source identity or publication path.

The real package may add only the exact dependencies selected by #227. If implementation is deferred, remove
the workspace member rather than retaining a dormant executable.

## Completion

Exercise through the real application:

- text PDF;
- multi-column/ambiguous reading order;
- page without native text;
- malformed PDF;
- oversized/long document and cancellation;
- child crash/timeout and cleanup;
- restart/reuse of the prepared artifact;
- Search returning the correct retained PDF revision, page/region and excerpt.

Completion is a searchable retained PDF with honest gaps. A worker constructor, echo response, checksum or
qualification report is not document support.
