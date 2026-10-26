# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions and unrun work; it is not
architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R050, 2026-10-26 — active Qdrant package/runtime instruction synchronization.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `9edd2c1d6276639f39c5ff81b02b4f5f40320d2a` (R049).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), then the
[implementation sequence](../implementation/README.md#delivery-sequence), then only the affected owner recipe,
actual current callers and owning issue.

R050 checks the remaining active Qdrant package/runtime entrypoints after R049. It changes documentation only. The
product remains unimplemented.

## R049 decisions retained

- Walking-skeleton code cut upgrades server/client/schema/query together to Qdrant 1.20.x.
- Baseline lexical retrieval uses one scoring full-text payload index, explicit `code` tokenizer/text processing
  and `TextQuery`; no baseline sparse BM25 inference or fixed local `avg_len`.
- Plain root/object Windows finalization is owner-private and handle-based after a native canary; current
  `tempfile::persist_noclobber` is not trusted as the Windows finalizer.
- `PublishedView` names an exact physical collection and matching redb snapshot; aliases are not serving authority.
- Stage 3A complete immutable generations are the correctness/reference profile and ship only for corpus classes
  whose predeclared #215 latency/throughput/resource/disk budgets pass.
- Stage 3B per-source point mutation is required where those budgets fail and absent otherwise.
- Per-root/source custom shards are not a general optimization because BM25 statistics are shard-local.
- Qdrant Edge remains an isolated later replacement candidate, never a fallback or permanent dual backend.

## R050 inspected active surfaces

- `docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`;
- `docs/runtime/QDRANT_NATIVE_WINDOWS.md`;
- `crates/search-index-qdrant/README.md` and `AGENTS.md`;
- `search-projection-planner/FUNCTIONS.md`;
- `search-point-identity/FUNCTIONS.md`;
- `search-index-reclaimer/FUNCTIONS.md`;
- `search-epoch-pins/FUNCTIONS.md`;
- R049 programme/issues/PR synchronization.

## Findings and corrections

### F1 — runtime Qdrant pages still described the pre-1.20 boundary

The runtime adapter was generic enough not to conflict directly, but it did not state the selected 1.20 scoring
full-text/`TextQuery` profile. The Windows page could leave the historical 1.19 smoke looking like current
platform evidence.

**Decision:** make 1.19 explicitly historical, state that no 1.20 artifact/runtime has been executed, and define
the exact 1.20 Windows evidence matrix.

### F2 — package index still made stable point identity look mandatory

The directory README described projection and point identity as permanent baseline responsibilities. Projection's
owner recipe began directly with changed-source diff, occurrence reuse, retain/create/close and validity closure.
Point identity required stable occurrence incarnation immediately.

**Decision:** Stage 2/3A projection is a complete-generation assembler with deterministic generation-bound point
IDs and actual retrieval-text payloads. Cross-generation stable identity, occurrence matching and closures are
Stage 3B only after failed budgets. The planner/identity crates may disappear if their baseline remainder is small.

### F3 — old reclaimer could restore point deletion and writer-ledger complexity

The reclaimer recipe still specified exact point-ID cleanup pages, sequence watermarks, possible delayed writers,
abandoned routes and per-point cursor/checkpoint state.

**Decision:** baseline cleanup is whole physical collection only. Move the small terminal delete/exact-absence
helper to publication/bridge and delete the platform. Stage 3B still never physically deletes individual points.

### F4 — view lifetime needed the physical-generation profile made explicit

The pin recipe was broadly correct but still spoke in route/sequence language.

**Decision:** Stage 2/3A `PublishedView` carries physical collection + retrieval profile + redb snapshot with no
sequence. Sequence appears only in Stage 3B. Resource evidence counts retained redb pages and physical generations,
not Arc count or a watermark.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | invocation; typed root/plain settings; handle-safe final-manifest-last root; one guard | #333/#266/#360 |
| 2 Walking skeleton | one file → ObjectRef → preparation → Qdrant 1.20 scoring full-text generation → TextQuery → CLI excerpt | #264 and vertical owners |
| 3A correctness/reference convergence | restart; authoritative inventory; complete fresh generation/view switch; measured release gate | #270/#271/#331/#261/#300/#303 |
| 3B optimization where budgets fail | source-local diff, stable occurrence identity, insert-only additions, fixed closures | #259/#329/#261/#263/#205 |
| 4 Exact/session | literal Exact and one framed live session/QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root collection/archive/import and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy and lock rules

- Issue closure order is not dependency order.
- One manager owns each vertical cut; parallel writes are disjoint.
- No backend abstraction, alias manager, custom-shard ranking layer, watcher ledger, init journal, point reclaimer
  or object-operation archive.
- No redb writer, view-manager mutex or QueryWindow mutation borrow spans object/Qdrant/process/model I/O.
- Complete build and durable evidence commit precede view installation.
- Last guard drop only wakes cleanup; it never deletes inline.
- Optional donors appear only with a consumer.
- Current-caller search is local to code moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad suites follow connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Qdrant 1.20 direct BM25/code-tokenizer review | CHECKED upstream; selected, not installed |
| Active Qdrant package/runtime instructions | SYNCHRONIZED through R050 |
| Generation-bound baseline point identity | SELECTED, not implemented |
| Stage 3B source-local diff/stable identity | CONDITIONAL on failed budgets |
| Whole-generation-only baseline cleanup | SELECTED, not implemented |
| `tempfile` platform implementation review | CHECKED; Windows finalizer corrected |
| Windows handle-based finalizer | SELECTED boundary, canary UNRUN |
| Fresh-generation release budgets | REQUIRED INPUT/MEASUREMENT, not defined or run |
| Qdrant Edge/custom sharding | REVIEWED, not baseline |
| Compiler evidence | R031 materializer + unitizer only |
| Current binaries/Foundation/Qdrant 1.20/walking skeleton | UNRUN |
| Windows durability/restart/relevance/scale/end-to-end | UNRUN |

No documentation review proves correctness. R050 removes active instructions which could have recreated mandatory
point-level mutation and the old reclaimer platform.

## Immutable prior records

- R001–R031: historical audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: first implementation-readiness correction at `1eeb4191`.
- R047: active-surface hostile verification at `858d1e34`.
- R048: donor/analogue synthesis and fresh-generation split at `fed10c1e`.
- R049: Qdrant 1.20/Edge/sharding and platform-finalization correction at `9edd2c1d`.
- R050: active Qdrant runtime/package/identity/reclaimer synchronization.

## Next action

Implement Foundation with the Windows handle-finalization canary and minimal compiler/Clippy. Then upgrade the real
Qdrant dependency/schema/query to 1.20 and implement #264. Do not begin the old changed-source planner, stable point
identity or point reclaimer before Stage 3A measurements require Stage 3B.
