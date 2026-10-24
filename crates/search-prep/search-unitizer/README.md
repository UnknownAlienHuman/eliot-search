# search-unitizer

Turn a materialization into useful units with exact source coordinates. A unit is an occurrence in a
particular representation; its ID does not promise semantic stability across revisions.

Read the [implementation guide](../../../docs/implementation/README.md) for integration,
[FUNCTIONS.md](FUNCTIONS.md) for existing functions and [AGENTS.md](AGENTS.md) for changes here.

## Accepted v3 baseline

The implemented `prepare_unit_set_input` / `build_unit_manifest` path accepts nonempty,
byte-identical UTF-8 text/code with `ExactBytes`, an empty loss map and equal input/canonical digests.
It derives contiguous line-preferred ranges, occurrence IDs and an immutable manifest.
Current v3 does not support arbitrary transcoded, normalized or lossy materializations.

[layout.rs](src/layout.rs) implements the reusable range algorithm. Raw units cover each input byte
once, preserve UTF-8 boundaries and obey actual unit/count limits. Source admission and Qdrant
publication happen in their respective application owners.

## Markdown structural v4

[markdown_unitset/build.rs](src/markdown_unitset/build.rs) is implemented. It builds raw v3 coverage
plus structural occurrences and inline observations from the existing materializer product.
Heading relationships differ from physical containment; overlapping structural observations do not
increase the covered byte count. Selected normalized values retain mapped provenance.

## JATS structural v5

[jats_unitset/build.rs](src/jats_unitset/build.rs) is implemented. It builds raw coverage plus element
occurrences and the materializer's attribute/text/comment observations. Original XML ranges and mapped
values remain distinct. It introduces no XML parser.

These new UnitSet paths are not connected to production daemon ingestion. The old DIRECT daemon uses
the existing layout/profile APIs. That is a caller gap, not missing UnitSet implementation.

## Refactoring target

Consume one immutable materialization and build units once. Structural construction currently invokes
full materializer replay through `verify_markdown` / `verify_jats`, rereading and reparsing the source.
Remove that requirement with the ingress/callers. Validate externally reconstructed or persisted
artifacts at their acceptance boundary.

The v3 builder also constructs a canonical value tree, hashes it, serializes the whole manifest,
copies the encoded bytes and throws them away after checking length. Keep typed unit records and one
bounded encoding where persistence needs it. Use real content/occurrence identity where consumed;
source/fixture hashes, nested envelopes and receipts are not preservation requirements.

Change v3/v4/v5 and their actual consumers together. Legacy formats and APIs do not need a compatibility
layer. An unsupported old root may be rejected before writing and left intact.
