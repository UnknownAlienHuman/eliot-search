# Historical P00 contract map

The [architecture manifesto](../../architecture/README.md) and
[implementation guide](../../implementation/README.md) govern the new application.

These files describe the earlier P00 schema. Read a specific record only when it helps understand a
real producer/consumer. They do not require the old recipe family, grant/plan/proof fields, canonical
wire bytes, API digests or receipt process to survive refactoring. Change real types and their consumers;
there is no contract freeze or separate permission issue.

| Existing record | Historical detail |
|---|---|
| [Types](TYPE_REGISTRY.md), [completions](TYPE_COMPLETIONS.md), [canonical types](CANONICAL_TYPES.md) | Existing names, encodings and bounds |
| [Source graph](SOURCE_GRAPH.md) | Source, revision, preparation and old domain schemas |
| [Queries/results](QUERY_AND_RESULTS.md), [recipes](RECIPES.md), [recipe results](RECIPE_RESULTS.md) | Existing request/result forms |
| [Protocol](PROTOCOL_AND_LIFECYCLE.md), [ports](PORT_OPERATIONS.md) | Existing envelopes and port surface |
| [Support records](SUPPORT_SCHEMAS.md), [reasons](REASON_CODES.md) | Existing auxiliary fields and errors |

Keep only the semantics used by the new application. Source truth, coherent publication and concrete
bounds remain useful; every old field, helper type and handoff record does not.
