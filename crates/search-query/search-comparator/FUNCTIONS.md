# search-comparator functions

## Current function map

[lib.rs](src/lib.rs) contains the following pure kernels. Their callers are package unit tests.
Implementations, behavior observations, configuration relations, lineage and readback/access flags
arrive as supplied inputs; these functions do not discover or read the evidence themselves.

| Function | Actual behavior |
|---|---|
| `validate_comparison_request` | Check the supplied request, local resolution, profile/limits and live-state fields. |
| `validate_comparable_implementation` | Check supplied candidate identity, match basis, observation fields and evidence/access flags. No source readback occurs. |
| `collapse_repository_lineages` | Group supplied candidates using supplied lineage relations; it does not inspect Git ancestry or discover forks. |
| `normalize_behavior_signature` | Sort/deduplicate supplied observation components and compute their signature metadata. It does not extract behavior from source. |
| `align_evidence_roles` | Group supplied observations by requested axes and evidence roles. |
| `classify_conflict` | Compare observation digests, supplied applicability and assurance. Different digests under `Overlapping` currently become `Conflict` after flag/assurance checks, even without an incompatibility check. |
| `compare_axes` | Build a bounded matrix from supplied local evidence, lineage groups, role alignments and predicate relations. It does not produce those observations. |
| `order_recommended_reading` | Sort and bound supplied reading candidates; it does not read source or create locations. |
| `assemble_behavior_set` | Assemble the supplied comparison products into the existing contract result. |
| `revalidate_comparison` | Compare result bindings with supplied current fence/portfolio state. |

There are no separate implemented `partition_configuration_variants` or
`compute_comparison_coverage` APIs. The old logical signatures are not a list of missing functions to add.

## Connect and simplify

The [README](README.md#minimal-connected-behavior) specifies the useful path: ordinary retrieval in
selected reference repositories, exact source ranges and a compact comparison with explicit match
bases and unknowns. Reuse available syntax/SCIP/LSP locations through that same source/publication path.

Replace digest-based conflict inference with actual property/value comparison only where an existing
producer supplies comparable observations. Otherwise return the source excerpts and the unresolved
axis. Tests and documentation describe evidence to inspect; their presence proves no runtime behavior.
Known forks can share a group; missing provenance cannot become a count of independent votes.

Keep necessary ordering/grouping as small helpers. Remove unused signature, predicate-receipt and
revalidation platforms while changing their actual callers. Comparison shares the ordinary result and
window; it requires no parallel retrieval engine, general behavior analyzer or mandatory model call.
