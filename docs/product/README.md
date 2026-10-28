# Product behavior

This directory defines user-visible outcomes, not package status or agent-control machinery. These are target
requirements; [the registry](../audit/ANALYSIS_REGISTRY.md#evidence-status) records what currently runs.

| Capability | Observable result | Owner |
|---|---|---|
| Search | Bounded ranked cards with exact source/revision/location, verified excerpt, nomination basis and gaps | [Query](../../crates/search-query/README.md) |
| Exact | Stated raw/decoded/derived predicate over one pinned inventory, with actual coverage and incomplete/invalid outcomes | [Exact](../../crates/search-query/search-exact/FUNCTIONS.md) |
| Continue/Expand | Same-session paging and bounded context around an emitted item without rerunning the corpus or widening scope | [Window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Orientation/comparison | Useful next reads and supported comparisons from real evidence, with ambiguity/unknown axes | [Agent workflows](AGENT_ANALYSIS_FRAMEWORK.md) |
| Documents | Supported representations with honest page/section/region coordinates, transformations and gaps | [Agent workflows](AGENT_ANALYSIS_FRAMEWORK.md) |
| Restart/update | Committed state survives; edits/removals converge within declared corpus budgets | [Delivery sequence](../implementation/README.md#delivery-sequence) |

Stage 3A publication rebuilds complete generations while preparation can reuse unchanged artifacts. Point-level
incrementality is required only where those generations fail predeclared release budgets. Do not promise unrelated
index writes are absent in Stage 3A or silently weaken the large-corpus requirement.

Ranked retrieval is navigation, not proof of identity, equivalence, independence, completeness or absence. Exact
retained-source operations own complete claims over what they actually evaluate. Deleting an obsolete package
does not delete its useful product requirement.

[Architecture](../architecture/README.md) defines invariants. Optional producers join the same evidence path when
selected. Historical package-status/wave/control graphs stay in Git history and are not regenerated.
