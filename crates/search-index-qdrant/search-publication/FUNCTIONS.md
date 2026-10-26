# Publication — complete generations first, measured incrementality later

One serialized publisher connects retained prepared state to physical Qdrant generations, durable evidence/current
metadata and one matching `PublishedView`. It owns no source reader, parser, query window or second recovery
platform.

## Desired state

Publication compares one consistent selected source/preparation snapshot with the durable published inventory.
Queues are acceleration only; startup/explicit/periodic reconciliation rediscovers differences.

Outcomes remain distinct:

- usable preparation, including zero units: selected complete outcome;
- terminal unsupported/failure: selected gap-only outcome;
- confirmed removal/exclusion after complete inventory proof: omitted from desired selected inventory;
- running/incomplete observation: prior served view remains; pending/gap is reported;
- exact desired/published match: no publication.

Incomplete inventory never retires unseen data.

## Fixed pending generation input

Before Qdrant effects, persist one bounded `PendingGenerationBuild` containing:

```text
operation/generation identity
expected durable current/source/preparation guards
never-reused physical collection name
retrieval profile and explicit scoring full-text schema
complete desired evidence/inventory pages
actual retrieval text payloads and fixed point IDs
page checksums/counts/limits
```

Pages live in immutable objects when too large for compact redb rows. Retry uses those fixed pages; it never rereads
live paths, regenerates units or splices newer work into the same generation.

A failed or uncertain generation is never served. Its name is never reused. Recovery first resolves or abandons
that exact pending build before creating another.

## Stage 2 / 3A complete-generation operation

```text
read fixed pending pages
→ create exact physical Qdrant 1.20 collection/schema
→ create scoring full-text index with explicit text processing
→ explicit insert_only complete build
→ exact schema/point/count readback
→ bounded redb transaction:
     compare expected durable current + pending build
     install complete evidence/inventory rows
     set durable current physical generation/profile
     add predecessor RetiredCleanup when needed
     remove pending build
  commit
→ open and verify the matching redb read snapshot
→ install PublishedView under a short manager lock
→ report visible
```

No redb writer or view-manager lock spans object/Qdrant I/O. Do not begin another publication until view
installation succeeds or recovery refuses serving.

`PublishedView` carries the exact physical collection. Qdrant aliases are not consulted. Once visible, a Stage 3A
collection is immutable. A real desired-state difference builds a new complete generation.

## Stage 3A release gate

Stage 3A is the mandatory correctness/reference implementation. It is the shipped mode only for declared corpus
classes where #215 records numeric budgets and raw measurements for:

- full build p50/p95 and edit-observed → view-installed latency;
- points/payload bytes and backend writes;
- peak memory/CPU/handles;
- current + building + reader-retained generation disk;
- build throughput versus coalesced update arrival rate;
- cleanup backlog and longest supported reader/window lifetime.

The publisher coalesces to the latest desired state while one build runs. It must remain bounded and eventually
publish the newest complete state. If declared latency/disk/throughput budgets fail, Stage 3B becomes required for
that corpus class before release. Do not choose arbitrary thresholds inside the publisher.

## Stage 3B optional incremental publication

Only after measured need:

- persist one fixed complete-source delta;
- reuse only unambiguous occurrences with unchanged retrieval input/context;
- explicitly `insert_only` new immutable points;
- apply one fixed non-creating exclusive `valid_until` closure to replaced points;
- exact readback precedes one atomic complete-source evidence/inventory/current commit;
- individual point IDs are never physically deleted;
- profile/schema change or measured closed-point growth creates a fresh generation.

Default upsert is forbidden. Missing-point closure fails rather than creating. Transition into the incremental
profile is an explicit generation rebuild, never reinterpretation of an existing collection.

Custom shard keys are not a general Stage 3B substitute because full-text/BM25 statistics are shard-local and a
portfolio query across shards would compare different score populations or require another fusion owner.

## Commit/install gap and restart

Durable current commit precedes process-local view installation. If the process crashes in the gap, startup:

1. reads durable current generation/profile;
2. verifies exact physical collection/schema and complete evidence snapshot;
3. opens the matching redb read snapshot;
4. installs `PublishedView` before accepting query traffic;
5. refuses serving if any binding is missing/conflicting.

No alias, receipt or publication journal can substitute for these real observations.

<a id="view-lifetime-and-whole-generation-cleanup"></a>
## View lifetime and whole-generation cleanup

One strong current `Arc<PublishedView>` plus weak retired states is sufficient. Query clones the guard under a
short manager lock and releases the lock before backend/object work. A live `QueryWindow` may retain the same
guard.

A `RetiredCleanup` row names one exact never-reused physical collection. Cleanup may start only when:

- it is not durable/current;
- no pending/current/future build can target it;
- no live view/window guard uses it;
- no owned unresolved attempt remains targetable;
- no redb writer or view-manager lock is held across backend I/O.

Last guard drop only wakes cleanup. The bridge terminally deletes the exact collection and verifies exact logical
absence. Never reuse the name. Uncertainty retains the row and bounded publication backpressure. Logical absence
is not physical-erasure proof. Local immutable-object collection remains stopped-root maintenance.

## Removal

Delete old floor/journal/proof machines, supplied readback, daemon publication ledger, isolated-route succession,
possible-writer platform, per-point reclaimer, raw pin registry and exclusive fixtures after real callers move.
Keep only fixed pending input, compact durable current/evidence/cleanup rows and the small view manager.

## Verification

Run minimal Rust 1.98 check/Clippy for publication/bridge/control/query/daemon consumers. Focused live cases:
complete first build, zero/gap generation, lost insert response, foreign point/schema, commit/install crash,
restart install, dropped wake-up, edit/removal full rebuild, incomplete inventory preserving prior view, old guard,
cleanup/name non-reuse, delayed request/delete race, uncertainty/backpressure and Stage 3A measurement matrix.
Stage 3B cases run only if selected.
