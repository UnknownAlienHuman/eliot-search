# Code map — search-unitizer

The [implementation guide](../../../docs/implementation/README.md) defines the target.
This map describes actual APIs and the work to simplify, not an additional future framework.

## Raw layout and UnitSet

| Location / symbols | Current behavior |
|---|---|
| [layout.rs](src/layout.rs): `unitize_text`, `unitize_text_checked` | Reusable line-preferred UTF-8 range partition; the checked helper is private to the crate. |
| [manifest/v3_profile.rs](src/manifest/v3_profile.rs): `validate_v3_unitizer_profile` | Validates the implemented exact text/code decisions and limits. |
| [manifest/input.rs](src/manifest/input.rs): `prepare_unit_set_input` | Borrows a materializer product, calls `verify_materialization`, checks the source/profile and ExactBytes condition, converts scalar line endpoints to bytes. |
| [manifest/build.rs](src/manifest/build.rs): `build_unit_manifest` / private `assemble_manifest` | Partitions the representation, derives occurrences, checks coverage/IDs and builds `VerifiedUnitSet`. |
| [manifest/codec.rs](src/manifest/codec.rs): `canonicalize_unit_manifest` | Constructs a `CanonicalValue` tree, rehashes the body, encodes it and copies encoded bytes into a wrapper. |
| [manifest/codec/decode.rs](src/manifest/codec/decode.rs): `decode_unit_manifest` | Parses/checks current v3 bytes and returns a proposed manifest. |
| [manifest/verify.rs](src/manifest/verify.rs): `verify_unit_manifest` | Rebuilds the expected whole manifest and compares it. `manifest_digest` simply returns the stored digest. |
| [manifest/verify.rs](src/manifest/verify.rs): `diff_unit_manifests` | Returns created/retained/retired IDs; retention requires equal descriptors. |

`UnitSetInput`, `UnitManifest`, `UnitDescriptor` and `VerifiedUnitSet` keep fields private.
Current `V3SourceBinding` supplies source/revision/materialization/representation identities; the
unitizer does not admit sources or turn materializer digests into source UUIDs.

The v3 input is nonempty, byte-identical UTF-8 with no loss or overlap. The output keeps unit ranges,
anchors and occurrence metadata rather than source bodies. Exact raw coverage and useful structural
observations remain separate facts.

## Markdown and JATS

| Location / symbols | Current behavior |
|---|---|
| [markdown_unitset/build.rs](src/markdown_unitset/build.rs): `MarkdownUnitizationSource`, `unitize_markdown` | Accepts a product plus revision-read port, invokes `verify_markdown`, builds raw v3 and creates structural/inline observations. |
| [markdown_unitset/codec.rs](src/markdown_unitset/codec.rs), [decode.rs](src/markdown_unitset/decode.rs) | Implements `canonicalize_markdown_unit_manifest` / `decode_markdown_unit_manifest` for the current v4 envelope. |
| [markdown_unitset/build.rs](src/markdown_unitset/build.rs): `verify_markdown_unit_manifest` | Reconstructs the expected set and compares the whole supplied manifest. |
| [jats_unitset/build.rs](src/jats_unitset/build.rs): `JatsUnitizationSource`, `unitize_jats` | Invokes `verify_jats`, builds raw v3 and adds the existing complete subset observations. |
| [jats_unitset/codec.rs](src/jats_unitset/codec.rs), [decode.rs](src/jats_unitset/decode.rs) | Implements `canonicalize_jats_unit_manifest` / `decode_jats_unit_manifest` for the current v5 envelope. |
| [jats_unitset/build.rs](src/jats_unitset/build.rs): `verify_jats_unit_manifest` | Reconstructs and compares the expected set. |

The current source inputs carry a `RevisionReadPort` because owning materializer verification reruns
the entire raw read/decoding/parser path. These are implemented replay semantics, not a requirement
to retain that cost. Use a single immutable materializer result in the ordinary producer→unitizer
call. Retain validation of actual persisted/external data at its acceptance boundary.

The Markdown and JATS codecs embed a complete encoded raw v3 manifest inside the structural envelope.
Keep one useful prepared artifact and its unit/structural records. A raw coverage summary does not
need another independently serialized manifest and identity tree.

## Context and reuse

Use existing headings, section parents, ranges and inline observations for a source guide and bounded
neighbor selection. Markdown's [model](src/markdown_unitset/model.rs) marks references potentially
incomplete; an observed destination is a navigation candidate, not a complete cross-source graph.
Connect the actual prepared-record reader to query composition rather than adding another notebook store.

