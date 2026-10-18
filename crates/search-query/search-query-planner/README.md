# search-query-planner — simplification map

The current package builds proof-shaped immutable plans from caller-created capability, grant, scope,
`SearchTaskPlan`, dependency and fingerprint records. Public main contains planner fixtures but no
production caller of `compile_canonical_plan`.

The product needs a much smaller operation selector:

```text
resolved scope
+ pinned PublishedView
+ query / requested mode
+ real constructed service references
+ finite limits
→ ordered private operations for this request
```

The first lexical path is one Qdrant BM25 operation and may not justify a generic plan object at all.
Multiple branches earn a private plan only when exact/overlay/dense operations are simultaneously real.

Keep useful pure logic for operation ordering, bounded per-branch limits and incompatible-route/profile
splitting. Remove public `CapabilitySet`, `SearchTaskPlan`, canonical dependency/fingerprint inputs,
recipe-support matrices and persistent plan identity with their consumers. A changed scope/view/profile is
represented by a new request input, not a stale-plan proof ceremony.

Availability comes from concrete handlers/services supplied by daemon composition. The local CLI does not
need a capability registry. A restricted-client advertisement can be derived from the same actual handler
table and remains informational.

See [FUNCTIONS.md](FUNCTIONS.md), [agent notes](AGENTS.md) and the
[query cutover](../README.md#direct-operations-not-a-recipe-operating-system).
