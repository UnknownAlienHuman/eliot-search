# Wave 2 single-manager execution packet — current revision

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.  
**Live writer queue:** programme issue #352 and [launch gate](AGENT_LAUNCH_GATE_2026-10-09.md).  
**Long-range obligations:** [PROJECT_COMPLETION.md](../product/PROJECT_COMPLETION.md).  
**Mandatory process:** [EXECUTION_PROTOCOL.md](../product/EXECUTION_PROTOCOL.md).  
**Exact package exceptions:** [WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md).

`PROJECT_COMPLETION.md` is not the live next-task queue. Its dated “next task”, base SHA and local stage text are subordinate to #352, the launch gate, current merged artifacts and the exact active issue.

This packet supplies remaining Wave-2 source order and reusable subagent assignments. Every implementation is a bounded mergeable `SLICE`; no PR grows until an entire programme closes.

## 1. Delivered; do not repeat

| Task | PR | Actual delivery |
|---|---:|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest helpers, stored/compute distinction and source guard |
| #253 | #322 | Unicode 18 full-fold decision/goldens; no tokenizer |
| #258 | #323 | Shared indexed payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata and package-status validation |
| #256 | #338 | Stateless S11 identity; legacy retirement remains #329 |
| #339 | #340 | Windows Cargo path-prefix handling in Qdrant tooling |
| #327 | #341 | Rustls 0.23.45 repair; live acceptance remains later |
| #257 | #342 | Source-verified UnitSet v3; durable integration remains #331 |

These are scoped source deliveries, not installed product acceptance.

## 2. Current writer slice

The current programme is #266. Draft PR #344 contains substantive root-admission code but expanded to 119 files and multiple owner families. It is governed by #349/#350.

Before more source work:

```text
freeze current scope
→ classify remaining findings
→ select only the first dependency-safe tranche
→ open/review/merge that tranche
→ publish new main SHA
→ create the successor branch from new main
```

Never open stacked source PRs.

Separate owners unless an exact `B0` dependency is proved:

```text
#343 redb read-only inspection
#345 record-artifact unknown-publication staging retention
#346 all-target fixture compilation debt
#347 native original-object unlink/late cleanup
#348 original request → durable effect reconciliation
```

Unsupported recovery/cleanup remains unavailable and fail-closed.

After the minimum typed root API and required normal callers merge, start `#235.core` from the resulting `main`. Do not wait for complete #266 programme, harness, cleanup, redb inspection or qualification closure.

## 3. Remaining Wave-2 writer train

```text
#266 bounded serial tranches
→ #235.core typed provider client
→ #238 maintained TOML and honest v1/v2 digest migration
→ #241 bounded GlobSet admission
→ #246 mature literal matcher cutover
→ #252 XID + NFC code identifiers
→ #226 Markdown
→ #226 JATS
```

Other whole-project stages follow the long-range map only after their actual prerequisites are merged. Do not restart delivered tasks.

## 4. Definition of Ready

Before code, record:

```text
exact base SHA
primary owner/state/effect
allowed production paths
at most two narrow adapter families
immediate reverse consumers
persisted-byte/profile disposition
legacy replacement/deletion owner
minimum check/Clippy/focused fixtures
known baseline failures
non-goals and adjacent owners
```

Publish `SCOPE_FROZEN` no later than the second source commit. Findings become `B0`, `F1`, `F2`, `D` or `Q`; they do not automatically widen the PR.

Default split triggers:

```text
> 30 changed production files
> 2,500 changed production lines
> 1 primary owner
> 2 adapter families
> 1 persisted migration
> 1 cross-owner cutover
```

Crossing a trigger means stop feature work, choose tranche 1, merge it, then create tranche 2 from the newly merged `main`. No stacked PRs.

## 5. Reusable subagent assignments

| Role | Return artifact |
|---|---|
| Authority/caller | owner, allowed paths, reverse consumers, duplicate paths |
| Donor/supply-chain | exact version/checksum/license/MSRV/features/advisories |
| Compatibility/deletion | byte disposition, rebuild/migration rule, deletion owner |
| Recovery/security | `B0`, `FOLLOW_UP(owner)` or `NO_BLOCKER` with causal evidence |
| Bounds/allocation | pre-allocation/effect ceiling gaps and disposition |
| Focused fixtures | smallest causal cases and expected nonzero count |
| Diff/API | API delta, scope escape, unused compatibility |
| Final exact-head | blocking finding or scoped `APPROVE_SOURCE` |

Rules:

- one pre-code pass and one final exact-head pass;
- no repeated full-repository audit after each commit;
- no subagent-created scope amendment or merge authority;
- consolidate duplicate findings;
- a load-bearing commit expires prior review;
- the manager independently verifies critical claims.

Read-only research for the next non-overlapping slice may run during final review, but no successor branch/PR is opened before current merge.

## 6. Source gates

For every tranche:

```text
locked Rust 1.98 check for changed owners
immediate reverse-consumer compilation for public APIs
strict Clippy for the same production closure
focused causal fixtures with nonzero cases
affected source guards
```

Capture broad baseline failures once and compare once at final head. New candidate diagnostics block. Identical unrelated baseline debt is `D`, not a source gate.

Do not repair the entire all-target graph inside an unrelated branch. Full workspace, native Windows, live Qdrant, fault, scale and release evidence remain late gates.

## 7. PR and merge discipline

One writer and one active source PR are mandatory. The PR body is the current status record; do not post a long progress comment after every commit.

Before merge, require formal review bound to the final SHA. A security-review badge, signature, guard or author statement is not independent acceptance.

A programme issue may remain open after a slice merges. The next independent owner begins once its actual artifact is on `main`, not when every prior follow-up is closed.

Open PR means merge candidate. Historical programme/tracking/gate packet PRs are closed after current owner links are preserved. Do not create new coordinator PRs.

## 8. Donor and compatibility rules retained

- `search-contracts` remains the sole canonical/digest owner.
- Exact-byte SHA compatibility is distinct from domain-separated `sha256_raw`.
- Existing S11 legacy callers remain legacy until #259/#262 and deletion owner #329.
- UnitSet v2 cannot masquerade as v3; durable preparation remains #331.
- Parser/matcher/Unicode/Git/document donors stay behind narrow adapters and exact profile identities.
- Markdown/JATS consume delivered UnitSet types, not a second document catalogue.
- Qdrant remains the sole indexed substrate; vendor types stay private.

## 9. Stop conditions

Stop the affected slice and route a follow-up when:

- work belongs to another owner;
- a second authority/catalogue/journal/parser/client would be created;
- persisted bytes would be reinterpreted without versioned migration;
- a package-only change would break a declared consumer;
- donor source contradicts the assumed API/profile;
- broad baseline debt is being repaired merely to obtain a green aggregate command;
- qualification work is being pulled into source implementation.

Do not stop the whole project. Merge the coherent fail-closed result, preserve the follow-up and continue from the new accepted base.
