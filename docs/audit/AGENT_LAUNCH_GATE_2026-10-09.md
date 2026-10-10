# Agent launch gate — current execution state

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.
**Live writer queue:** programme issue #352.
**Mandatory process:** [EXECUTION_PROTOCOL.md](../product/EXECUTION_PROTOCOL.md).
**Long-range obligations:** [PROJECT_COMPLETION.md](../product/PROJECT_COMPLETION.md).
**Current process issues:** #349 and #350.

`PROJECT_COMPLETION.md` is not the live next-task queue. Its dated “next task”, base SHA or local stage wording is subordinate to #352, this gate, current merged artifacts and the exact active issue.

## Current verdict

```text
Wave 1 canonical foundation:             MERGED
Wave 1 Unicode full-fold decision:       MERGED
Shared indexed contract/tooling:         MERGED
S11 point identity #256:                 MERGED via #338
UnitSet v3 #257:                         MERGED via #342
Rustls repair #327:                      MERGED via #341
Current writer:                          process PR #351, source-free
PR #344 disposition:                     CLOSED UNMERGED / FROZEN DONOR 6d698ea
Next source slice:                       dependency-ready #354 child after #351
Next independent owner after root API:   #235.core
Installed product / release:             NOT QUALIFIED
```

Earlier text saying the next task is #256 or #257 is historical. Do not repeat delivered work.

## Launch topology

One manager owns one writer worktree, commits, merges, root dependency pins and `Cargo.lock`. Five to ten subagents perform bounded read/research/review assignments.

Every source task is a bounded `SLICE`:

```text
Definition of Ready
→ SCOPE_FROZEN by second source commit
→ bounded subagent reports
→ manager implementation
→ changed owners + immediate reverse-consumer gates
→ affected static guards + deferred test inventory
→ formal exact-head review
→ merge
→ next branch from new main
```

No stacked source PRs. Read-only research for the next non-overlapping slice may run during final review.

## Current #266 gate

PR #344 has 119 changed files and more than 11,000 additions. It must not continue through more “narrow continuation” amendments.

The manager must:

1. preserve the closed #344 donor at exact `6d698eafa573e7a8a5eb9dca6ce17ae7878c3bce` without new commits;
2. classify every remaining finding `B0`, `F1`, `F2`, `D` or `Q`;
3. merge #351, make #354 dependency-ready and choose only its first exact tranche; #355/#356/#358 are PROGRAM parents, not source branch authority;
4. open/review/merge tranche 1;
5. publish the new `main` SHA;
6. create tranche 2 from that new `main`; repeat serially as needed;
7. keep #343/#345/#346/#347/#348 outside unless an exact B0 dependency is proved;
8. obtain formal review on each final head;
9. allow #235.core after the minimum root API/caller cutover merges.

Unsupported recovery/cleanup remains unavailable and fail-closed. It is not fabricated merely to merge.

## Finding triage

```text
B0  required for declared slice compilation or safety
F1  same-owner follow-up
F2  adjacent-owner follow-up
D   unchanged baseline/harness/lint debt
Q   native/live/scale/release qualification
```

A finding becomes B0 only with exact path/symbol/caller evidence showing why the frozen result cannot remain unavailable, legacy-only or fail-closed.

## Build gate

For each source tranche:

- Rust 1.98 locked check for changed owners;
- immediate reverse-consumer compilation for public API changes;
- strict Clippy for that production closure;
- specify deferred unit/integration/native/crash/fault inventory with its qualification owner;
- affected source guards.

Use production targets only (`--lib`/`--bins` as applicable). Tests execute during integrated qualification, not as source merge gates. Tiny debugging runs are diagnostic only. Reuse recorded broad baseline debt; compare only relevant failing production diagnostics once. New candidate production diagnostics block.

Full workspace, installed Windows, live Qdrant, fault, scale, disclosure and release evidence remain #264/#215/#140 or exact named gates.

## PR policy

Open PR means merge candidate. Do not open new programme/tracking/gate PRs. Historical non-mergeable PRs are closed after current owner links are preserved.

The PR body is the current status record. Formal review binds the final SHA, no unresolved B0/P1, and explicit F1/F2/D/Q dispositions. Self-approval is not required; a formal COMMENT submission may record independent reviewer evidence and manager disposition. A badge alone is insufficient. PROCESS/DOCS are source-free. After #351, three bounded source merges precede another process programme PR unless a verified B0 prevents coding.

## Delivered evidence scope

Completed source deliveries remain source-scoped:

- #319 canonical/digest foundation;
- #322 Unicode decision/goldens;
- #323 indexed contract;
- #326 Cargo metadata/status tooling;
- #338 S11 point identity;
- #340 Windows metadata prefix fix;
- #341 Rustls patch;
- #342 UnitSet v3.

None establishes installed/native/Qdrant/full-product qualification. Continue from current `main`; never reset to a historical SHA embedded in an older packet.
