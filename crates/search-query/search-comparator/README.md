# search-comparator

Help a user compare relevant implementations by showing actual source, useful differences and what
remains unknown. Follow the [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md).

## Current source

[lib.rs](src/lib.rs) contains pure kernels: `collapse_repository_lineages`,
`normalize_behavior_signature`, `align_evidence_roles`, `compare_axes` and
`order_recommended_reading`. Their callers here are unit tests. Callers supply implementations,
behavior observations, configuration relations, access/readback booleans and lineage receipts.
The package does not discover repositories, extract behavior from code, read tests or resolve callers.

`normalize_behavior_signature` sorts supplied observation digests; it does not derive a semantic
signature. `classify_conflict` labels different observation digests under overlapping predicates as
`Conflict` once assurance checks pass. Different observations can both be true. This rule cannot
establish incompatible behavior without an actual comparison of the observed property and value.

## Minimal connected behavior

Use ordinary retrieval within the selected reference repositories to nominate implementations. Keep
the match basis: exact symbol identity, matching name/signature, syntax pattern or lexical similarity.
Read the nominated retained ranges through the same source validator and return a compact comparison
of signatures, implementation excerpts and available test/doc/reference locations.

For each requested axis, show a source-backed observation or an explicit unknown. Syntax outlines and
[SCIP/LSP locations](../../search-prep/search-code-enricher/README.md#optional-semantic-locations)
can supply navigation and selected symbol facts. They do not automatically infer all errors, side
effects, behavioral equivalence or normative correctness. Tests/documents describe evidence to inspect;
their presence alone proves no runtime property. Configuration-specific evidence stays distinguishable.

Group known forks/mirrors using recorded repository provenance. Collapse identical occurrences by real
source identity. Neither same text nor a matching SCIP package name proves independent repository
lineage; when provenance is missing, show it as unknown instead of counting an independent vote.

Reuse small ordering/grouping helpers where useful. Remove unconsumed behavior-digest, predicate-receipt
and result-revalidation platforms with their last consumers. Comparison may be a bounded expansion of
the ordinary query result, not a second search application, a universal behavior analyzer or a required
LLM pass. A missing comparison candidate is not proof that an equivalent implementation does not exist.
