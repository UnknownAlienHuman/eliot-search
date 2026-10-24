# Model worker disposition

## Current source

At inspected R032 head `3a5d3032`:

| Path | Actual state |
|---|---|
| `src/main.rs` | 1,047-byte dependency-free unconditional unavailable exit; no parsing, framing, model, artifact or daemon integration. |
| `Cargo.toml` | No dependencies; the package is still a workspace member. |
| source message | Refuses activation using retired P15/G6 qualification wording. That message is obsolete legacy behavior, not a product gate. |
| callers | No daemon or other production caller found in the established current caller map. |

Earlier package docs disagreed on the file size (989, 1,047 and 1,073 bytes). R033 records the current Git tree
size and removes the contradictory claims. This is documentation evidence only; the stub code is unchanged.

The previous speculative `search-model-provider` Rust implementation was deleted under #528. Do not leave a
permanent refusal binary as its replacement.

## Immediate change

Delete:

```text
bins/eliot-search-model-worker/
workspace member bins/eliot-search-model-worker
lock/package/status references whose only subject is the stub
```

No baseline user behavior changes. Qdrant native BM25 remains the selected retrieval path. A binary name,
unavailable exit or optional flag does not advertise a model capability.

## Future selected integration

Only a concrete measured candidate may introduce code. Implement the smallest owner with its real consumers:

```text
prepared retrieval input
+ exact model artifact/tokenizer/task/options
→ bounded document vectors
→ ordinary projection/publication

query text
+ same published query role/options
→ one query vector
→ ordinary scoped Qdrant query

bounded source-backed candidate snippets
+ selected reranker profile
→ reordered or explicit subset candidate IDs
```

Rules:

- choose one model/profile, not a provider/plugin catalogue;
- model/task/options are part of retrieval-profile identity;
- cache document work by exact model input/context/role/model/options;
- query input is bounded and normally ephemeral; do not build a raw-query archive;
- validate vector count, dimensions, finite values and candidate identity before use;
- reranking cannot create evidence, widen scope or add a candidate not supplied;
- no automatic fallback to another model/runtime and no ordinary runtime model download;
- optional failure preserves lexical availability only when the request selected that fallback, with a gap.

Use an in-process library first. A process is justified only by real dependency/crash/resource isolation. If a
worker is selected, use one narrow owned child with finite frames/batches/concurrency/memory, one absolute
deadline/cancellation and parent-owned terminate/wait. Reuse the current local framing mechanism; do not build
another pairing/grant/capability protocol.

## Verification when reintroduced

Measure the exact selected model against the lexical baseline on a declared corpus. Exercise document/query role
compatibility, dimensions, invalid/non-finite output, cancellation, artifact mismatch, restart/cache reuse and
lexical availability after model failure. A constructor, refusal stub or provider trait does not establish a
model capability.
