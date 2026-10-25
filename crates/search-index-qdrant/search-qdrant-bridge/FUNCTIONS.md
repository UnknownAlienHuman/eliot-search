# Qdrant bridge — one private SDK adapter

The bridge translates Eliot-owned operations to the workspace-pinned `qdrant-client` and validates responses.
Publication owns ordering, pending state and visibility; query owns scope and result use. Vendor types,
credentials and status text stay private here.

## One runtime

One daemon-owned `qdrant_client::Qdrant` serves publication and query. Inputs are endpoint, optional actual API
key and ordinary transport settings. Use one absolute deadline/cancellation per attempt; nested calls do not
renew it. Dropping a local future is not server cancellation.

Remove supervisor binding, qualification gates, secret leases, relative timeout reset and the process-local
mutation ledger with their callers. Live probes are diagnostics, not constructor credentials.

## Operations required by the baseline

Implement only:

- create and verify one physical generation;
- explicit `insert_only` create of immutable points;
- one non-creating fixed `valid_until` closure;
- exact point readback;
- native BM25 query;
- exact collection delete with terminal/wait semantics;
- exact collection existence/absence readback.

Add count/scroll/hybrid/vector/payload helpers only for a real consumer.

Official Qdrant documentation makes default upsert overwrite-or-insert and exposes `insert_only`/`update_only`
from server 1.17. The selected 1.19 baseline therefore must send an explicit mode; do not rely on defaults.

## Immutable point creation and retry

A point ID binds the complete immutable retrieval identity:

```text
fixed PointId + fixed Document/vector input + fixed immutable payload
→ explicit insert_only
→ exact readback
```

After timeout or lost response, publication may repeat the same fixed insert. If another attempt inserted it,
exact readback accepts identical state; conflicting state is an error. This is safe because individual point IDs
are never physically deleted inside the generation. Full upsert is not a retry primitive.

## Fixed closure

The only permitted point mutation is one exclusive `valid_until = target_sequence` value. Use a selected API
which cannot insert a missing point: explicit `update_only` for a full-point operation, or a documented existing-
point payload operation plus exact readback.

A missing point is an inconsistency; closure never creates it. Repeating the same value is allowed. Reopening or
changing the boundary is forbidden. Older views remain valid because their sequence is below the exclusive bound.

## Native lexical input

Use Qdrant native `Document` input for the selected BM25 profile. Pending pages retain the bounded text/options
needed for dispatch. Generated sparse coefficients are Qdrant output, not locally predictable proof.

Readback validates exact point ID, immutable source/occurrence/profile/filter/group bindings, `valid_from`, fixed
optional `valid_until`, required vector field, finite sparse structure and response limits. Retained source bytes,
not payload/vector digests, establish citation evidence.

## Scope and IDF

For every sparse query leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

The IDF corpus does not filter hits by itself. Explicit empty scope returns empty without RPC. Unknown IDs,
missing generation/schema and incomplete inventory are errors. Preserve server order; top-k is navigation, not
exhaustive absence.

## Whole-generation cleanup

The bridge deletes only a publication-selected retired collection after the publication owner proves no current,
pending or live-view consumer can target/use it.

```text
delete exact collection with wait/terminal semantics
→ exact exists/get observation says absent
```

Never reuse the collection name. Do not infer completion from list omission alone. If the request outcome or
absence observation is uncertain, return unresolved and keep the durable cleanup row. Logical collection absence
is not evidence that every backend file was physically reclaimed.

Point operations do not create collections by design, but the delayed-point/delete race remains a required live
case against the pinned server. The bridge has no cleanup journal; publication owns the finite unfinished row and
backpressure.

## Errors

Keep structured classes for authentication, missing/foreign generation, already-exists/identity conflict,
resource limit, transport, definite pre-dispatch failure, unresolved attempt and malformed readback. Do not
flatten them into receipt status or infer readiness from a fingerprint.

## Focused checks

Connected native ingestion/query; explicit update modes; identical insert retry; foreign conflict; fixed closure
retry and missing-point refusal; native Document readback; explicit/empty/unknown scope; filtered IDF; response
bounds; collection delete/absence including a delayed point attempt; and one shared runtime.
