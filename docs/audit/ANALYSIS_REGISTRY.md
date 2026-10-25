# Analysis registry

This file is continuation memory and evidence status. It is not architecture, permission or a checklist that
must be completed before coding.

<a id="resume"></a>
## Resume

**Checkpoint:** R046, 2026-10-25 — implementation-readiness blockers removed.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected head:** `af337ec7d8d87cf8304b96c3413a9a23702410cc` (R045).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), then
[implementation delivery sequence](../implementation/README.md#delivery-sequence), then only the affected owner
recipe and current callers.

## R046 findings

### F1 — configuration completion cycle

#333 and the previous delivery sequence required effective settings to reach root, source, Qdrant and query
constructors before those later owners were connected. An agent could not complete the first slice without
prematurely designing later slices.

**Decision:** settings are incremental. Foundation ships invocation/root/plain-storage settings only. Each later
vertical slice adds fields with its real consumer; #333 remains an umbrella until baseline fields are connected.

### F2 — encrypted-storage blocker

#330 and source documentation made #598's private selected-key work a prerequisite for the only object writer.
That contradicted the architecture, where encrypted storage is optional, and could block the entire product.

**Decision:** plain current-format immutable storage is baseline and implemented independently. #598 owns only an
optional encrypted profile after accepted public handoff. No private branch/worktree was inspected or changed.

### F3 — publication recovery explosion

The former publication recipe permitted physical point deletion. A delayed insert after deletion could recreate
an ID, forcing possible-writer tracking, route abandonment and isolated-generation succession.

**Decision:** baseline never physically deletes individual points. New points use fixed insert-only creation;
closures repeat one fixed `valid_until`; closed points stay until a fresh never-reused generation rebuild.
Cleanup deletes whole retired generations after view/window guards release. This removes the isolated-route
recovery platform.

### F4 — stale/conflicting active documents

The dependency graph used a broken implementation anchor; TEST_CUTOVER named R032 as current; the workspace map
still assigned local maintenance to publication despite the removed retention crate's documentation owner.

**Decision:** align navigation, test order and workspace disposition with the current implementation spine.

## Current implementation sequence

| Stage | Observable result | Main issues |
|---|---|---|
| Foundation | parsed invocation, typed root/plain settings, inspect/init/open and factual status | #333/#266/#360 |
| Walking skeleton | one file → plain ObjectRef → preparation → fresh Qdrant → one Search → pinned CLI excerpt | #264 plus direct owners |
| Convergence | restart/lost wakeups, inventory/reconcile, incremental publication, rebuild/compaction | #270/#271/#331/#261/#300/#303 |
| Session/Exact | Exact and one framed live session with QueryWindow | #249/#275/#280/#332 |
| Maintenance/depth | stopped-root collection/archive/import and selected optional capabilities | #302/#244/#281/#227/#335 |

## Non-blocking rules

- #598 encrypted storage, watcher, regex, MCP, model, overlay, document worker and managed Qdrant cannot block the
  plain lexical walking skeleton.
- Issue closure order is not implementation dependency order.
- One manager owns each vertical cut and integration commit.
- Current caller inspection is local to the code being moved/deleted; it is not another repository-wide audit.
- Broad tests follow connected code. Minimal Rust 1.98 check/Clippy follows each production cut.

## Evidence status

| Area | Status |
|---|---|
| Canonical architecture/implementation/owner handoff | CHECKED through R046 |
| Root/config/source/preparation/publication/query design | IMPLEMENTATION-READY, not implemented |
| Plain object profile | SELECTED, not implemented |
| Encrypted #598 profile | PROTECTED/OPTIONAL |
| Qdrant/client pin research | donor evidence only; real path unrun |
| Compiler evidence | R031 materializer + unitizer only |
| Windows/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

## Immutable prior records

- R001–R031: historical audit/removal/dependency work remains in Git history through `da382b66`.
- R032–R045: canonical documentation reconciliation and publication closeout remain at `af337ec7`.
- R046 supersedes the previous horizontal delivery order and late-writer cleanup recipe for current
  implementation work.

## Next action

Implement Foundation, then immediately the one-file walking skeleton. Update documentation only with concrete
code cuts or new verified evidence; do not restart a general documentation audit.
