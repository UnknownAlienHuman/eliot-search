# Historical W3 Qdrant qualification programme

The previous W3 programme attempted to turn one exact Windows Qdrant executable, Rust client revision,
artifact digest, process/secret lease and a large probe matrix into runtime admission. Public main then made
`QualifiedGate` and the lease/equality receipts mandatory inputs to `RealDataPlane::connect`.

That design is retired. The selected 1.19.0 pair and probe outputs remain useful historical evidence, but a
report produced by an earlier test run is not authority over the service currently running.

## What the old programme established

The 2026-09-10 bridge run exercised thirteen useful behaviors against the selected pair, including:

- server identity and one-shard schema;
- required payload indexes and strict-mode rejection;
- signed epoch/open-upper-bound filtering;
- sparse IDF and independently scoped IDF population;
- `wait=true`, write ordering and exact mutation/readback;
- schema/readback response shape.

Several broader process/publication/restart claims remained unavailable. Do not convert the partial result
into either a product-ready claim or a reason to keep the constructor gate.

## Current use

Use this directory only when:

- changing the pinned Qdrant server/client pair;
- changing a concrete schema/query/mutation behavior;
- reproducing a platform-specific defect;
- recording a release or performance evaluation.

A focused run records the exact version, configuration, command, platform and raw result needed for that
claim. Independent review may be useful for a release decision, but the codebase does not need an immutable
review receipt or package-wide acceptance state.

Normal startup instead performs the operations it actually needs:

```text
configured endpoint + optional credential
→ live connection
→ current server capability/version observation
→ exact selected collection generation/schema readback
→ publication/query operations
```

Unsupported or incompatible current state fails truthfully. A previous probe report cannot override the
current service, and a missing report cannot replace an actual failed connection/schema readback.

## Useful ownership boundary

| Responsibility | Current owner |
|---|---|
| optional local Qdrant child/endpoint lifecycle | selected daemon/supervisor composition |
| SDK translation, collection/point/query/readback RPCs | `search-qdrant-bridge` |
| native BM25 text/profile and prepared publication pages | preparation/planner |
| stable point identity | point-identity owner |
| publication recovery and current view | `search-publication` + typed redb current/pending rows |
| source evidence | retained object store + `ViewManifestEntry` |
| old-view lifetime and cleanup | current-view/publication owner |

Do not restore the old reclaimer, epoch-pin, acceptance-review or process-receipt platforms merely because
they appear in historical probe ownership.

## Current baseline to verify

For the connected local product, focus on:

1. create/verify one selected physical generation;
2. ingest real text using native `qdrant/bm25` `Document` input;
3. query with selected-scope `idf.corpus` and preserve Qdrant order;
4. exact full/narrow mutation readback with `wait=true`;
5. unknown write recovery from actual Qdrant state after restart;
6. `CurrentPublishedView` switch and old-view evidence survival;
7. exact retired-point or old-generation cleanup after final guard.

One node, one shard, one replica and one serialized publisher are the baseline. Strong ordering, strict mode
and another topology are configuration choices, not universal correctness floors. Measure before expanding.

## Retired requirements

Do not require:

- exact executable SHA/build, client source checksum and thirteen probe outcomes as a runtime token;
- a purpose/incarnation-bound secret callback and process/owner equality receipt before every RPC;
- `QUALIFIED | UNQUALIFIED` global product state or automatic indexed-mode disablement based on this archive;
- immutable reviewer receipts, package handoff acceptance or frozen source spelling;
- strongest write ordering for the local one-writer topology;
- exact counts for ordinary ranked top-k;
- source revision/range or vector digests echoed through Qdrant payload as evidence;
- separate publication/reclaim/pin qualification state machines.

Keep credentials out of repository/logs, keep vendor types private and state live claims only after actual
execution. These are direct engineering requirements; they do not justify the retired qualification
operating system.

The current adapter and cutover map are in
[`../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md`](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md)
and [`../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`](../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).
