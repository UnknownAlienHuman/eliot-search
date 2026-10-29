# Working on implementation comparison

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [query cutover](../README.md), package
[README](README.md) and local [source map](FUNCTIONS.md).

The established audit found pure comparison kernels with package-test callers and supplied observations at the
inspected revision. Treat this as revision-specific evidence. Before deletion, inlining or connection, search
current manifests, Rust/non-Rust callers, constructors, module inclusion, features, targets, tests and workflows.
Move every surviving consumer in the same cut.

Connect ordinary retrieval, exact source readback and compact comparison output together. Preserve each
candidate's match basis and selected repository/configuration. Use known provenance for fork grouping. Matching
names, text, scores or digests do not prove semantic equivalence, contradiction or independent lineage. Missing
provenance and requested observations remain unknown.

Show signatures, implementation excerpts and available tests/docs/references. Syntax and semantic indexers
supply only their actual facts; they do not infer every error, side effect or runtime property. Different
observation digests are not conflicts without incompatible property/value evidence.

Use the existing evidence validator and `QueryWindow`. Remove unconsumed behavior-signature, predicate-receipt,
assurance and revalidation machinery with their last callers. If only small grouping/reading-order helpers remain,
inline them into query composition and delete the package.

After code, run minimal Rust 1.98 check/Clippy for actual query/source/preparation/protocol consumers. Package
self-tests of supplied data are not product delivery.
