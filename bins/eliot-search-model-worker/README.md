# eliot-search-model-worker

Optional process for a selected embedding or reranking runtime.

## Current state

[`main`](src/main.rs) is a dependency-free refusal stub. Every invocation emits
`UNAVAILABLE MODEL_WORKER_PROFILE_DISABLED` and exits with code 2; arguments and environment do not
activate a model. There is no model loading, IPC loop, encoding or reranking implementation.
The message still names old P15/ADR/G6 requirements; those are obsolete implementation guidance.

## Target

Use this process only when the chosen runtime needs dependency, crash or resource isolation. A library
inside an existing owner is also valid. The [model path](../../crates/search-model-provider/README.md)
does not require both a provider framework and a worker.

The daemon supplies bounded text and concrete model settings over its owned channel. The worker loads
the selected model once, processes finite encode/rerank batches and returns vectors or candidate order.
It has no source-discovery, publication, context-handle or generated-answer role. A knowledge graph and
LLM query planner are not worker prerequisites.

Keep finite frames, tokens, batches, concurrency and memory, and the request's deadline/cancellation.
The daemon owns startup, shutdown and actual capability status. Optional work must not block ordinary
lexical search; unavailable code remains unavailable until a real caller works.

[Function map](FUNCTIONS.md) · [Implementation guide](../../docs/implementation/README.md)
