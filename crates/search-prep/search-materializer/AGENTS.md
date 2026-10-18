# Working on search-materializer

Use the root [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md). Change this owner and its actual unitizer,
storage and daemon consumers together.

## Keep the useful work

- Decode supported retained bytes, construct source coordinates, record real transformations/loss,
  and return one immutable preparation result.
- Connect [materialize_text_or_code](src/product/pipeline.rs) to the retained-byte reader or simplify
  its existing input API. Preserve actual revision/content identity through the handoff.
- Use the existing Markdown/XML donors in [markdown/parse.rs](src/markdown/parse.rs) and
  [jats/parse.rs](src/jats/parse.rs). Do not introduce another parser in unitizer.
- Retain original byte spans and distinguish mapped values from exact original subranges.
  Parsed content and resource references remain inert.
- Keep concrete input/output/event/depth limits and the caller's cancellation/deadline. Logical work
  counters are not measured CPU or memory and need no additional accounting framework.

## Reduce copies and duplicate owners

Move an owned valid UTF-8 buffer into its string and reuse it for unchanged normalization.
Build UTF-16 line coordinates during decoding instead of retaining per-scalar temporary tables.

Pass a product directly to unitization. The current full-source `verify_markdown` / `verify_jats`
replay on every pure handoff is a cost to remove with the consumers. Check reconstructed/persisted
data at its real acceptance boundary; do not replace replay with a new proof token.

Keep one existing immutable-byte writer/reader. Consolidate the duplicate preparation/revision file
engines while this package retains preparation formats. Remove unused admission-plan/receipt helpers
when their exclusive consumers disappear. Do not add a new artifact store, provider registry or port
to preserve an obsolete boundary.

## Internal formats and verification

Old DIRECT formats, profile hashes, source/fixture-definition digests and perpetual staging retention
are not preservation requirements. New roots may reject old formats before writes; original files and
sole retained copies remain intact. Optional formats do not gate the baseline.

Check exact UTF-8, actual offset-changing transforms, parser ranges, missing/corrupt retained bytes and
cancellation in the affected path. Test the changed behavior with real consumers. There are no package
permission walls, line quotas, slice exceptions or authored-fixture status gates to maintain.
