# Working on implementation comparison

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [local README](README.md).
Make relevant implementations easy to compare through actual source ranges and useful observations.

Current comparison functions are pure kernels with test callers and supplied observations. Connect
ordinary retrieval, source readback and compact comparison output together. Keep small grouping and
reading-order helpers where useful; the current package/API separation need not survive integration.

Preserve each candidate's match basis and selected repository/configuration. Use known provenance for
fork grouping. A matching name, text or digest does not prove semantic equivalence, contradiction or
independent lineage. Missing provenance and requested observations remain unknown.

Show signatures, implementation excerpts and available tests/docs/references. Syntax and semantic
indexers supply only their actual facts; they do not infer every error, side effect or runtime property.
Do not classify different observation digests as conflicts without incompatible property/value evidence.

Use the existing source validator and result/window owner. Remove unconsumed behavior-signature,
predicate-receipt and result-revalidation machinery with its last consumers. Comparison is an optional
bounded expansion of useful search results, not a prerequisite LLM pass or another search application.
The [function map](FUNCTIONS.md) describes current source and the missing data producers.
