# search-model-provider

Optional embeddings and reranking for the ordinary preparation/query path.

## Current state

This directory contains documentation only: no Cargo package, runtime or provider implementation.
[`ModelProviderPort`](../search-ports/src/optional.rs) declares `profile`, `encode` and `rerank`,
but has no implementation. The [worker binary](../../bins/eliot-search-model-worker/src/main.rs)
always reports unavailable and exits with code 2. Planner capability flags do not connect a model.

The implementation target uses Qdrant's local BM25 input for ordinary lexical search, without a model
worker. Scope orientation, source links, syntax outlines and imported semantic occurrences also need no LLM.

## Selected model path

When a concrete model is useful, integrate its library directly or use the existing worker if dependency
or resource isolation requires a process. No runtime library or model is selected by this document.
One small model owner prepares bounded inputs and validates vectors or a reranked candidate subset;
preparation, Qdrant publication and query remain with their existing owners.

Encode changed prepared content in bounded background batches. Encode a query once for the selected
retrieval profile; use document/query task modes consistently, including asymmetric model instructions.
A selected reranker receives the already bounded query candidates and their source-backed snippets.
It may reorder or drop them, and returns no new evidence or generated answer.

Reuse expensive outputs by exact model input and model/task/options. A source edit changes current
`UnitId` even for an unchanged chunk; a cached vector can be reused while its source/revision/range
occurrence is rebuilt. Cache data never supplies source authority or bypasses the current scope.

See the [implementation guide](../../docs/implementation/README.md) and
[concrete change map](FUNCTIONS.md). There is no mandatory graph, query-rewriting model, answer
orchestrator, qualification-receipt chain or separate migration platform to build first.
