# Search documentation

Keep the implementation context small. These links follow the branch being viewed; refactor work is in
[PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).

## Product and implementation

| Document | Purpose |
|---|---|
| [Architecture](architecture/README.md) | Product purpose, observable requirements, boundaries and principles. |
| [Implementation](implementation/README.md) | Selected mechanisms, owners, removals and donor seams. |
| [Product behavior](product/AGENT_ANALYSIS_FRAMEWORK.md) | Useful navigation, ambiguity, comparison, document and exact-verification outcomes. |
| [Query operations and current seams](../crates/search-query/README.md) | Search/Exact/Continue/Expand behavior, scope/evidence rules and actual producer/consumer gaps. |
| [Ownership graph](handoff/DEPENDENCY_GRAPH.md) | Effect/data ownership and cutover relationships, not a package-count target. |

Nearest FUNCTIONS owns the concrete recipe and source seams. Issues name change scope and callers and link
to that recipe rather than repeating another algorithm.

For the current cutover, the [daemon caller/effect map](../bins/eliot-searchd/FUNCTIONS.md#inspected-callers-and-effects),
[config caller/settings map](../crates/search-config/FUNCTIONS.md),
[test dispositions](implementation/TEST_CUTOVER.md#mixed-migration-tests-retained-for-behavior-extraction) and
[donor versions and limits](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#installed-selected-and-optional)
give concrete implementation entrypoints. They distinguish working pieces from target behavior.

The remaining [query/publication/rebuild boundaries](../bins/eliot-searchd/FUNCTIONS.md#query-publication-and-rebuild-boundaries)
and [CLI selection/removal map](../bins/eliot-search/FUNCTIONS.md#current-cli-selection-and-orphan-removal) are
now named. For Markdown adoption, use the [maintained parser correction](../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction);
the upstream fix is released but the project's dependency has not yet changed.

For settings work, use the [typed layer/path recipe](../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation)
and [configuration index](config/README.md). Unused wave/section catalogues and their duplicate guides are
removed; their historical field lists do not define a supported file schema.

## Resume research

[Audit navigation](audit/README.md) leads to the single [analysis registry](audit/ANALYSIS_REGISTRY.md):
[resume point](audit/ANALYSIS_REGISTRY.md#resume-after-interruption),
[remaining static questions](audit/ANALYSIS_REGISTRY.md#remaining-static-audit),
[delivery work](audit/ANALYSIS_REGISTRY.md#delivery-remaining-separate-from-static-audit),
[topic/checkpoint index](audit/ANALYSIS_REGISTRY.md#evidence-index-by-pass-and-topic), and
[execution evidence](audit/ANALYSIS_REGISTRY.md#execution-evidence).

The former static-closure map is historical. Current review is not exhaustive or closed, and implementation is
not complete. The registry distinguishes unread/unresolved scope from established defects awaiting code.
Audit and donor records explain evidence; they do not authorize work or create qualification gates.

## Supporting material

[Configuration](config/), [Qdrant boundary](runtime/QDRANT_ADAPTER_BOUNDARY.md),
[Windows/runtime notes](runtime/QDRANT_NATIVE_WINDOWS.md), and [ADRs](adr/) provide selected supporting detail.
Check their concrete source/version and current owner; a title alone gives no authority or execution evidence.

## History

Retired Architecture 8.4/P00/wave masters, packets, receipts, generated maps, standalone DIRECT/DPAPI runbooks,
swarm assignments and structural qualification remain in Git history. Do not regenerate them or preserve dead
APIs to satisfy their instructions. Recheck useful historical behavior and place its rule in the actual owner.
