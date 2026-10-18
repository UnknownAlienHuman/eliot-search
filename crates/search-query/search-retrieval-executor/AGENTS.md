# Working on retrieval execution

Read the [architecture](../../../docs/architecture/README.md),
[query design](../README.md), [README](README.md) and [function map](FUNCTIONS.md).

Connect the real Qdrant bridge through one request context holding the pinned view, resolved scope,
absolute deadline, cancellation and finite counters. Do not preserve public scheduler snapshots, plans,
leg tickets, terminal ledgers or per-leg timeout resets.

For baseline lexical search, call one BM25 query and preserve server order. Use Qdrant native RRF/grouping
for actual indexed branches. Add one small private integer rank merge only when a real non-Qdrant branch
exists. Never compare raw scores across populations or include leg ID in candidate identity.

Bound backend nominations before collecting/sorting them. Keep branch contributions separate from stable
point identity. Pass final order unchanged to evidence lookup/result formatting.

Remove unconditional exact count, fake completeness, floating/default fusion and process-test adapters from
the product path. If the surviving code is direct calls plus a tiny merge, inline it rather than preserve
this package for fixtures. Verify cancellation/deadline, scope/IDF population, duplicate/conflicting points,
limit enforcement and order through the connected application.
