# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions and unrun work; it is not
architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R048, 2026-10-25 — donor/analogue synthesis and simpler convergence plan.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `858d1e34a9053b6c3f51ba5d81d3b92ceb14b9f3` (R047).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), then the
[implementation sequence](../implementation/README.md#delivery-sequence), then only the affected owner recipe,
actual current callers and owning issue.

R048 reviews concrete implementation donors and analogous search systems. It changes documentation only. The
product remains unimplemented.

## R048 inspected evidence

### Primary donors and source revisions

- Git loose-object writer at `7e34d8174dcda3435d1404c9a30bef46afe6c13e`;
- `tempfile 3.27.0` source/API at current master;
- Tantivy at `b9c4ed1c62729001c3c1ce0cc6cdf7dd4a5eb82a`;
- Zoekt at `572f5e4a3a37532300298e2ba38513c15b2e0319`;
- ripgrep `ignore` walker and existing ELIOT source-reconcile donor notes;
- Watchman official clock/recrawl/subscription behavior;
- qntx/qmd at `8747cafab6b3af79d93f92b2c27ff5cfaf935c8f`;
- Veles at `f49adb4f828543d2b9bb5e3469582abcf03adaaa`;
- codesearch at `765e3daad6f4614b1d91789d682c6e94d5be3a08`;
- Qdrant official update-mode, collections/aliases, delete-collection and native-BM25 material.

### ELIOT owners checked

Implementation spine, dependency graph, donor register, root owner, plain object store, source reconciliation,
Qdrant bridge/publication, test cutover and active programme handoff.

## Findings and decisions

### F1 — stage 3 still introduced incremental complexity too early

The R047 sequence moved the first result ahead of broad recovery, but its next stage still bundled restart,
inventory, stable point identity, per-source point closure, compaction and cleanup.

Tantivy and Zoekt demonstrate the useful smaller pattern: build immutable derived artifacts, make visibility an
explicit later step and garbage-collect old artifacts separately. Their exact implementations are not adopted.

**Decision:** split convergence:

- **3A Fresh-generation convergence:** restart/reopen, authoritative inventory and desired-state reconciliation;
  any selected change builds a complete fresh never-reused Qdrant generation, commits matching evidence/current
  state, then explicitly installs `PublishedView`.
- **3B Measured incremental publication:** add stable occurrence reuse, `insert_only` point additions, fixed
  non-creating closures and compaction only after 3A works and full-rebuild cost is measured.

The fresh-generation profile is production architecture, not a throwaway. It uses the same retained objects,
prepared outcomes, evidence rows and physical collection identity. It prevents #329 identity continuity and
intra-generation mutation from blocking the first restart-convergent product.

### F2 — plain object publication had an unresolved stable-Rust primitive choice

`std::fs::rename` may replace a destination; stable Rust 1.98 has no stable `rename_noreplace`. Git supplies the
right shape: same-directory temporary file, streaming identity, sync, close and content-addressed finalization.

`tempfile 3.27.0` provides `NamedTempFile::persist_noclobber` with MSRV 1.63.

**Decision:** select `tempfile` as the preferred implementation-time candidate for #330/#360, not an installed
pin. Use final-directory staging, streaming address, `sync_all`, no-clobber persistence and exact existing/final
readback. A crash-left hard link/stage is safe maintenance debris. If Windows retained-handle/no-reparse needs a
smaller native finalizer, keep it private to the object/root owner instead of inventing a filesystem framework.

### F3 — Foundation initialization recovery remained larger than necessary

The current root task still allowed `initialize --resume`. Foundation creates only an empty current-format
control/object layout; no user source has been accepted yet.

**Decision:** select **manifest-file-last plus discard/retry**, not an initialization replay state machine:

```text
validate empty/owned target
→ create exact initialization marker + minimal layout
→ flush/close/verify
→ publish final root manifest no-clobber last
→ verify current root
```

Before the final manifest the root is `IncompleteInitialization`. Ordinary open writes nothing. Explicit
`discard-incomplete` removes only the exact recognized empty initialization layout after acquiring exclusion;
unknown/conflicting entries refuse and require manual relocation. No baseline resume path is needed.

### F4 — Qdrant aliases would create a cross-system split-brain risk

Qdrant can switch aliases atomically, but that switch cannot atomically install ELIOT's matching redb evidence
snapshot. Old pinned views also need the old physical generation.

**Decision:** `PublishedView` names the exact physical collection. Queries address it directly. Aliases are not
serving, recovery or cleanup authority and no alias manager is added to baseline.

### F5 — event systems confirm inventory-first correctness

Watchman treats clocks/events as observations and recrawls conservatively on uncertainty. The `ignore` donor has
separate builder, traversal and entry-attached error channels.

**Decision:** retain the existing ELIOT design: startup/explicit/periodic `ignore::WalkBuilder` inventory is
correctness; a watcher only sets stable-root dirty latches. Overflow/error/fresh instance schedules a root scan.
No durable event cursor, event replay ledger, Watchman service or whole-tree watcher cache.

### F6 — an embedded Tantivy fallback would not simplify scoped lexical search

Tantivy has excellent commit/reload and segment mechanics, but default BM25 statistics aggregate across all
segments. ELIOT would have to own exact scoped token/document statistics to reproduce its selected IDF semantics.
That creates another statistics subsystem and a second backend path.

**Decision:** keep Qdrant as the only selected indexed backend. Do not add a backend trait or Tantivy fallback.
After #264/3A, reconsider the backend only if measured setup, relevance, scoped-IDF, recovery or scale criteria
fail. A replacement prototype must consume the same retained/prepared/evidence boundaries and replace the
derived backend rather than dual-run indefinitely.

### F7 — modern analogues demonstrate both useful topology and failure modes

- qntx/qmd recently removed a database mutex from around embedding/reranking/generation and locks per statement;
  this reinforces ELIOT's no-lock-across-I/O rule.
- Veles writes its manifest first, treats a missing symbols artifact as empty, trusts equal size+mtime without
  rehash and derives removals from unseen paths. These are explicit counterexamples for ELIOT.
- codesearch combines many grammars, embedding downloads, two indexes, watcher/debouncer, HTTP/TUI, SCIP and a
  vendored MCP patch. It is a useful product catalogue but a negative architecture donor for the baseline.

**Decision:** preserve small leaves and progressive capability delivery. Do not interpret popularity or feature
breadth as a reason to adopt a subsystem.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | parsed invocation; root/plain settings; inspect/init/open; one guard; factual status | #333/#266/#360 |
| 2 Walking skeleton | one file → plain ObjectRef → preparation → one fresh generation → Search → retained CLI excerpt | #264 and vertical owners |
| 3A Fresh-generation convergence | restart/reopen; authoritative inventory; desired-state reconciliation; complete fresh generation and view switch | #270/#271/#331/#261/#300/#303 |
| 3B Incremental optimization | stable occurrence reuse; point additions/fixed closures; measured compaction thresholds | #329/#259/#261/#263 |
| 4 Exact/session | literal Exact and one framed live session/QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root collection/archive/import and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy and lock rules

- Issue closure order is not dependency order.
- One manager owns each vertical integration cut; parallel writes are disjoint.
- No backend abstraction, alias manager, watcher ledger, init journal or object-operation archive.
- No redb writer, view-manager mutex or QueryWindow mutation borrow spans object/Qdrant/process/model I/O.
- A complete build and durable evidence commit precede view installation.
- Last guard drop only wakes cleanup; it never deletes inline.
- Optional donors appear only with their consumer.
- Current-caller search is local to code being moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad suites follow connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Donor/analogue source inspection | CHECKED for named revisions/questions |
| Fresh-generation-first decision | SELECTED, not implemented |
| `tempfile` no-clobber candidate | INSPECTED/SELECTED CANDIDATE, not installed |
| Root discard-only initialization | SELECTED, not implemented |
| Qdrant physical-generation/no-alias serving | SELECTED, not executed |
| Qdrant incremental update/cleanup races | STAGE 3B / REQUIRED LIVE CASES |
| Watcher/inventory authority | CHECKED design, not implemented |
| Backend replacement criteria | DOCUMENTED, not measured |
| Compiler evidence | R031 materializer + unitizer only |
| Current binaries/Foundation/walking skeleton | UNRUN |
| Windows durability/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No analogue proves ELIOT correctness. R048 narrows the code required before the next falsifiable product result.

## Immutable prior records

- R001–R031: historical audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: first implementation-readiness correction at `1eeb4191`.
- R047: active-surface hostile verification at `858d1e34`.
- R048: donor/analogue synthesis, object/root primitive selection and fresh-generation convergence split.

## Next action

Implement Foundation with the selected manifest-last/discard-only plain root, run minimal compiler/Clippy, then
implement #264. The first convergence extension is restart plus complete fresh-generation rebuild. Do not begin
incremental point mutation, stable cross-revision point reuse, watcher integration or backend abstraction first.