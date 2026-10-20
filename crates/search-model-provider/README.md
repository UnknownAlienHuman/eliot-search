# search-model-provider — design note, not a package

This directory contains documentation only. There is no Cargo package, implementation or runtime owner.
That is the correct baseline state.

The remaining `eliot-search-model-worker` is only an always-unavailable workspace binary and must be deleted.
Do not recreate `ModelProviderPort`, a generic provider registry or a disabled pseudo-capability around it.
Qdrant native BM25 supplies the first working lexical path; syntax, headings, links and imported code
locations need no language model.

## When model code is justified

Add one concrete integration only after:

- the source → Qdrant → retained-excerpt product path works;
- a declared evaluation set demonstrates a material need beyond lexical/structural retrieval;
- one exact model, tokenizer, document/query/rerank role, options and packaged artifact are selected;
- preparation/query/publication consumers are updated in the same implementation slice.

Prefer a direct bounded library in the existing owner. Use a child process only for demonstrated dependency,
crash or resource isolation. Do not require both a provider abstraction and worker.

## Narrow responsibilities

A selected model owner may:

- encode bounded prepared retrieval inputs in finite batches;
- encode one bounded query with the matching published query role/options;
- rerank a finite list of already source-backed candidate snippets;
- validate output count, dimensions, finite values and candidate IDs;
- report explicit optional failure.

It may not discover sources, widen scope, write Qdrant independently, create evidence, persist raw queries,
generate answers, rewrite questions, build a graph or schedule research.

Reuse expensive document outputs by exact input/context/role/model/options. A reused vector never supplies the
current source occurrence, revision or range; publication rebuilds those bindings normally. A model/profile
change is ordinary retrieval-profile/generation rebuild, not migration compatibility.

No runtime model, donor library or version is selected by this documentation. A later issue must name the
exact candidate and measured reason before adding code.
