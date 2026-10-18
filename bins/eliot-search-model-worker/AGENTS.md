# Working on the optional model process

Read the [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md) and [local README](README.md).
The current binary only exits unavailable. Implement a worker when a selected runtime needs this
process boundary; do not build its former lifecycle/function catalogue in advance.

Keep `main` a thin loader and request loop. Change the real daemon caller and model input/output owner
together. Share the same selected model settings and framing; an owned local child needs no separate
pairing, attestation or receipt system.

Bound input bytes/tokens, queue, batch, concurrency and output before inference. Propagate one deadline
and cancellation. If the runtime cannot interrupt work, let the daemon terminate the child after a
finite wait and reject its late response. Load the model once for accepted batches, not per file/query.

Return only vectors or an order/subset of supplied candidates. Source selection, retained bytes,
Qdrant writes, source validation, context windows and expensive-output reuse belong to their existing
application owners. The worker must not independently read the corpus, re-extract a graph, rewrite the
question, generate an answer or schedule further research.

Model inputs stay out of normal logs. Missing artifacts, invalid outputs and crashes produce concrete
optional failure; they do not disable lexical search or start another model silently. Report actual
connected behavior, not an unavailable stub's contract tests as model support.
