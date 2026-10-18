# Model worker function map

## Actual source

The only production function is [`main`](src/main.rs). It ignores argv/env, writes
`UNAVAILABLE MODEL_WORKER_PROFILE_DISABLED` to stderr and returns exit code 2.
[Cargo.toml](Cargo.toml) has no runtime dependencies. The
[unavailable tests](tests/unavailable.rs) check this refusal behavior; they do not exercise a model.
None of the old `validate_startup`, `serve_encode`, `serve_rerank`, session or removal APIs exists.

## Selected process implementation

The [model integration map](../../crates/search-model-provider/FUNCTIONS.md) owns model inputs,
output reuse, preparation, Qdrant and reranking. Keep only these process effects here when needed:

| Effect | Small implementation |
|---|---|
| Load | Construct the chosen runtime once from concrete configured model/tokenizer artifacts and device settings. Report actual readiness or loading error to the daemon. |
| Receive | Decode bounded typed frames from the daemon-owned channel. Check task, input count/bytes/tokens, dimensions and remaining deadline before model work. |
| Run | Invoke the selected library for a bounded encode batch or finite rerank subset. Do not introduce a generic provider registry, graph pipeline or query planner. |
| Return | Bound output; return the matching request's vectors, scores/order or failure. The application validates shape/identity once at its model-output boundary. |
| Stop | Observe cancellation, discard late output, drain only within the remaining limit and exit. The daemon owns forced termination and any bounded restart. |

Model outputs have no durable publication effect until the normal preparation/query owner accepts them.
A timeout or crash leaves that operation incomplete; it does not justify a worker replay database,
shutdown receipt or independent recovery engine. Reuse already computed vectors through the existing
prepared-artifact owner, not a second process-local corpus cache.

Exercise loading, a real daemon request, output limits, invalid results, cancellation, child exit and
continued lexical operation for the selected runtime. Do not add startup qualification, compatibility
or line-budget gates, and do not advertise this target as current executable behavior.
