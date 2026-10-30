# search-materializer

Turn exact retained bytes into the selected text representation, coordinates and explicit gaps.
[FUNCTIONS.md](FUNCTIONS.md) owns the algorithm; the [preparation map](../README.md) routes adjacent work.

## One preparation pass

Start with the [first connected format](FUNCTIONS.md#first-connected-format), then pass its owned product to
unitization. Filesystem publication belongs to the shared object store, not another materializer writer.

<a id="allocation-discipline"></a>
## Memory and reuse

[Read limits](FUNCTIONS.md#enforce-the-execution-limit-before-acquisition) and
[artifact acceptance](FUNCTIONS.md#prepared-artifact-acceptance) define what is checked and when.
Do not read “validate a saved artifact” as “rerun its parser on every load.”

## Current duplication

The [pinned earlier source map](https://github.com/UnknownAlienHuman/eliot-search/blob/d83e1e27d27768b9803b5fd809c1631ff4cf73d7/crates/search-prep/search-materializer/FUNCTIONS.md#inspected-source-useful-work-and-actual-defects)
records duplicate writers, missing reader budget propagation and profile/empty-input defects. Recheck actual
callers at the code cut; preserve useful decoding/maps, not the legacy pipeline or historical file-size targets.
