# Domain helpers — move behavior to its real owner

This is legacy source, not a required shared-rule service. [Local scope](../../docs/architecture/LOCAL_SCOPE.md)
selects no grants, credential state or restricted clients. Do not replace this package with a domain-v2 facade.

<a id="gross-production-inventory"></a>
<a id="operation-level-caller-census"></a>
## Source seams

The [immutable source/caller inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-domain/FUNCTIONS.md)
records the earlier census. Recheck actual imports, Cargo/features and binary dispatch at the code cut.

<a id="transition_publication"></a>
<a id="transition_source_ownership--source_ownership_v2"></a>
<a id="classify_mutation_outcome"></a>
<a id="cargo-dependencies"></a>
| Existing family | Real receiving owner |
|---|---|
| Publication transition/receipt ladder | [Publisher](../search-index-qdrant/search-publication/FUNCTIONS.md) and typed control transactions |
| Namespace ownership/cutover | [Current source rows](../search-source/search-source-registry/FUNCTIONS.md), no ownership-transfer platform |
| Generic mutation-result booleans | The actual effect owner: current-row readback, immutable-file verification, backend observation or child wait |
| Grant/currentness/eligibility/visibility algebra | Local source selection and actual constructed handlers; delete unused proof APIs |

<a id="replacement-rules-by-owner"></a>
<a id="scope-and-eligibility"></a>
<a id="currentness"></a>
<a id="coverage-and-assurance"></a>
<a id="ordering"></a>
<a id="visibilityreadiness"></a>
<a id="state-transitions"></a>
## Keep one rule owner

[Query](../search-query/README.md) routes scope, rank order, evidence, Exact completeness and windows.
[Implementation](../../docs/implementation/README.md#owners) routes effects and lifetime. Do not copy their
algorithms into a global domain package. A pure helper stays shared only when current consumers need identical behavior.

## Deletion order

Move useful behavior and its immediate callers together. Remove old public types, exports, fixtures and dependency
edges with that cut. Recheck reverse consumers; delete the crate if no independent multi-consumer algorithm remains.
No compatibility wrapper or hypothetical-future enum earns permanence.

## Focused verification

Run the [minimal production closure](../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) for the
actual changed owner/consumers. Later behavioral cases belong to that owner, not a second domain qualification suite.
