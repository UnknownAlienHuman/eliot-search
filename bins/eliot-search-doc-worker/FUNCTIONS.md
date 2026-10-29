# Document extraction — source audit and #227 implementation map

## Current package

| Current source | Actual state |
|---|---|
| `src/main.rs` | 1,073-byte unconditional unavailable exit. No argument parsing, request framing, parser or daemon integration. |
| `src/sha256.rs` | 4,756-byte hand-written SHA-256 implementation not declared as a module. |
| `Cargo.toml` | Declares `search-contracts`, `search-ports`, `search-provider-protocol` and `search-materializer`; none is used by `main.rs`. |
| package status | Workspace member and apparent executable despite no capability. |

These exact facts were rechecked on the R039 branch. The package must not remain as an unavailable placeholder.
[#227](https://github.com/UnknownAlienHuman/eliot-search/issues/227) either replaces it with the concrete parser
or removes it until implementation begins.

Delete the custom SHA-256. Source/revision identity comes from the retained `ObjectRef`; a derived artifact uses
the shared selected object/artifact mechanism. A document worker never creates an independent source identity.

## Donor state and implementation-time selection

`pdfium-render` and PDFium are absent from the current ELIOT lock/runtime. The PDFium boundary is selected as a
candidate for the first native-text PDF profile, but no crate version, feature closure or native artifact is
installed or frozen.

At implementation time:

1. inspect the current official `pdfium-render` API/releases, MSRV, advisories, features and transitive closure;
2. pin one exact crate version with one explicitly packaged matching PDFium artifact;
3. verify the binding/version relationship on every selected platform;
4. disable rendering/image/network and other unused features where supported;
5. record artifact source, license, hashes and packaging/runtime checks in the donor register and this owner map.

Do not bind an arbitrary system library, auto-download a runtime artifact or preserve an old inspected method
name merely to satisfy documentation.

## One selected PDF path

```text
parent preparation:
  exact retained PDF ObjectRef
  + finite extraction profile/settings
  + absolute deadline/cancellation

owned child when selected:
  explicitly packaged matching PDFium artifact
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

The worker is not a long-lived provider service. Start with one document in flight. Reuse a child across a
bounded batch only after measuring startup cost and preserving simple cleanup. No parser runs during an ordinary
query.

## Input and output

Input contains only what the selected parser needs:

```text
request ID
retained object input/handle or bounded bytes
source revision/ObjectRef supplied by parent
selected extraction profile/version
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

Do not return source authority, Qdrant records, receipts, grants or one detached completeness Boolean. The parent
validates page counts, bounds, UTF-8, coordinate ranges and profile/object binding.

Extracted text is not an exact byte range of the binary PDF. Use a mapped page/region representation. Preserve
page boundaries and honest reading-order limitations; do not fabricate binary byte offsets.

## Boundary and limits

Use the smallest private typed stdio frame owned by #227 or the current local frame helper after #280. Do not
depend on generic `search-ports`, the old provider pairing/grant protocol or a universal worker supervisor.

Parent owns:

- process creation and exact executable/library path;
- one absolute deadline and cancellation;
- input/output byte and page/text ceilings;
- OS memory/CPU/process containment when selected;
- kill/wait and temporary-file cleanup on every outcome;
- rejection of late, malformed or mismatched responses.

A page-size check cannot bound all native allocations. The child boundary plus practical process limits is the
reason to isolate PDFium. Safe bounded Rust parsers for other selected formats may run directly.

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

Persist/reload the artifact through the common object store. Unitize pages/structural blocks and return citations
to the retained PDF plus derived page/region location. Reuse on later queries and unchanged revisions; no parser
rerun at query time.

## Alternatives are not simultaneous requirements

- OCR is a separate later profile for pages without usable native text, with explicit language/settings/gaps.
- Xberg/Docling/layout/table extraction is selected only for a concrete corpus that needs it.
- GROBID is selected only for scholarly TEI/reference workflows.
- No automatic fallback chain or generic document-provider registry.

## Remove with implementation

- refusal-only `main.rs` and unavailable/qualification message;
- unlinked `src/sha256.rs`;
- unused internal dependency edges;
- generic provider/runtime qualification documents and echo fixtures;
- any second document database, source identity or publication path.

If implementation is deferred, remove the workspace member rather than retain a dormant executable.

## Completion

Exercise through the real application: text PDF; multi-column/ambiguous reading order; page without native text;
malformed/oversized/cancelled input; child crash/timeout/cleanup; restart/reuse; and Search returning the correct
retained PDF revision, page/region and excerpt.

Completion is searchable retained PDF evidence with honest gaps. A candidate donor, constructor, echo response,
checksum or qualification report is not document support.
