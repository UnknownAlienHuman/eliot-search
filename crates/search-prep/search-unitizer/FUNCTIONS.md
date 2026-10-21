# Unitization: source ranges and conservative reuse

## Existing source seams

`layout.rs` provides line-preferred UTF-8 partitioning. `manifest/input.rs` consumes a materializer product;
`manifest/build.rs` partitions it and derives revision/coordinate/ordinal-bound IDs. The codec constructs a
canonical tree, hashes, serializes and copies it; verification rebuilds the expected full manifest.
Markdown/JATS builders call `verify_markdown`/`verify_jats`, reopening and parsing the source again, then embed
a full raw-v3 manifest inside v4/v5. The daemon still calls older DIRECT layout/profile paths.

Reuse the actual parsers/range mapping and one owned materialization. Remove adjacent-stage reprocessing,
duplicate encodings, receipt/profile compatibility and obsolete callers together. Validate independently
loaded artifacts at their boundary, not by rerunning the producer inside the same request.

## Boundaries

Use actual Markdown/JATS sections/items, a selected parser's real code structures, or line-preferred plain
text when no syntax producer exists. Split oversized units locally and merge small peers only under the
same context. Model-specific packing is a projection, not source identity. Keep original bytes/maps distinct
from bounded heading/path/signature retrieval text.

## Exact reuse rule

The old recommendation 'structural anchor plus duplicate position' is insufficient: inserting the first of
several equal blocks shifts every duplicate position. Do not infer continuity from an ordinal, matching name
or equal hash, and do not build an LCS/AST-edit/lineage engine merely to preserve IDs.

For one changed source:

1. Load old/new bounded unit descriptors. If the source and relevant profiles are unchanged, reuse the entire
   existing product, including repeated units, without matching it again.
2. Use a source-stable parent scope. A parser-provided named parent is usable only when it is unique in both
   versions; otherwise use a known enclosing scope or treat the affected region as ambiguous.
3. Form a match key from that parent scope, actual unit kind and exact retrieval input identity, including
   any heading/path/context that was supplied. Count occurrences in old and new descriptors.
4. Reuse only a one-old/one-new key, after checking the input equality required by its real content format.
   A digest collision or incompatible profile is not a match. Unrelated absolute range movement is allowed.
5. All unmatched or multiply occurring keys get new incarnations for this operation. Retire the unmatched
   old points through publication. Do not greedily pair equal blocks by ordinal or claim that a moved block
   was tracked when the data cannot disambiguate it.

Within one bounded source this can use an ordinary sorted descriptor array or map; no cross-repository
identity service. The [planner](../../search-index-qdrant/search-projection-planner/FUNCTIONS.md) assigns and
persists birth values for unmatched occurrences. Query evidence always uses the exact new or old binding
from its pinned view, never a guessed migrated range.

This deliberately sacrifices some reuse for edited repetitive files. It does not promise all edits are
cheap. Report created/retained/closed points for repetitive, generated and oversized files; do not conceal
rewrite amplification behind inference-cache hits. A more advanced matcher needs measured benefit first.

## Artifact and memory

One source body/materialization and one bounded descriptor set are live at a time. Reuse the buffer for
unchanged UTF-8. Encode the one prepared artifact once at persistence. Source metadata, coordinates and
reusable input are separate fields, not three serialized manifests or a generic cache database.

The [control map](../../search-control-redb/FUNCTIONS.md) specifies compact published evidence rows and
snapshot lookup; unitization does not invent another manifest index. Publication input carries actual
text/options/vectors, not only their hashes.

## Donors and limits

Docling's hierarchical-then-token split and Unstructured's section boundaries are design donors, not runtime
dependencies. Content-defined chunking is not selected by this baseline: it may improve large unstructured
input, but does not resolve duplicate occurrence identity or prove semantic boundaries. The existing
[donor register](../../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) retains the inspected sources.

Check real consumers after implementation: one materialization, exact ranges after transforms, unchanged
source reuse, unique-unit shift, duplicate insertion/removal/reorder, changed input context and replay of the
same pending operation. No frozen v3/v4/v5 byte fixtures or new global identity framework are prerequisites.
