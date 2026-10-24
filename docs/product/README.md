# Product behavior

This directory owns user-visible outcomes. It does not define package status, launch authority or an agent
control plane.

## Required capabilities

| Capability | Observable result | Current owner |
|---|---|---|
| Search | Bounded ranked cards with exact source/revision/location, verified excerpt, nomination reason and truthful gaps | [Query operations](../../crates/search-query/README.md) |
| Exact | A stated raw/decoded/derived predicate over one pinned inventory, reporting complete match/no-match, incomplete or invalid with accounting | [Exact owner](../../crates/search-query/search-exact/FUNCTIONS.md) |
| Continue/Expand | Same-session paging and bounded context around an emitted item without rerunning the corpus or widening scope | [Window owner](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Orientation | Useful entry files, definitions, tests/docs and next reads from observed source facts; ambiguity and missing producers remain visible | [Agent analysis behavior](AGENT_ANALYSIS_FRAMEWORK.md) |
| Comparison | Selected implementations compared over requested axes with exact evidence, conditions and unknown/unsupported axes | [Agent analysis behavior](AGENT_ANALYSIS_FRAMEWORK.md) |
| Documents | Supported page/section/table/reference representations use the same retained-source/evidence path with honest coordinates and loss | [Agent analysis behavior](AGENT_ANALYSIS_FRAMEWORK.md) |
| Incremental/restart | Committed source and published state survive restart; edits/removals converge without stale completion or full unrelated rewrite | [Implementation](../implementation/README.md) |

Ranked retrieval is navigation, not proof of identity, correctness, equivalence, consensus, completeness or
absence. Exact retained-source operations own complete claims over the inventory and representation they
actually evaluate.

## Reading and status

- [Architecture](../architecture/README.md) defines product boundary and principles.
- [Agent-oriented analysis behavior](AGENT_ANALYSIS_FRAMEWORK.md) defines workflows and evidence semantics.
- [Implementation](../implementation/README.md) selects current mechanisms and owners.
- [Analysis registry](../audit/ANALYSIS_REGISTRY.md) records current implementation/evidence status.

Deleting a broken package does not delete a product requirement. Optional syntax, model, document, overlay and
MCP producers join the same source/query/evidence path only when selected by a real workflow.

`PACKAGE_STATUS.toml`, wave queues and execution protocols from Architecture 8.4 were removed. Git history is
their archive. Do not recreate a machine-readable package/status graph: source presence, closed tasks and
structural checks do not prove connected behavior.
