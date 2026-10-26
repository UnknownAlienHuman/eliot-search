# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions, published changes and
unrun work; it is not architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R051, 2026-10-26 — exact Qdrant 1.20 query/schema API and remaining query/preparation/control owners reconciled.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `a4c23110c94f77c67e8f82057e5a84e11dbf1ad6` (R050).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), the [implementation sequence](../implementation/README.md#delivery-sequence),
then only the affected owner recipe, actual current callers and owning issue.

R051 changes documentation only. The product remains unimplemented. Next implementation is Foundation plus the
Windows handle-finalization canary, then the exact Qdrant 1.20 dependency/schema/query cut and #264.

## R051 inspected evidence

### Official Qdrant Rust client 1.20

Verified signed tag `v1.20.0`:

```text
tag object  3e7920854c271883ae1bca60097c73dbb7082124
commit      3b5a75aa371169486a9af8fc2748a76b379f0d02
release     2026-10-22
```

Inspected generated proto/builders/conversions and official snippet tests:

- `collections.proto`, `points.proto`;
- `CreateCollectionBuilder`, `TextIndexParamsBuilder`, `CreateFieldIndexCollectionBuilder`;
- `TextQueryBuilder`, `QueryPointsBuilder`, `SearchParamsBuilder`, `UpsertPointsBuilder`;
- `qdrant_client/conversions/query.rs`;
- BM25 text-index and TextQuery snippet tests.

Exact usable surface is present:

- vectorless collection (`vectors_config` optional; `PointStruct.vectors` optional);
- `TokenizerType::Code`;
- `TextIndexParamsBuilder::bm25_scoring`;
- `TextQueryBuilder::{k,b}` and `Query::new_text`;
- `QueryPointsBuilder::{using,filter,params,limit}`;
- `SearchParamsBuilder::idf`;
- `UpdateMode::InsertOnly`.

### ELIOT active owners checked

- lexical profile;
- query family README/AGENTS, access/scope, planner and retrieval executor;
- unitizer/materializer;
- redb control/evidence owner;
- Qdrant bridge/publication/runtime owners;
- implementation/test/index entrypoints;
- issues #272/#278/#279/#291/#331 and programme handoff.

## Findings and decisions

### F1 — R050 still left the old sparse BM25 profile active

`search-lexical` still selected `Document(text, "qdrant/bm25")`, fixed `avg_len`, word tokenizer, sparse document/
query inference and local alias expansion. Query docs still named Qdrant 1.19 and the old lexical owner.

**Decision:** baseline is one vectorless Qdrant 1.20 collection with `retrieval_text` payload, scoring full-text
index, `Code` tokenizer and `TextQuery`. Remove sparse BM25 inference, local `avg_len`, local postings/numerical
encoders and alias-expansion profile from the connected baseline.

### F2 — code tokenizer makes local alias expansion harmful by default

The official 1.20 code tokenizer splits camelCase/snake_case/acronym transitions and indexes the joined identifier.
Appending local parts duplicates terms and changes term frequency/document length.

**Decision:** unitization emits text/context exactly once. No automatic snake/camel/acronym aliases. A measured
alternative text composition is a new retrieval profile/fresh generation. Exact punctuation/location remains
retained-source Exact.

### F3 — free text is any-term nomination, not a hidden query language

`TextQuery` ranks points containing any query term; required/excluded terms belong in request filters.

**Decision:** arbitrary search text is not parsed as Boolean/phrase/negation syntax. Typed required/excluded
clauses, if exposed, become explicit filters. This prevents accidental product semantics from donor syntax.

### F4 — scope needed two explicit filter levels

Prior text alternated between “same filter for retrieval and IDF” and “query narrowing does not change IDF”.

**Decision:** use:

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Source/access selection affects both candidates and statistics. Request-only path/kind/topic narrowing does not.
Whole-collection base omits `idf`. Explicit empty returns without RPC; unknown/incomplete never widens.

### F5 — readiness needed an actual scorer/query canary

Schema/point/count readback can succeed while the chosen text field/scorer/query wiring is unusable.

**Decision:** before publication commit, verify exact schema/points/count **and execute one deterministic TextQuery
canary** against fixed generation inputs. Stage 2/3A creates text/filter indexes before ingestion and uses explicit
`insert_only` + `wait`.

### F6 — unitizer/control still made Stage 3B mandatory

Unitizer still required old/new occurrence matching; control state still centered route/profile/sequence and
ordinary incremental deltas.

**Decision:** Stage 2/3A has no occurrence matching, stable cross-generation identity or sequence. Units produce
bounded retrieval text; complete generation capture derives deterministic generation-bound point IDs. Control
pins physical collection + profile + redb snapshot. Source-prefix merge, births, sequence and closures appear only
if failed budgets activate Stage 3B.

### F7 — Qdrant statistics after point mutation strengthen fresh-generation-first

Qdrant documents that text statistics may temporarily contain stale updated/deleted data until optimization.

**Decision:** immutable Stage 2/3A collections perform no point updates/deletes after visibility. If Stage 3B is
activated, its qualification includes scoring/optimizer convergence, not only mutation readback.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | parsed root/plain settings; handle-safe initialize/open; one guard; factual status | #333/#266/#360 |
| 2 Walking skeleton | one file → ObjectRef → prepared retrieval text → vectorless Qdrant 1.20 text index → TextQuery → retained CLI excerpt | #264 and vertical owners |
| 3A correctness/reference | restart; authoritative inventory; complete fixed generation; old-view lifetime; cleanup; budget matrix | #270/#271/#331/#261/#300/#303 |
| 3B conditional optimization | source-local diff, stable occurrence IDs, point additions/closures only where 3A budgets fail | #259/#329/#261/#263 |
| 4 Exact/session | literal Exact and one framed live QueryWindow session | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root collection/archive/import and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy / lock rules

- Issues are owners, not dependency order.
- One manager owns each vertical cut; parallel writes are disjoint.
- No backend abstraction, alias manager, custom-shard fusion, watcher ledger, init journal, query scheduler/ticket
  system, lexical qualification receipts or object-operation archive.
- No redb writer, view mutex or window mutation borrow spans object/Qdrant/process/model I/O.
- Complete backend build + deterministic query canary + durable evidence commit precede view installation.
- Last view drop only wakes cleanup.
- Current-caller search is local to code moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad evidence follows connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Qdrant Rust 1.20 API/source inspection | CHECKED for named tag/types/tests |
| Vectorless scoring-text profile | SELECTED, not compiled/executed |
| TextQuery any-term/filter semantics | CHECKED upstream; not executed in ELIOT |
| Base population vs request-only narrowing | SELECTED, not implemented |
| No local alias expansion | SELECTED, not relevance-tested |
| TextQuery readiness canary | REQUIRED, unrun |
| Stage 2/3A no occurrence matching/sequence | DOCUMENTED, not implemented |
| Windows handle finalizer | SELECTED CANARY, unimplemented |
| Qdrant dependency/server upgrade | NOT DONE; repository lock remains 1.19 |
| Compiler evidence | R031 materializer + unitizer only |
| Foundation/walking skeleton/current binaries | UNRUN |
| Windows/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No donor source inspection proves ELIOT correctness.

## Immutable prior records

- R001–R031: audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: implementation-readiness correction `1eeb4191`.
- R047: active-surface hostile verification `858d1e34`.
- R048: donor/analogue synthesis and fresh-generation split `fed10c1e`.
- R049: Qdrant 1.20/platform-finalization correction `9edd2c1d`.
- R050: active Qdrant-owner synchronization `a4c23110`.
- R051: exact query/schema API plus lexical/query/unitizer/control reconciliation.

## Next action

Implement Foundation and the native Windows finalization canary, run minimal compiler/Clippy, then upgrade the exact
Qdrant Rust client/server/schema/query profile to 1.20 and implement #264. Do not begin sparse BM25 inference, local
alias expansion, occurrence matching, sequence/closure, query scheduler/tickets, watcher or optional profiles first.
