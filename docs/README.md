# Search documentation

Keep the implementation context small. These links follow the branch being viewed; refactor work is in
[PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).

## Product and implementation

| Document | Purpose |
|---|---|
| [Architecture](architecture/README.md) | Product purpose, observable requirements, boundaries and principles. |
| [Implementation](implementation/README.md) | Selected mechanisms, owners, removals and donor seams. |
| [Product behavior](product/AGENT_ANALYSIS_FRAMEWORK.md) | Useful navigation, ambiguity, comparison, document and exact-verification outcomes. |
| [Ownership graph](handoff/DEPENDENCY_GRAPH.md) | Effect/data ownership and cutover relationships, not a package-count target. |

Nearest FUNCTIONS owns the concrete recipe and source seams. Issues name change scope and callers and link
to that recipe rather than repeating another algorithm.

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
