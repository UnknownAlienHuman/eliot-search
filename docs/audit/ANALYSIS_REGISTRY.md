# Analysis registry

Continuation memory and evidence status. It is not architecture, permission or a checklist which must complete
before coding.

<a id="resume"></a>
## Resume

**Checkpoint:** R047, 2026-10-25 — implementation-readiness verification and active-surface synchronization.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected head:** `1eeb41912893d323a677ae5b94c8d45f0a636453` (R046).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), then the
[implementation sequence](../implementation/README.md#delivery-sequence), then only the affected owner recipe and
actual current callers.

R047 verifies every active instruction surface which can direct the next implementation: root entry documents,
implementation/test/dependency spine, config/object/publication/Qdrant owners, donor register and load-bearing
open issues. It does not claim product execution.

## R047 findings and decisions

### F1 — root entrypoints still advertised the old horizontal order

`README.md` still said to finish broad #333 before source/preparation. `QUICKSTART.md` linked a removed registry
anchor and mixed the first product smoke with later restart convergence.

**Decision:** Foundation contains root/plain settings only. It is followed immediately by #264's one-file
walking skeleton. Full restart/lost-wakeup convergence is the next stage, not a prerequisite for the first CLI
result.

### F2 — #598 still contradicted the plain-storage baseline

The #598 body still called selected-key storage a prerequisite for #270/#330/#352, while R046 only corrected a
comment and other documents. #266/#267 and several later issues could still direct an agent to wait for the
private handoff.

**Decision:** #598 is an optional encrypted-object profile only. Plain `ObjectRef` put/open/read under #330 is the
baseline and is implemented independently. Private #598 branches/worktrees remain protected; any accepted
profile extends the same object boundary and cannot create a second source/store/query path.

### F3 — open issues could restore superseded recovery and lifetime rules

#215 still required unsettled-writer/isolated-generation recovery. #263 referred to unresolved-writer rules.
#300 incorrectly made fixed point closure wait for old readers. #266/#267/#360 retained old storage-profile
language. #189/#214 used stale registry anchors/checkpoints. #315 could defer current object identity to #598.

**Decision:** synchronize those issue bodies with the current spine. A fixed exclusive `valid_until` closure is
safe for older sequences; only whole-generation deletion waits for all guards. Issue bodies track work but never
override canonical owners.

### F4 — donor evidence was stale and one active anchor was broken

The donor register still named R041 and linked the removed config `#sparse-layers-and-validation` heading.

**Decision:** update to R047, point at `#layer-semantics`, and state the exact Qdrant update-mode evidence.

### F5 — Qdrant retry semantics were selected but not fully bounded

Official Qdrant documentation states that default upsert inserts or overwrites, while `insert_only` and
`update_only` are available from server 1.17. The selected 1.19 baseline therefore supports explicit modes.
However, documentation had overclaimed that delayed point mutations could not affect collection cleanup without
stating the required delete/absence proof or runtime evidence.

**Decision:**

- new point: explicit `insert_only`, identical fixed retry and exact readback;
- closure: one non-creating fixed `valid_until`, never default upsert;
- no individual point deletion;
- no new pending operation after generation retirement;
- whole-generation cleanup waits for all guards, uses terminal collection deletion, verifies exact logical
  absence and never reuses the name;
- uncertain deletion/absence retains `RetiredCleanup` and backpressure;
- logical absence does not claim physical disk erasure;
- delayed-point/delete behavior remains a required live case on pinned Qdrant 1.19.

### F6 — lock order was implicit

Owner documents prohibited some locks across I/O but did not provide one cross-owner acquisition order. An agent
could still compose a publisher/view/window/redb cycle.

**Decision:** add the explicit lifetime/lock order to the implementation spine and dependency graph. No redb
writer, view-manager mutex or window mutation borrow spans object/Qdrant/process I/O. View install follows commit;
cleanup never runs inline from last guard drop.

### F7 — release and test instructions needed the same stage boundary

The release issue and test cutover mixed first-smoke evidence with later reconciliation, sessions and cleanup.

**Decision:** separate Foundation, one-file product smoke, convergence, Exact/session and maintenance/release
confidence. Broad suites do not block the first production path.

## Current implementation sequence

| Stage | Observable result | Primary ownership |
|---|---|---|
| Foundation | one invocation; root/plain typed settings; inspect/init/open; one guard; factual status | #333/#266/#360 |
| Walking skeleton | one explicit file → plain ObjectRef → preparation → fresh Qdrant generation → one Search → pinned CLI excerpt | #264 plus #267/#270/#330/#331/#259/#263/#261/#272/#293/#280/#332 |
| Convergence | restart/reopen; inventory/reconcile; ABA/lost-wakeup handling; incremental publication; rebuild/compaction | #270/#271/#331/#329/#261/#300/#303 |
| Exact/session | literal Exact and one framed live session with one QueryWindow | #249/#275/#280/#332 |
| Maintenance/depth | stopped-root collection/archive/import and selected optional capabilities | #302/#244/#281/#227/#335 |

## Non-blocking and anti-bureaucracy rules

- Issue closure order is not implementation dependency order.
- One manager owns each vertical cut and integration commit; parallel work is disjoint.
- Settings, traits, donors and optional adapters appear only with a real consumer.
- Plain storage and lexical Search do not wait for #598, watcher, regex, documents, MCP, models, overlay,
  managed Qdrant or persistent pipe.
- Current caller inspection is local to code being moved/deleted.
- Historical waves, packets, receipts, package maps and closed issue bodies are not authority.
- The first connected path uses final owners and values; no throwaway second route.
- Minimal Rust 1.98 check/Clippy follows each production cut; broad tests follow connected behavior.

## Active-surface synchronization

R047 updates canonical files and the following open issues before branch advancement:

- #189 — current registry anchor and vertical-cut scope;
- #214 — current R047 evidence and remaining tooling removal;
- #215 — release scenarios aligned with insert-only/no-per-point-delete design;
- #263 — exact Qdrant update modes and generation cleanup boundary;
- #264 — walking-skeleton completion separated from stage-3 convergence;
- #266/#267/#360 — plain baseline independent of #598;
- #300 — closure does not wait for old readers; generation deletion does;
- #315 — current plain object identity does not wait for encrypted handoff;
- #598 — optional encrypted profile, not baseline prerequisite;
- #352 and PR #610 — current programme/handoff.

At publication of this checkpoint these metadata surfaces are synchronized to the exact R047 commit.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Canonical architecture, implementation spine and owner handoff | CHECKED through R047 |
| Active load-bearing issue alignment | CHECKED through R047 |
| Dependency/lock order | DOCUMENTED, not executed |
| Root/config/source/preparation/publication/query design | IMPLEMENTATION-READY, not implemented |
| Plain object profile | SELECTED, not implemented |
| Encrypted #598 profile | PROTECTED/OPTIONAL |
| Qdrant `insert_only`/`update_only` API availability | OFFICIAL DONOR EVIDENCE; selected 1.19 satisfies version floor |
| Qdrant delayed-operation/collection-delete race | REQUIRED LIVE CASE, UNRUN |
| Compiler evidence | R031 materializer + unitizer only |
| Current binaries/Foundation/walking skeleton | UNRUN |
| Windows durability/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No documentation review proves the product will work. R047 removes known instruction cycles and defines the
smallest executable proof which can falsify the remaining assumptions quickly.

## Immutable prior records

- R001–R031: historical audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation and publication closeout through `af337ec7`.
- R046: first implementation-readiness correction at `1eeb4191`.
- R047: hostile verification of active documents/issues, lock order and Qdrant retry/cleanup boundary.

## Next action

Implement Foundation, run the minimal compiler/Clippy closure, then immediately implement the one-file walking
skeleton and execute it against real Qdrant 1.19. Update documentation only with concrete code cuts or newly
verified evidence; do not restart a general documentation audit.