In `derive_occurrence` in [manifest/build.rs](src/manifest/build.rs), the content digest describes the
chunk, but the occurrence identity also includes provenance, revision-bearing anchor, ordinal and
coordinates. Unchanged text in an edited file is not guaranteed to keep `UnitId`. Preserve the exact
source binding while reusing expensive outputs independently by actual prepared input and model/task/options.
Heading/path context supplied to an encoder belongs in that input; full-document context depends on the
document too. Whitespace-normalized text or a stable positional ID cannot establish identical model input.

Keep original text/ranges separate from any retrieval prefix or generated summary. Reuse the current
artifact owner for selected derived outputs; do not duplicate raw text in every unit or build a generic
cache database. Rebinding occurrences may require new index points even when no inference repeats.

## Rewrite amplification and target boundaries

The current raw layout starts at the previous byte end and chooses the next preferred line boundary.
`derive_occurrence` then includes the complete revision content digest, absolute byte/line coordinates
and ordinal in each identity. S11 point identity includes that `UnitId`. A small insertion near the start
of a file can therefore move every later boundary and turn an otherwise local edit into a large
create/retire suffix. A content-output cache can avoid some inference but cannot repair this publication
amplification.

Make the source UnitSet model-independent and structure-first:

- Markdown/JATS sections and items, code definitions/blocks and document-native elements are the primary
  boundaries. Split only an oversized structural unit; merge adjacent small peers only when they share
  the same heading/context and remain within the selected bound.
- Tokenizer/model-specific packing is a derived projection. Changing an embedding model, token limit or
  retrieval prefix rebuilds that projection, not source occurrence identity or every source map.
- Keep three facts distinct: current source revision/range, a logical occurrence within its structural
  parent, and the exact prepared/model input that may be reused. A content hash alone cannot distinguish
  two equal passages in one source; include a stable structural anchor and local duplicate position.
- Preserve the old and new revision/range binding in versioned manifests. Reusing a retrieval artifact
  never turns old coordinates into current source evidence.

For unstructured oversized text, content-defined chunking can be evaluated as a local fallback inside
a section, then snapped to UTF-8 and useful line/sentence boundaries. It is not the default semantic
chunker or a global source identity: FastCDC was designed for deduplication, scans the complete input and
can still split repetitive or meaningful text poorly. Measure retained units, model calls and Qdrant
points rewritten after representative insertions before selecting it.

Useful donors are Docling Core's
[`HybridChunker`](https://github.com/docling-project/docling-core/blob/f5f01f97e9e1c7a50c1f378e114ca063085985ce/docling_core/transforms/chunker/hybrid_chunker.py),
which refines hierarchical chunks by token-aware splitting and merges adjacent peers with matching
headings; Unstructured's
[`by_title`](https://github.com/Unstructured-IO/unstructured/blob/1b02b4d189c260ed9b1aca29583203e233d853cb/unstructured/chunking/title.py),
which preserves section boundaries and optionally page boundaries; and the
[FastCDC paper](https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia) as the
content-defined-boundary fallback. Reuse the mechanisms, not their runtime or storage stack.

## Encoding cost to remove

`assemble_manifest` constructs/hashes `body_value(&body)`, then calls
`canonicalize_unit_manifest`. That function constructs the body tree again, rehashes it, serializes
the envelope and performs `encoded.as_slice().to_vec()`. The builder checks length and discards that
entire byte buffer. `verify_unit_manifest` repeats construction when explicitly invoked.

Use the existing typed unit records and one bounded encoder at the persistence boundary. Either
retain the encoding that will be written or count/hash while encoding, without a second whole tree
and discarded full copy. Source, fixture and configuration-definition hashes are not prerequisites
for unit identity; keep only hashes with a real content/occurrence/integrity consumer.

No new serialization framework is needed here. Follow the shared serialization choice in implementation
and change the actual codecs, projection/planner readers and persistence consumer together. The current
wire schema can change; fixing its canonical bytes is not the product goal.

## Old DIRECT consumers

[unitization.rs](src/unitization.rs), [manifest/profile.rs](src/manifest/profile.rs) and
[layout_manifest](src/layout_manifest) contain receipt-bound unitization and the older layout codec.
The daemon's [direct_preparation/layout.rs](../../../bins/eliot-searchd/src/direct_preparation/layout.rs)
currently calls the layout/profile path. The new v3/v4/v5 paths have no production daemon caller yet.

Reuse its useful exact partitioning. Remove old receipts, formats and exclusive fixtures with their
consumers rather than keeping a second implementation. Unsupported old roots may be rejected before
writes and left intact; no legacy reader/export programme must precede the new source→UnitSet path.
