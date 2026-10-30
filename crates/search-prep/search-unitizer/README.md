# search-unitizer

Turn one prepared representation into bounded source-backed ranges. [FUNCTIONS.md](FUNCTIONS.md) owns the rules;
the [preparation map](../README.md) routes the input, object writer and publication consumer.

<a id="accepted-v3-baseline"></a>
## First connected path

Use the existing line-preferred range mechanism in [layout.rs](src/layout.rs) for unchanged UTF-8. No structural
parser, historical UnitSet envelope or cross-revision identity is required to return the first useful excerpt.
The recipe distinguishes reusable representation ranges from publication-owned backend point IDs.

<a id="markdown-structural-v4"></a>
<a id="jats-structural-v5"></a>
## Other formats

Consume actual materializer structures/maps only when that format's end-to-end path is selected. Do not add a
second parser or infer raw coverage from overlapping structures. Optional formats do not block plain text.

## Refactoring target

The [pinned earlier inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/d83e1e27d27768b9803b5fd809c1631ff4cf73d7/crates/search-prep/search-unitizer/README.md)
records v3/v4/v5, parser replay and discarded encodings. It is inspection history, not an instruction to migrate
all formats at once. Replace the actual caller's route, then remove displaced representations and exclusive fixtures.
Keep original data intact; do not add another manifest/proof layer to maintain old internal APIs.
