# eliot-search-model-worker — delete until a model is selected

The current binary is a dependency-free refusal stub: it ignores every argument and environment variable,
prints `MODEL_WORKER_PROFILE_DISABLED`, and exits with code 2. It loads no model, serves no request and has no
daemon caller. Keeping an always-unavailable executable in the workspace creates a false product surface.

Remove this package and its workspace member now. The docs-only
[`search-model-provider`](../../crates/search-model-provider/README.md) remains a decision note, not a crate.
Baseline Search uses Qdrant native BM25 and does not wait for embeddings or reranking.

A model implementation may return only after all of these exist:

1. the lexical source → Qdrant → retained-excerpt path works;
2. a measured corpus/query set shows a specific model materially helps;
3. the exact model, tokenizer, task roles, options and artifact packaging are selected;
4. direct preparation/query consumers are changed in the same slice.

Prefer an in-process library when it is bounded and operationally acceptable. Create a child worker only
when native dependencies, crash isolation or resource control justify the process boundary. Do not recreate a
generic model-provider trait, provider registry, qualification receipt, lifecycle OS or universal worker
protocol in advance.

When a worker is genuinely selected, its narrow contract is:

```text
bounded encode/rerank request
→ one loaded selected model
→ finite vectors or reordered/subset candidate IDs
→ typed optional failure
```

It never owns source discovery, scope, Qdrant writes, citations, context windows, query rewriting, generated
answers, graphs or research orchestration. Model failure leaves the lexical path available and explicit.
