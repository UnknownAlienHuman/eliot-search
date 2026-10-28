# search-comparator — source-backed comparison or a small helper

Help a user compare relevant implementations by showing actual source, useful differences and explicit unknowns.
Follow the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [query cutover](../README.md).

## Revision-specific evidence

At the established audit revision, this package contained pure grouping/signature/axis/reading-order kernels and
its repository callers were package unit tests. Callers supplied implementations, observations, configuration
relations, lineage and access/readback booleans; the package discovered or read none of that evidence. This is a
revision-specific caller finding, not permanent current-branch truth.

Before deleting, merging or connecting the package, search current manifests, Rust imports/constructors/path
inclusion, non-Rust tooling, features, targets, tests and workflows. Name every real consumer and replace only
the behavior it actually needs. Do not harden supplied-proof machinery merely because fixtures still call it.

## Minimal connected behavior

Use ordinary retrieval in the selected source scope to nominate implementations and preserve the match basis:
explicit location, exact symbol identity, matching name/signature, syntax observation or lexical similarity.
Read nominated retained ranges through the same evidence validator.

For each requested axis, return source-backed signatures/excerpts and available test/doc/reference locations, or
an explicit unknown. Syntax/semantic locations supply only the facts they actually produce. Same names, scores,
text or observation digests do not prove equal behavior, conflict, correctness or independent lineage.

Group known forks/mirrors only from recorded provenance. Missing provenance stays unknown. Different observations
become a conflict only when a real producer supplies comparable property/value evidence that is incompatible;
digest inequality alone is insufficient.

Keep small ordering/grouping helpers where useful. Remove unconsumed behavior-signature, predicate-receipt,
assurance and result-revalidation systems with their last callers. Comparison is a bounded expansion of the
ordinary retained-evidence path, not a second search application, universal analyzer or mandatory model call.

## Completion

One real query selects source-backed implementations, displays supported axes and explicit unknowns, and does not
count copies as independent evidence. If no substantial independent algorithm remains, inline the useful helpers
into query composition and delete the package. After code, run minimal Rust 1.98 check/Clippy for changed query,
source, preparation and protocol consumers.
