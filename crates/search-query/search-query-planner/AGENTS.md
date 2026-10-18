# Working on search-query-planner

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and
[query cutover](../README.md#direct-operations-not-a-recipe-operating-system).

Do not preserve caller-created `CapabilitySet`, `SearchTaskPlan`, grants, dependency graphs, plan IDs or
fingerprints. Change the request, planner, executor and daemon consumers together. Connected services and
the pinned view are the actual input authority.

Keep only nontrivial pure selection:

- choose implemented branches requested by the current typed request;
- keep one resolved scope and published view;
- split only incompatible physical routes/profiles;
- assign finite work limits under one absolute deadline;
- preserve deterministic operation/fusion order.

If the baseline lexical path is one direct Qdrant call, inline it rather than manufacturing a one-leg plan.
Optional branches stay unsupported until their real handlers exist. Do not add a capability registry,
canonical descriptor, qualification credential or V3 request family.

Verify the connected query and truthful unsupported behavior. Planner fixtures and frozen plan hashes are
not product completion.
