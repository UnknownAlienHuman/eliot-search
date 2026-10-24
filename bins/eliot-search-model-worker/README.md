# eliot-search-model-worker — remove the refusal stub

The current workspace binary is not a model capability. At inspected R032 head `3a5d3032`, `src/main.rs` is
**1,047 bytes**, has no dependencies or request loop, ignores argv/environment, prints an unavailable message and
exits with code 2. There is no model runtime, artifact loader, encoder, reranker or daemon caller.

The current source message still cites retired P15/G6 qualification authority. That string is legacy code to
remove with the stub, not a current activation rule. Keeping an always-unavailable executable in the workspace
creates a false product surface.

Remove this package and its workspace member under the consolidation work. The docs-only
[`search-model-provider`](../../crates/search-model-provider/README.md) remains a design note, not a crate.
Baseline Search uses Qdrant native BM25 and does not wait for embeddings or reranking.

## When model code may return

Only after all of these exist:

1. the retained-source → Qdrant → pinned-excerpt path works;
2. a declared corpus/query set demonstrates one specific model materially helps;
3. the exact artifact, tokenizer, task role, options, packaging and finite resource policy are selected;
4. direct preparation/query/publication consumers change in the same slice.

Prefer a bounded in-process library when operationally acceptable. Create a child only when a real native
dependency, crash boundary or resource-isolation need justifies it. Do not recreate a generic provider registry,
plugin framework, qualification receipt chain, lifecycle operating system or universal worker protocol.

A selected worker has one narrow contract:

```text
bounded encode or rerank request
→ one loaded selected model/profile
→ finite validated vectors or ordered/subset candidate IDs
→ explicit optional failure
```

It cannot discover sources, widen scope, write Qdrant independently, create citations/evidence, rewrite queries,
generate answers, build graphs or schedule research. Model failure leaves the lexical path available when the
request permits that fallback and reports the gap explicitly.

Read [FUNCTIONS.md](FUNCTIONS.md), the [model design note](../../crates/search-model-provider/README.md) and the
[current implementation sequence](../../docs/implementation/README.md#delivery-sequence). Until a concrete
consumer is selected, deletion—not hardening the refusal path—is the correct implementation.
