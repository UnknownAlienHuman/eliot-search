# search-query-planner

**C22 — Server-owned query planner.**

**Status:** `SOURCE_PRESENT`; planning, dependency capture, leg-graph and fingerprint source exists, while canonical scope/recipe/ranking integration and installed product qualification remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #125/#213/#221.

Compile a normalized recipe, coherent view, validated grant and budgets into an immutable vendor-neutral `SearchTaskPlan`.

## Owns

- recipe normalization
- load-bearing dependency capture
- bounded leg graph
- `PlanFingerprint`
- deterministic ordering and replan triggers
- priority and budget assignment

## Must not own

- accepting raw client Qdrant plans
- database clients
- mixing source/workspace view revisions
- unbounded legs or queues
- embedding client admission authority
- hard dependency on subject-resolution or optional-provider implementations

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 9,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
