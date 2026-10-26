# Qdrant bridge — one private physical-generation SDK adapter

The bridge translates Eliot-owned operations to the workspace-pinned `qdrant-client` and validates responses.
Publication owns ordering, pending state, physical-generation lifecycle and visibility; query owns scope/result
use. Vendor types, credentials and status text stay private.

## One runtime

One daemon-owned `qdrant_client::Qdrant` serves publication and query. Inputs are endpoint, optional actual API
key and ordinary transport settings. Use one absolute deadline/cancellation per attempt; nested calls do not renew
it. Dropping a future is not server cancellation.

Remove supervisor binding, qualification gates, secret leases, relative timeout resets and process-local mutation
ledgers with their callers. Live probes are diagnostics, not constructor credentials.

## Physical collections, no alias authority

Every ELIOT generation has one installation-scoped never-reused physical collection name. `PublishedView` carries
that exact name and query addresses it directly.

Do not use a Qdrant alias as serving or recovery authority. Alias mutation cannot atomically install ELIOT's
matching redb evidence snapshot, and an old pinned view must continue to address its old physical generation.
An alias may exist as an operator convenience outside correctness, but the bridge API does not require one.

## Baseline operations

Walking skeleton and fresh-generation convergence require only:

- create one physical generation with selected schema;
- verify exact generation/schema/profile;
- explicit `insert_only` create of immutable points;
- exact point readback;
- native BM25 query against an exact physical generation;
- terminal delete of an exact retired collection;
- exact collection existence/absence readback.

Do not add count/scroll/hybrid/vector/payload mutation helpers until a real consumer exists.

Stage 3B may add one fixed non-creating `valid_until` closure. Prefer explicit `update_only` full-point update or a
payload operation whose official semantics cannot insert a missing point; test the exact selected SDK request.
Default upsert is forbidden for retry.

## Complete fresh-generation build

```text
fixed physical collection + selected schema
→ bounded pages of fixed point IDs/Document inputs/payloads
→ insert_only
→ exact readback
```

A timeout/lost response may repeat the identical fixed insert. If another attempt already inserted it, exact
readback accepts identical state. Foreign/conflicting state fails the generation.

The collection is unservable until publication commits matching evidence/current state and explicitly installs
the `PublishedView`. Once visible under the fresh-generation profile, it receives no point mutation.

## Incremental profile — optional later consumer

If stage 3B is selected from measurement:

- stable point identity is generation/profile bound;
- new point remains explicit `insert_only`;
- fixed closure repeats only the same exclusive `valid_until`;
- closure may not create a missing point or reopen/change an existing boundary;
- no individual point is physically deleted;
- compaction/profile change creates another physical generation.

The bridge owns no replay journal. Publication persists the exact pending page/artifact and observes actual state.

## Native lexical input

Use Qdrant native `Document` input for the selected BM25 profile. Pending build pages retain actual bounded text
and explicit tokenizer/language/stemmer/stopword options needed for dispatch. Generated sparse coefficients are
Qdrant output, not locally predictable proof.

Readback validates at least:

- exact point ID;
- immutable source/occurrence/profile/filter/group bindings;
- expected profile-specific validity fields, if any;
- required vector field and finite sparse response shape;
- aggregate response bounds.

Retained source/prepared bytes establish evidence, not payload/vector digests.

## Scope and IDF

For every sparse query leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

The IDF corpus does not filter hits. Explicit empty scope returns empty without RPC. Unknown IDs, missing
physical generation/schema or incomplete inventory are errors, not empty success. Preserve Qdrant server order;
top-k is navigation, not exhaustive absence.

The fresh-generation profile can often express base population through one physical collection plus selected
source filters, avoiding intra-generation validity clauses. The incremental profile adds exact sequence validity
to both retrieval and `idf.corpus`; the clauses must remain identical.

## Whole-generation cleanup

Publication requests deletion only after the generation is retired, no pending/future publication can target it,
no view/window guard uses it and no owned unresolved attempt remains.

```text
delete exact collection with selected terminal/wait semantics
→ exists/get exact collection identity
→ observe logical absence
```

Never reuse the name. Do not infer completion from list omission alone. Uncertain delete/absence remains unresolved
to publication, which retains cleanup/backpressure. Logical absence is not physical disk erasure.

The delayed point-attempt versus collection-delete race remains an executed Qdrant 1.19 requirement. Do not hide
it behind an alias or process-local receipt.

## Backend boundary

Qdrant is the only selected indexed backend. Do not add a generic backend trait or Tantivy/Zoekt fallback. Those
systems inform immutable build/visibility mechanics only. If post-3A measurements reject Qdrant, prototype a
replacement through the same retained/prepared/evidence values and replace the derived backend deliberately.

## Errors

Keep structured classes for authentication, missing/foreign generation, schema/profile mismatch,
already-exists/identity conflict, resource limit, transport, definite pre-dispatch failure, unresolved attempt,
malformed readback and uncertain collection deletion. Do not flatten them into readiness/receipt status.

## Focused checks

- exact collection/schema creation and readback;
- complete insert-only one-file build;
- identical lost-response insert retry;
- foreign/conflicting point refusal;
- native Document query and retained-evidence result;
- explicit empty/unknown/multi-source scope and scoped IDF;
- no alias dependency;
- no default upsert in retry path;
- whole-generation delete/absence, name non-reuse and uncertainty;
- delayed point-attempt/delete race on pinned server/client 1.19;
- one shared runtime for publication and query.

Run minimal Rust 1.98 check/Clippy for bridge and actual publication/query/daemon consumers before live cases.