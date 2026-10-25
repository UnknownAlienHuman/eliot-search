# Qdrant bridge — one private SDK adapter

The bridge translates Eliot-owned operations to the workspace-pinned `qdrant-client` and validates responses.
Publication owns ordering, pending state and visibility; query owns scope and result use. Vendor types,
credentials and status text stay private here.

## One runtime

One daemon-owned `qdrant_client::Qdrant` serves publication and query. Inputs are endpoint, optional actual API
key and ordinary transport settings. Use one absolute deadline/cancellation per attempt; nested calls do not
renew it. Dropping a local future is not server cancellation.

Remove supervisor binding, qualification gates, secret leases, relative timeout reset and the process-local
mutation ledger with their callers. Live probes remain diagnostics, not constructor credentials.

## Baseline operations only

Implement only:

- create/verify/delete one physical generation;
- insert-only create of immutable points;
- set one fixed `valid_until` closure;
- exact point readback;
- native BM25 query;
- exact collection existence/absence readback.

Add count/scroll/hybrid/vector/payload helpers only for a real consumer.

## Immutable point creation and retry

A point ID binds the complete immutable retrieval identity. Creation uses insert-only semantics.

```text
fixed PointId + fixed Document/vector input + fixed immutable payload
→ insert-only
→ exact readback
```

After timeout or lost response, publication may repeat the same fixed insert. If another attempt already inserted
it, exact readback accepts identical state; conflicting state is an error. This is safe because the baseline
never physically deletes individual point IDs. Do not use full upsert as retry.

The only permitted mutation of a published point is writing the same fixed exclusive
`valid_until = target_sequence`. Repeating that closure is idempotent; reopening or changing the boundary is
forbidden.

## Native lexical input

Use Qdrant native `Document` input for the selected BM25 profile. Pending pages retain the actual bounded text and
options required for dispatch. Generated sparse coefficients are Qdrant output, not locally predictable proof.

Readback validates:

- exact point ID;
- immutable source/occurrence/profile/filter/group bindings;
- `valid_from` and optional fixed `valid_until`;
- required vector field, finite sparse structure and response bounds.

Retained source bytes, not payload digests or vectors, establish citation evidence.

## Scope and IDF

For every sparse query leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

The IDF corpus does not filter hits by itself. Explicit empty scope returns empty without RPC. Unknown IDs,
missing generation/schema and incomplete inventory are errors, not empty success. Preserve Qdrant server order;
top-k is navigation, not exhaustive absence.

## Generation cleanup

The bridge deletes only exact retired collections requested by publication, with `wait=true`, then verifies
absence. Collection names are installation-scoped and never reused. There is no per-point physical deletion in
the baseline and no bridge cleanup journal.

## Errors

Keep structured classes for authentication, missing/foreign generation, already-exists/identity conflict,
resource limit, transport, definite pre-dispatch failure, unresolved attempt and malformed readback. Do not
flatten them into receipt status or infer readiness from a matching fingerprint.

## Focused checks

Connected native ingestion/query, identical insert retry, foreign point conflict, fixed closure retry, native
Document readback, explicit/empty/unknown scope, filtered IDF, response bounds, generation delete/absence and one
shared runtime.
