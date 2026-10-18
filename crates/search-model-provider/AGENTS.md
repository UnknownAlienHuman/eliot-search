# Working on optional models

Start with the [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md) and [local README](README.md).
This directory has no implementation; do not turn its former function wishlist into a new framework.

Select a real model/runtime only for an implemented embedding or reranking use case. Change the model
owner, preparation/query callers and Qdrant input/schema together. Use a library directly when practical;
a separate worker earns its place through actual dependencies or resource isolation.

Keep finite input bytes/tokens, batch size, queue/concurrency, output dimensions and deadline/cancellation.
Apply limits before allocation/inference. Validate actual outputs; a profile label or receipt does not
make a vector well formed or a snippet source-backed. Respect the selected scope and client restrictions.

Coalesce identical model inputs and reuse their outputs across source occurrences. Include actual
context, model revision, document/query/rerank task and input/output options in that reuse decision.
Rebind current source/revision/range independently; do not key all reuse by revision-bearing `UnitId`
or reuse an old handle, scope decision or citation as model output.

Normal search remains usable without a model. A failed optional leg has a visible gap; rerank failure
can preserve the previous order. Do not add LLM query rewriting, generated research verdicts, an
extraction graph or a second query orchestrator. Remove unused port/scaffold requirements with their
consumers instead of manufacturing receipts or another provider registry.
