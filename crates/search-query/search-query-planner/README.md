# search-query-planner

**C22 — deterministic server-owned query planner.**

**Status:** substantive source exists for the closed eleven v1 recipes, coherent snapshot/grant checks,
bounded leg construction, dependency capture and deterministic plan identity. The supported daemon does
not yet expose a complete real-Qdrant recipe path, and general free-text/orientation is not in v1.

## Owns

- recipe/body normalization and validation;
- requested-scope intersection through accepted access outputs;
- coherent source/workspace/portfolio snapshot dependencies;
- bounded exact/direct/indexed/provider leg graph;
- deterministic leg ordering and budgets;
- `PlanFingerprint`/compiled-plan identity;
- replan/unavailable decisions on load-bearing drift.

## Must not own

- accepting raw client Qdrant plans, filters, collection names or point IDs;
- database clients or vendor types;
- mixing source/workspace view revisions;
- unbounded legs, queues or per-file filter explosions;
- client admission/task authority;
- an implicit online-research or model fallback.

## Current gaps

- real daemon composition remains incomplete;
- the aligned S9.5/S10.3 eligibility/Qdrant stack is not integrated on `main`;
- free-text retrieval and scope orientation require #213;
- the accepted baseline lexical/structural ranking profile requires #221;
- corpus/portfolio management requires #218.

This package is `SOURCE`, not a publicly `ENABLED` recipe service.

- **Product area:** Architecture S19–S21
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Function contract:** [FUNCTIONS.md](FUNCTIONS.md)
- **Current status matrix:** [../../../docs/product/IMPLEMENTATION_STATUS.md](../../../docs/product/IMPLEMENTATION_STATUS.md)
