# Model worker disposition

## Current source

[`src/main.rs`](src/main.rs) is 989 bytes and always returns unavailable. `Cargo.toml` has no dependencies.
There is no model runtime, IPC loop, artifact loader, encoder, reranker or daemon consumer. The executable is
still a workspace member and therefore looks like a supported component despite having no behavior.

The previous speculative `search-model-provider` Rust implementation was deleted under #528. Do not leave a
permanent refusal binary as its replacement.

## Immediate change

Delete:

```text
bins/eliot-search-model-worker/
workspace member bins/eliot-search-model-worker
package-map/status/coverage entries whose only subject is the stub
```

No baseline behavior changes. Qdrant native BM25 remains the only selected retrieval path. Optional semantic
flags do not become ready because a binary exists.

## Future selected integration

Only a concrete measured candidate may introduce code. Implement the smallest owner with its real consumers:

```text
Prepared retrieval input
+ exact model artifact/tokenizer/task/options
→ bounded document vectors
→ ordinary projection/publication

Query text
+ same published query role/options
→ one query vector
→ ordinary scoped Qdrant query

bounded source-backed candidate snippets
+ selected reranker profile
→ reordered or explicit subset candidate IDs
```

Rules:

- choose one model/profile, not a provider/plugin catalogue;
- model/task/options are part of the retrieval-profile identity;
- cache document work by exact model input, contextual prefix, role, model and options;
- query input is bounded and normally ephemeral; do not build a raw-query archive;
- validate vector count, dimensions, finite values and candidate identity before use;
- reranking cannot create evidence, widen scope or add a candidate not supplied;
- optional failure returns lexical order/results plus an explicit gap when the request allowed fallback;
- no automatic fallback to another model or runtime;
- no model download/network access at ordinary runtime.

Use an in-process library first when acceptable. A process is justified only by real dependency/crash/resource
isolation. If selected, create one narrow owned child with finite frames/batches/concurrency/memory, one
absolute deadline/cancellation and parent-owned terminate/wait. Reuse the current local framing mechanism;
do not build another pairing/grant/capability protocol.

## Verification when reintroduced

Measure the exact selected model against the lexical baseline on a declared corpus. Exercise document/query
role compatibility, dimensions, invalid/non-finite output, cancellation, artifact mismatch, restart/cache
reuse and lexical availability after model failure. A constructor, refusal stub or provider trait does not
establish a model capability.
