# Optional model implementation map

## What exists

| Source | Actual API or behavior |
|---|---|
| [This directory](README.md) | Documentation only; the old profile/qualification/encode function catalogue has no implementation. |
| [`search-ports::optional`](../search-ports/src/optional.rs) | `ModelProviderPort::profile`, `encode`, `rerank`, with associated input/output types and `OperationContext`. No implementing provider or daemon caller. |
| [Planner](../search-query/search-query-planner/src/lib.rs) | Capability booleans can create `Semantic` and `Rerank` leg drafts. They perform no inference. |
| [Indexed executor](../search-query/search-retrieval-executor/src/indexed.rs), [real bridge](../search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) | Current `query_filtered` accepts sparse `(u32, f32)` pairs; it is not a ready dense-vector path. |
| [Worker](../../bins/eliot-search-model-worker/src/main.rs) | `main` emits the disabled message and exits 2; no model loading or request loop. |

## What to implement when a model is selected

1. Keep one concrete model configuration: actual model/tokenizer revision, task-specific input
   instructions and truncation, vector dimensions/normalization, and finite resource limits. Choose
   only options the selected runtime consumes; use its API directly. The existing trait may be
   simplified or removed with callers if it contributes no useful boundary.
2. In [preparation](../search-prep/search-materializer/src/product/pipeline.rs), form bounded batches
   from already retained/prepared text. Coalesce identical inputs before inference. Store reusable
   outputs with the existing prepared artifacts; do not reread source paths or persist a job ledger
   for an ordinary encode operation.
3. Key reusable output by exact input text plus any heading/context actually supplied, model revision,
   task and relevant tokenizer/template/truncation/dimension/normalization options. Document and query
   modes can differ. Current unit identity includes its source revision: rebuild the occurrence and
   coordinates after an edit while reusing unchanged expensive output. Changed context invalidates
   reuse even when the central chunk text is unchanged. Keep reuse bounded; it is not another authority
   database, cross-client answer cache or long-lived copy of every query.
4. Change [projection inputs](../search-index-qdrant/search-projection-planner/src/model.rs), the real
   bridge and their callers together for the selected named vector. Publish vectors only under their
   actual compatible profile. Rebuild affected derived vectors when that meaning or shape changes;
   reuse unaffected preparation and the ordinary publication lifecycle.
5. In [query composition](../../bins/eliot-searchd/src/query_composition.rs), encode each distinct
   selected query input once, reuse it across compatible legs and retrieve through the same Qdrant
   owner. Do not cast dense values into the sparse API or add an alternative search store.
6. Rerank only a bounded candidate subset using the snippets already read by the query's source owner.
   Require unique input identities, finite scores and output contained in that input; preserve source
   identity and the resulting order through projection. A failed optional rerank can return the previous
   order with a gap. It cannot emit an answer, follow-up research task or new source.

Validate one output per accepted encode input (or explicit per-item failures), correct dimensions and
finite values. Use the request's remaining deadline and cancellation; limit model input tokens as well
as bytes before inference. Shape validation does not establish retrieval quality or source correctness.

Exercise the connected profile with representative document/query pairs, an unchanged-chunk edit,
changed input context, invalid outputs, cancellation and model unavailability. Compare useful retrieval
against lexical-only results and account for actual model calls and reused outputs. No benefit receipt,
canonical request hash, mandatory worker or graph build is an implementation prerequisite.
