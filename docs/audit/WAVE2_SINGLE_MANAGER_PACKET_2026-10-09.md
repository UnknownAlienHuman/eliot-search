# Wave 2 single-manager execution packet — current revision

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.  
**Whole-project map:** [PROJECT_COMPLETION.md](../product/PROJECT_COMPLETION.md).  
**Mandatory process:** [EXECUTION_PROTOCOL.md](../product/EXECUTION_PROTOCOL.md).  
**Exact package exceptions:** [WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md).

This packet supplies the remaining Wave-2 source order and reusable subagent assignments. It no longer authorizes one PR to grow until a whole programme closes. Every implementation is a bounded mergeable `SLICE` under the execution protocol.

## 1. Delivered; do not repeat

| Task | PR | Actual delivery |
|---|---:|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest helpers, stored/compute distinction and source guard |
| #253 | #322 | Unicode 18 full-fold decision/goldens; no production tokenizer |
| #258 | #323 | Shared indexed payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata and package-status/dependency validation |
| #256 | #338 | Stateless S11 identity profile; legacy retirement remains #329 |
| #339 | #340 | Windows Cargo path-prefix handling in Qdrant boundary tooling |
| #327 | #341 | Rustls 0.23.45 security repair; live acceptance remains later |
| #257 | #342 | Source-verified UnitSet v3; durable integration remains #331 |

Package checks and focused fixtures recorded by these PRs remain scoped source evidence. They do not qualify the installed product.

## 2. Current writer slice

The current programme is #266. Draft PR #344 contains substantial typed root-admission code, but its scope expanded to 119 files and multiple owner families. It is governed by #349 and #350.

Before more source work, the manager must:

```text
freeze current scope
→ classify remaining findings
→ split into reviewable dependency-ordered tranches as needed
→ run minimum source gates
→ obtain exact-head review
→ merge the minimum root-admission API/caller cutover
```

The following findings have separate owners and are not implicit blockers:

```text
#343 redb read-only inspection
#345 record-artifact unknown-publication staging retention
#346 all-target fixture compilation debt
#347 native original-object unlink/late cleanup
#348 original request → durable effect reconciliation
```

A finding may stay in the active PR only as a documented `B0` under the execution protocol. Unsupported recovery/cleanup remains unavailable and fail-closed.

After the minimum typed root API and required normal callers merge, start `#235.core` from the resulting `main`. Do not wait for complete #266 programme, harness, cleanup, redb-inspection or installed-qualification closure.

## 3. Remaining Wave-2 source order

The order names API/artifact dependencies, not whole-programme completion barriers:

```text
#266 bounded root-admission tranches
→ #235.core typed provider client
→ #238 maintained TOML and honest v1/v2 digest migration
→ #241 effective bounded GlobSet admission
→ #246 mature literal matcher cutover
→ #252 XID + NFC code identifiers
→ #226 Markdown
→ #226 JATS
```

Other whole-project stages proceed through `PROJECT_COMPLETION.md` after their actual prerequisites land. Do not restart #237/#253/#258/#250/#256/#257.

## 4. Definition of Ready for every slice

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

Publish `SCOPE_FROZEN` no later than the second source commit. Subsequent findings are classified `B0`, `F1`, `F2`, `D` or `Q`; they do not automatically widen the PR.

Default split triggers:

```text
> 30 changed production files
> 2,500 changed production lines
> 1 primary owner
> 2 adapter families
> 1 persisted migration
> 1 cross-owner cutover
```

Correctness is not omitted to meet a number. Crossing a threshold means split or obtain a reviewed exception before continuing.

## 5. Reusable subagent assignments

Use only the roles needed for the active slice.

| Agent role | Read scope | Return artifact |
|---|---|---|
| Authority/caller | exact issue, package instructions and current callers | owner, allowed paths, reverse consumers, duplicate paths |
| Donor/supply-chain | accepted donor reports and primary source | exact version/checksum/license/MSRV/features/advisories |
| Compatibility/deletion | current persisted profile and old callers | byte disposition, rebuild/migration rule, deletion owner |
| Recovery/security | active candidate source | `B0`, `FOLLOW_UP(owner)` or `NO_BLOCKER` with causal evidence |
| Bounds/allocation | loops, buffers, effects and deadlines | pre-allocation/effect ceiling gaps and disposition |
| Focused fixtures | changed boundary | smallest causal cases and expected nonzero count |
| Diff/API | candidate patch and reverse consumers | API delta, scope escape, unused compatibility |
| Final exact-head | final SHA and command records | blocking finding or scoped `APPROVE_SOURCE` |

Rules:

- one pre-code pass and one final exact-head review pass;
- no repeated broad repository audit after each commit;
- no subagent-created scope amendment or merge authority;
- consolidate duplicate findings;
- a load-bearing commit invalidates earlier review;
- the manager independently verifies the critical claims.

## 6. Source gates

For every tranche:

```text
locked Rust 1.98 check for changed owners
immediate reverse-consumer compilation for public APIs
strict Clippy for the same production closure
focused causal fixtures with nonzero cases
affected canonical/Qdrant/package source guards
```

Capture broad baseline failures once, compare once at final head. New candidate diagnostics block. Identical unrelated baseline debt is `D`, not a source gate.

Do not repeatedly repair the entire all-target test graph inside an unrelated source branch. Full workspace, native Windows, live Qdrant, fault, scale and release evidence remain their named late gates.

## 7. PR and merge discipline

One writer and one active source PR remain mandatory. The PR body is the current status record; do not publish a long status comment after every commit.

Before merge, require a formal review bound to the exact final SHA. A security-review badge, signature, source guard or author statement is not independent acceptance.

A broad programme issue may remain open after a source tranche merges. The next independent owner begins once its actual prerequisite artifact is on `main`, not when the entire prior programme and every follow-up is closed.

Open PR means merge candidate. Historical programme/tracking/gate packet PRs should be closed after their useful obligations point to current issues/docs. Do not create new non-mergeable coordinator PRs.

## 8. Donor and compatibility corrections retained

- `search-contracts` remains the sole canonical/digest owner.
- `sha256_raw` is domain-separated; v1 exact-byte compatibility must not add a prefix.
- Existing S11 legacy callers remain explicitly legacy until #259/#262 and deletion owner #329.
- UnitSet v2 cannot masquerade as delivered v3; durable preparation integration remains #331.
- Parser/matcher/Unicode/Git/document donors stay behind narrow adapters and exact profile identities.
- `quick-xml` positions require explicit raw-range mapping; Markdown/JATS consume the delivered UnitSet model, not a second document catalogue.
- Qdrant remains the sole indexed substrate; no parallel search database or vendor type leakage.

## 9. Stop conditions

Stop the affected slice and file/route a follow-up when:

- work belongs to another owner;
- a second authority, catalogue, journal, parser or client would be created;
- persisted bytes would be reinterpreted without a versioned migration;
- a package-only change would break a forbidden consumer;
- donor source contradicts the assumed API/profile;
- broad baseline debt is being repaired merely to obtain a green aggregate command;
- a qualification demand is being pulled into source implementation.

Do not stop the whole project. Merge the coherent fail-closed result, preserve the follow-up and continue from the new accepted base.
