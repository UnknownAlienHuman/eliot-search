# P00 contract pack — historical

This pack does not define current APIs, required readsets or acceptance. Do not read every schema or restore
recipes, grants, digests and receipts merely because a legacy registry names them.

Use [cross-owner values](../../implementation/README.md#minimal-cross-owner-values),
[shared data](../../../crates/search-contracts/FUNCTIONS.md), [concrete ports](../../../crates/search-ports/FUNCTIONS.md)
and the affected [operation owner](../../implementation/README.md#owners). Add a shared value with an actual producer
and consumer, not from the old schema catalogue.

The [original pack index](https://github.com/UnknownAlienHuman/eliot-search/blob/d3330da162822e6cfe7b4a5e1d8b91cb8dcc5aad/docs/contracts/p00/README.md)
preserves the old readsets and file inventory. Remaining files and `manifest.toml` here describe legacy source
or feed old tooling; they do not override the current owners. Retire those inputs with their actual consumers.
