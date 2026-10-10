# Agent launch gate — current execution state

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.  
**Whole-project map:** [PROJECT_COMPLETION.md](../product/PROJECT_COMPLETION.md).  
**Mandatory process:** [EXECUTION_PROTOCOL.md](../product/EXECUTION_PROTOCOL.md).  
**Current process issues:** #349 and #350.

## Current verdict

```text
Wave 1 canonical foundation:             MERGED
Wave 1 Unicode full-fold decision:       MERGED
Shared indexed contract/tooling:         MERGED
S11 point identity #256:                 MERGED via #338
UnitSet v3 #257:                         MERGED via #342
Rustls security repair #327:             MERGED via #341
Current writer slice:                    #266 / draft PR #344
PR #344 disposition:                     FREEZE OR SPLIT BEFORE MORE SCOPE
Next independent owner after root API:   #235.core
Installed product / release:             NOT QUALIFIED
```

Earlier text saying the next task is #256 or #257 is historical. Do not repeat delivered work.

## Launch topology

One manager owns one writer worktree, commits, merges, root dependency pins and `Cargo.lock`. Five to ten subagents may perform bounded read/research/review assignments.

The single-writer rule does not authorize an unlimited branch. Every source task is a bounded `SLICE` under the execution protocol:

```text
Definition of Ready
→ SCOPE_FROZEN by the second source commit
→ bounded subagent reports
→ manager implementation
→ changed owners + immediate reverse-consumer gates
→ focused causal fixtures
→ formal exact-head review
→ merge
```

No stacked unmerged source PRs. Read-only research for the next non-overlapping slice may begin while the current slice is in final review.

## Current #266 gate

PR #344 contains substantive implementation but exceeded the default review budget: 119 files, more than 11,000 additions and several owner families. It must not continue through more “narrow continuation” amendments.

The manager must:

1. publish one final `SCOPE_FROZEN` path/behavior inventory;
2. classify every remaining finding as `B0`, `F1`, `F2`, `D` or `Q`;
3. split the branch into dependency-ordered mergeable tranches when required;
4. keep #343/#345/#346/#347/#348 outside the tranche unless a concrete `B0` dependency is proved;
5. run the minimum production/reverse-consumer gates and focused causal fixtures;
6. obtain formal review at each final exact head;
7. merge the minimum typed root-admission API/caller cutover;
8. allow #235.core to start from the resulting `main` without waiting for complete #266 programme closure.

Unsafe or incomplete recovery/cleanup remains unavailable and fail-closed. It must not be fabricated merely to merge.

## Finding triage

```text
B0  required for declared slice compilation or safety; may stay in PR
F1  same-owner follow-up; file issue and merge bounded slice
F2  adjacent-owner follow-up; file under actual owner
D   unchanged baseline/harness/lint debt; not the slice gate
Q   native/live/scale/release qualification; route to named late gate
```

A finding becomes `B0` only when the manager explains why the frozen result cannot safely remain fail-closed, unavailable or legacy-only.

## Build gate

For each source tranche:

- Rust 1.98 locked check for changed owner packages;
- immediate reverse-consumer compilation for public API changes;
- strict Clippy for that production closure;
- focused nonzero causal fixtures;
- affected source guards.

Capture broad baseline failures once and compare once at final head. Unchanged pre-existing all-target debt does not block the tranche; new candidate diagnostics do.

Full workspace, installed Windows, live Qdrant, fault, scale, disclosure and release evidence remain #264/#215/#140 or their exact named gates.

## PR policy

Open PR means merge candidate. Do not open new programme/tracking/gate PRs. Historical non-mergeable PRs are closed after their useful obligations are preserved in current issues/docs.

The PR body is the current status record. Do not add a long progress comment after every commit. A security-review badge, source guard, signature or author statement is not independent acceptance.

## Evidence already scoped

Completed source deliveries remain source-scoped:

- #319 canonical/digest foundation;
- #322 Unicode decision/goldens;
- #323 indexed contract;
- #326 Cargo metadata/status tooling;
- #338 S11 point identity;
- #340 Windows metadata path-prefix fix;
- #341 Rustls patch;
- #342 UnitSet v3.

None establishes installed/native/Qdrant/full-product qualification. Continue from current `main`; never reset to the historical SHA embedded in an older packet.
