# Working on search-query-planner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [query cutover](../README.md) and local
[source audit](FUNCTIONS.md).

Do not repair, privatize or version the current 104 KiB proof protocol. `CapabilitySet`, `SearchTaskPlan`,
grants, safe-leg proofs, dependency graphs, plan IDs, fingerprints and drift observations have no connected
production caller and are not current compatibility contracts.

Implement the real handler first:

```text
pinned PublishedView
+ ResolvedScope
+ typed request
+ RequestContext
+ constructed service references
→ direct operation(s)
```

For baseline `Search`, call the real Qdrant BM25 handler directly. Do not manufacture a one-leg plan.
`Exact`, `Continue` and `Expand` dispatch to their direct owners. Optional operations remain unsupported
until a real handler exists.

Keep only nontrivial pure selection that survives integration:

- group compatible sources into one route/profile population;
- split incompatible physical routes/profiles;
- assign finite branch ceilings within the existing absolute deadline/counters;
- preserve deterministic internal operation order;
- fail unsupported modes before work.

Change request/contracts, daemon, executor and provider consumers in the same cutover. Delete old planner
APIs and fixtures rather than adding facades or a V3 family. If the remaining code is a few helpers, move
it into query composition and delete the crate.

After code, run minimal Rust 1.98 compiler/Clippy for changed production consumers. Do not spend the
implementation phase maintaining plan hashes, source-shape tests or recipe matrices for deleted APIs.
