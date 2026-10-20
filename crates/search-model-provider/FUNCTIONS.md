# Optional model path — implementation decision

## Current state

- this directory is documentation only and is not a workspace member;
- the old caller-mintable model-provider implementation was removed under #528;
- `search-ports::optional::ModelProviderPort` has no implementation or connected daemon caller and is part of
  the global port-registry removal;
- planner `Semantic`/`Rerank` flags do not load a model or create a Qdrant vector;
- Qdrant publication/query is currently lexical/sparse only;
- `eliot-search-model-worker` is a 989-byte dependency-free refusal stub still included in the workspace.

Delete that stub/package now. Its existence does not preserve useful design or user behavior.

## Selection gate

A new model integration needs one narrow issue containing:

```text
measured deficiency and evaluation set
exact model/artifact/tokenizer revision and license
exact document/query/rerank task modes and options
packaging and offline loading
input/output/resource/deadline limits
real preparation/query/publication consumers
failure/fallback semantics
removal criteria when benefit is absent
```

This is an engineering selection, not a qualification operating system. No immutable review receipt,
provider lifecycle or compatibility with the deleted pseudo-API is required.

## Small direct implementation

### Document encoding

Preparation supplies finite `RetrievalInput` records containing exact bounded text/context and role. The
selected owner encodes finite batches and returns vectors in input order. Validate count, dimensions, finite
values and profile identity before publication. Cache by exact input bytes/context/role/model/options.

### Query encoding

The query owner encodes one bounded query under the existing `RequestContext`. Document/query roles and
instructions must match the published profile. Query vectors are ephemeral unless a measured short-lived
cache is justified; do not persist raw query history.

### Reranking

Reranking receives only a finite set of already scoped, source-backed candidate snippets and stable candidate
IDs. It may return a permutation or explicit subset of those IDs. It cannot add candidates, evidence or new
text. Invalid/partial output is rejected or reported as an optional gap according to the request profile.

### Process choice

Run in-process when the selected library is bounded and operationally acceptable. If isolation is needed,
create one owned child for that exact runtime:

```text
load selected artifact once
→ bounded typed encode/rerank batches
→ finite validated result
```

Parent owns startup, timeout, cancellation, terminate/wait and resource limits. Reuse the current local
framing; no pairing, grant, capability digest or independent source access.

## Remove/avoid

- dormant model worker workspace member and package/status maps;
- unused `ModelProviderPort` and optional capability booleans;
- generic provider/plugin/model registry;
- model qualification/readiness receipts and lifecycle state;
- query rewriting, answer generation, graph extraction or research orchestration;
- automatic model/runtime fallback;
- network model download at ordinary runtime;
- source authority or citation fields returned by a model.

## Completion for any future model

A selected candidate is complete only when the real application improves the declared evaluation set,
retains lexical availability on failure, publishes/queries vectors under one exact profile and returns the
same retained source evidence. Model construction, a passing worker protocol or vector-shaped test output is
not completion.
