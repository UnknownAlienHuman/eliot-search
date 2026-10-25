# Working on search-query-planner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [query cutover](../README.md), package
[README](README.md) and local [source audit](FUNCTIONS.md).

The established audit found a 104 KiB proof protocol and no normal serving caller for its plan compiler at the
inspected revision. Treat that as revision-specific evidence. Before deletion or inlining, recheck exact current
manifests, Rust/non-Rust callers, constructors, module inclusion, features, targets, tests and workflows. Move
all surviving callers in the same cut.

Do not repair, privatize or version `CapabilitySet`, `SearchTaskPlan`, grants, safe-leg proofs, dependency graphs,
plan IDs, fingerprints or drift observations. Implement the real handler first:

```text
pinned PublishedView
+ ResolvedScope
+ typed request
+ RequestContext
+ constructed service references
→ direct operation(s)
```

`Search` calls the real lexical/Qdrant handler. `Exact`, `Continue` and `Expand` dispatch to their direct owners.
Optional operations remain unsupported until a real handler exists.

Keep only nontrivial pure selection that survives integration: compatible route/profile grouping, splitting of
actually incompatible routes, finite branch ceilings inside the existing absolute deadline/counters,
deterministic internal order and early unsupported-mode failure. Do not manufacture a one-leg plan.

Change request/contracts, daemon, executor and provider consumers together. Delete old planner APIs and fixtures
rather than adding facades or a V3 family. If the remaining code is a few helpers, move it into query composition
and delete the crate.

After code, run minimal Rust 1.98 check/Clippy for changed production consumers. Do not maintain plan hashes,
source-shape tests or recipe matrices for deleted APIs.
