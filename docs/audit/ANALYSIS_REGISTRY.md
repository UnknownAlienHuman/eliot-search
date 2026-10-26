# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions and unrun work; it is not
architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R049, 2026-10-26 — Qdrant 1.20, Edge, sharding and platform-finalization review.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `fed10c1e17e07f5bd309200dd25245fcbf85a0be` (R048).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), then the
[implementation sequence](../implementation/README.md#delivery-sequence), then only the affected owner recipe,
actual current callers and owning issue.

R049 deepens the donor review where R048 still relied on assumptions. It changes documentation and issue metadata
only. The product remains unimplemented.

## R049 inspected evidence

### Qdrant

- official Qdrant Server 1.20.0 release and Rust client 1.20.0 release;
- direct BM25 scoring on scoring full-text payload indexes;
- `TextQuery`, query-time `k`/`b`, `code` tokenizer and full-text corpus-statistics behavior;
- per-shard statistics, `idf.corpus`, custom/user-defined sharding and shard-resource guidance;
- Qdrant Edge 0.8.0 docs/API, Edge-vs-Server matrix, BM25 and current WAL/persistence fixes/issues.

### Filesystem finalization

- `tempfile 3.27.0` public docs and exact Unix/Windows `persist_noclobber` implementations;
- Git same-directory loose-object writer;
- Microsoft `FILE_RENAME_INFO`/`SetFileInformationByHandle` and `FlushFileBuffers` documentation.

### ELIOT surfaces

Implementation spine, donor register, root/object owners, publication/bridge, dependency/test handoff, programme
and load-bearing open issues.

## Findings and decisions

### F1 — Qdrant 1.20 removes unnecessary sparse-BM25 machinery

R048 still targeted server/client 1.19 and sparse-vector/document inference. Qdrant 1.20 can rank payload text
directly through a scoring full-text index and `TextQuery`; it computes term/document-length statistics from the
index. The new `code` tokenizer improves snake/camel/acronym lexical recall while keeping the full identifier.

**Decision:** the walking-skeleton code cut upgrades server/client/schema/query together to 1.20.x and uses one
explicit retrieval-text payload plus scoring full-text index. Do not generate baseline sparse BM25 vectors or own
`avg_len`. Exact punctuated identifier operations remain retained-source Exact.

The repository lock still contains `qdrant-client 1.19.0`; no dependency change or compile result is claimed by
R049.

### F2 — `tempfile::persist_noclobber` is not the selected Windows finalizer

R048 treated `tempfile` as the preferred cross-platform finalizer. Source inspection showed:

- Unix prefers no-replace rename but may fall back to hard-link then unlink;
- Windows uses path-based `SetFileAttributesW` and `MoveFileExW`.

The Windows path does not bind the operation to the already-open stage handle or retained destination-directory
handle, so it does not satisfy the selected anti-replacement boundary by itself.

**Decision:** use one owner-private handle-based Windows finalizer and execute a platform canary. `tempfile` may
remain a stage helper or Unix implementation aid, but is no longer the selected Windows publication primitive.
No generic filesystem abstraction is added.

### F3 — full-generation mode needs an explicit release gate

R048 correctly made complete fresh generations precede incremental mutation, but could be read as allowing full
rebuild forever without quantified evidence.

**Decision:** Stage 3A is the required correctness/reference profile. It is the shipped profile only for declared
corpus classes where measured edit-to-visible latency, rebuild throughput, temporary/retained disk, memory and
long-reader pressure satisfy numeric release budgets. If a target class fails, Stage 3B becomes required before
release for that class. #215 owns the declared numbers and raw measurements; implementation does not invent them.

### F4 — custom shards do not solve general portfolio rebuilds

Qdrant custom sharding can isolate a modest number of large partitions and route queries to selected shards.
However BM25/full-text statistics are shard-local. A query spanning several root/source shards would merge scores
computed from different populations; `idf.corpus` cannot reach across shards and may see partial/empty overlap.

**Decision:** no per-root/per-source custom-shard baseline and no local shard-result fusion layer. Custom sharding
is reconsidered only for a product scope always queried as one partition. Stage 3B remains the general optimization
path if full generations miss budgets.

### F5 — Qdrant Edge is promising but currently raises release risk

Qdrant Edge is embedded, BM25-capable and structurally attractive for local Search. The official 0.8.0 line is
still beta, single-shard/single-device, manually optimized/flushed and recently exposed WAL acknowledgement and
crash-replay defects addressed in newer Qdrant source work.

**Decision:** do not replace the server baseline now. After the server walking skeleton, run an isolated Edge
spike against a released version. Compare build size, startup, persistence/crash recovery, Windows behavior,
full-text/BM25 relevance, filtering, generation switch and cleanup. A successful result may replace the bridge;
there is never a permanent dual backend.

### F6 — stale R047 issue endings remained

Several open issues still ended with R047 language and one digest issue implied stable point identity was always
required.

**Decision:** synchronize #189/#205/#214/#215/#227/#259/#261/#263/#264/#266/#267/#270/#271/#300/#303/#315/
#329/#330/#333/#334/#335/#352/#360/#598 to the R049 sequence where material. Stable point identity and numeric
validity ranges remain Stage 3B only.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | parsed invocation; root/plain settings; final-manifest-last inspect/init/open; one guard; factual status | #333/#266/#360 |
| 2 Walking skeleton | one file → plain ObjectRef → preparation → Qdrant 1.20 scoring full-text generation → TextQuery → retained CLI excerpt | #264 and vertical owners |
| 3A Correctness/reference convergence | restart/reopen; authoritative inventory; complete fresh generation/view switch; measured release gate | #270/#271/#331/#261/#300/#303 |
| 3B Required optimization where budgets fail | stable occurrence reuse; insert-only additions; fixed closures; measured compaction | #329/#259/#261/#263/#205 |
| 4 Exact/session | literal Exact and one framed live session/QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root collection/archive/import and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy and lock rules

- Issue closure order is not dependency order.
- One manager owns each vertical integration cut; parallel writes are disjoint.
- No backend abstraction, alias manager, custom-shard ranking layer, watcher ledger, init journal or object-operation
  archive.
- No redb writer, view-manager mutex or QueryWindow mutation borrow spans object/Qdrant/process/model I/O.
- Complete build and durable evidence commit precede view installation.
- Last guard drop only wakes cleanup; it never deletes inline.
- Optional donors appear only with a consumer.
- Current-caller search is local to code being moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad suites follow connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Qdrant 1.20 direct BM25/code-tokenizer review | CHECKED upstream; selected, not installed |
| Qdrant custom-sharding/IDF review | CHECKED for baseline decision; not executed |
| Qdrant Edge 0.8 review | CHECKED candidate; rejected as current baseline |
| `tempfile` platform implementation review | CHECKED; Windows finalizer selection corrected |
| Windows handle-based finalizer | SELECTED boundary, API canary UNRUN |
| Fresh-generation correctness profile | SELECTED, not implemented |
| Fresh-generation release budgets | REQUIRED INPUT/MEASUREMENT, not defined or run |
| Stage 3B trigger | DEFINED by failed budgets, not implemented |
| Compiler evidence | R031 materializer + unitizer only |
| Current binaries/Foundation/walking skeleton | UNRUN |
| Windows durability/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No donor or documentation review proves ELIOT correctness. R049 reduces baseline code and makes the remaining
unknowns executable early.

## Immutable prior records

- R001–R031: historical audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: first implementation-readiness correction at `1eeb4191`.
- R047: active-surface hostile verification at `858d1e34`.
- R048: donor/analogue synthesis and fresh-generation split at `fed10c1e`.
- R049: Qdrant 1.20/Edge/sharding and platform-finalization correction.

## Next action

Implement Foundation with the owner-private Windows finalization canary and manifest-last/discard-only root; run
minimal compiler/Clippy. Then upgrade the actual bridge/client/schema to Qdrant 1.20 and implement #264 using a
scoring full-text index plus `TextQuery`. After the first result, implement Stage 3A and record numeric rebuild
budgets before deciding whether Stage 3B is required.
