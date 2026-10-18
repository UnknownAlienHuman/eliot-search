# Working on result formatting

Read the [architecture](../../../docs/architecture/README.md), [query design](../README.md),
[evidence resolver](../search-candidate-validator/README.md) and [function map](FUNCTIONS.md).

Accept only ordered source-verified evidence items. Preserve final rank, format bounded evidence cards,
attach an optional `WindowItemRef`, and summarize truthful gaps. Charge actual encoded output through the
provider/serving owner.

Do not read sources, sort raw scores, mint handles, manage continuations, issue receipts, create coverage
proofs or recheck local grants/security checkpoints. Remove caller metadata/envelopes and handle-binding
helpers with their consumers.

If the surviving code is trivial, inline it into query composition and remove this package. Verify through
the real request/output path, not package-shape fixtures.
