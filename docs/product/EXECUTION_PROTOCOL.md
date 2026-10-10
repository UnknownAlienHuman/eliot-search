# ELIOT Search bounded execution protocol

**Authority:** root `AGENTS.md`, Architecture Part I, accepted ADRs, the exact source issue, and process issue #350.  
**Applies to:** all remaining implementation, migration, integration, qualification and release work.  
**Topology:** one manager, one writer worktree, one dependency/`Cargo.lock` integrator, 5–10 bounded read/research/review subagents.

This protocol fixes an execution defect, not a product invariant. The repository already prevents overlapping writers, but previous instructions did not prevent one valid source task from absorbing every new finding discovered during implementation. The result was large draft PRs, repeated scope amendments, blocked independent work and review that never reached a stable exact head.

The manager must optimize for **small accepted source increments that delete duplicate authority**, not for completing an entire programme in one branch.

---

## 1. Work-item classes

Every active item has exactly one class.

### PROGRAM

A capability or product tracker. It lives as an issue and in `PROJECT_COMPLETION.md`. It may remain open across many source deliveries. It never serves as a code merge base and never accumulates implementation commits.

### SLICE

One causal source result, delivered by one mergeable PR. A slice has one primary state/effect owner, explicit narrow adapters, declared reverse consumers, one compatibility/deletion boundary and exact minimum gates.

### FOLLOW-UP

A defect discovered outside the frozen slice. It names the actual owner, affected path/symbol, consequence and dependency edge. Finding a follow-up is progress; absorbing it into the current PR by default is not.

### GATE

Evidence or qualification against an exact immutable candidate. A gate does not fix product source. Failures return to source-owner slices and create a new candidate.

**Do not create new programme, tracking or gate pull requests. Open pull requests are merge candidates.**

---

## 2. Definition of Ready

Before source edits, the manager records the following in the implementation issue or PR body:

```text
class: SLICE
base_sha:
branch:
primary_owner:
state_or_effect:
allowed_production_paths:
narrow_adapters:
reverse_consumers:
non_goals:
donor_or_spec_sources:
persisted_profile_decision:
replacement_or_deletion_boundary:
minimum_check:
minimum_clippy:
focused_fixtures:
known_baseline_failures:
```

A slice is not ready when the owner, compatibility rule, reverse consumers or minimum gates are still being invented during implementation.

### Path ownership

One slice owns:

- one primary package or one cohesive daemon composition owner;
- at most two narrowly named adapter families;
- only the direct reverse consumers needed to compile and cut over that result.

A path glob such as `bins/eliot-searchd/src/**` is not a useful slice boundary.

### Persisted identities

Before changing persisted bytes, digests, profile identity, schema or generation semantics, the slice states exactly one disposition:

```text
BYTE_COMPATIBLE
VERSIONED_REBUILD
LEGACY_DECODE_ONLY
UNSUPPORTED_REQUIRES_MIGRATION
```

No later review finding may silently change this disposition inside the same PR.

---

## 3. Scope freeze

The manager publishes a `SCOPE_FROZEN` block no later than the second source commit.

It contains:

```text
exact base SHA
exact head SHA at freeze
allowed production path list
declared public/private API delta
reverse-consumer list
known legacy path retained and its deletion owner
minimum source gates
out-of-scope owner list
```

After `SCOPE_FROZEN`, a comment cannot add another caller family, schema, lifecycle protocol, recovery owner or package. A necessary scope change requires one of:

1. revert to the last frozen head and redefine the slice before continuing; or
2. finish the current coherent result and file the new work under its owner; or
3. split the existing branch into ordered mergeable PRs.

Repeated “narrow continuation” comments are not a substitute for freezing scope.

---

## 4. Finding triage

Every new source/review finding receives exactly one disposition.

### B0 — same-slice blocker

The active PR cannot merge because, without the fix, its declared result:

- does not compile for a declared immediate reverse consumer;
- mints false authority or accepts caller-shaped proof;
- corrupts or ambiguously rewrites data;
- widens access or disclosure;
- makes a newly enabled production path unsafe;
- cannot remain fail-closed or unavailable.

A B0 report must name the exact path, symbol, caller and causal chain. “Related to the programme” is insufficient.

### F1 — same-owner follow-up

The finding is valid and belongs to the same package, but the frozen result remains correct when the affected behavior stays unavailable, legacy-only or fail-closed. Create a follow-up issue and merge the slice.

### F2 — adjacent-owner follow-up

The finding belongs to another package, state/effect owner, integration phase or qualification lane. Create an issue under that owner. Never edit that owner in the active PR merely to keep the programme moving.

### D — baseline/debt

The failure existed on the accepted base and the candidate introduces no new normalized diagnostic, broken caller or behavior. Record it once under the debt owner. It is not the slice gate.

### Q — qualification

The finding asks for installed/native/live-Qdrant/fault/scale/release evidence. Route it to #215, #264 or the exact named gate. Do not expand source scope to simulate acceptance.

### Decision rule

A follow-up becomes B0 only when the manager demonstrates why the frozen result cannot safely remain fail-closed, unavailable or legacy-only. The burden is on widening the PR, not on merging the bounded result.

---

## 5. Default review budget

These are mandatory split triggers, not permission to omit necessary correctness:

```text
primary owners:              1
narrow adapter families:     <= 2
changed production files:    <= 30
changed production lines:    <= 2,500
persisted migrations:        <= 1
cross-owner cutovers:         <= 1
```

Focused fixtures, exact generated goldens and source-ledger rows are reported separately from production-line totals.

When a threshold is exceeded:

1. stop adding feature work;
2. publish the exact reason and path groups;
3. split into dependency-ordered PRs;
4. obtain review on each final exact head.

A review-budget exception requires a written justification accepted before additional source changes. A broad issue does not itself justify a broad PR.

---

## 6. One-manager merge train

One writer remains mandatory. Throughput comes from merging smaller slices, not from several overlapping writers.

```text
select dependency-ready slice
→ freeze source scope
→ run bounded subagent research
→ manager implements and integrates
→ compile declared owners and reverse consumers
→ run focused causal fixtures
→ exact-head independent review
→ merge
→ publish new main SHA
→ start next writer slice from new main
```

### Permitted parallelism

While the current PR is in final review, subagents may research the next non-overlapping slice and return read-only maps. They may not create a stacked source PR, mutate shared manifests or assume the current PR will merge unchanged.

### Programme closure is not a merge prerequisite

A broad programme issue remains open after a coherent source slice merges. The next independent owner may begin when its actual API/artifact prerequisite is on `main`; it does not wait for every follow-up, optional profile or final qualification obligation of the preceding programme.

---

## 7. Subagent assignments

Use only the assignments relevant to the active slice.

| Role | Required output |
|---|---|
| Authority/caller map | exact owner, callers, reverse consumers, duplicate paths |
| Donor/supply-chain | exact source/version/checksum/license/MSRV/features/advisories |
| Compatibility/deletion | persisted-byte disposition and exact legacy deletion owner |
| Recovery/security red-team | B0 or follow-up findings with exact causal evidence |
| Bounds/allocation | pre-allocation/effect ceilings and deadline/cancellation gaps |
| Focused fixtures | smallest causal cases, no broad test programme |
| Diff/API review | public/private delta, unused compatibility and scope escapes |
| Final exact-head review | blocking findings or `APPROVE_SOURCE` at one SHA |

Rules:

- one pre-code research pass and one final exact-head review pass;
- no repeated full-repository audit for every commit;
- no subagent-created architecture or scope amendment;
- every finding ends with `B0`, `FOLLOW_UP(owner)` or `NO_BLOCKER`;
- duplicate reports are consolidated;
- the manager independently verifies load-bearing claims;
- a later load-bearing commit expires prior review.

---

## 8. Development gates

For a source slice, run:

1. locked Rust 1.98 check for changed owners;
2. immediate reverse-consumer compilation for changed public APIs;
3. strict Clippy for the same production closure;
4. only focused causal fixtures required by the changed boundary;
5. affected repository source guards.

### Baseline failures

Capture a complete baseline diagnostic fingerprint once. At the final candidate head, compare it once more.

```text
candidate adds diagnostic/signature/caller break → BLOCK
candidate preserves identical unrelated debt     → D follow-up, not slice gate
```

Do not rerun and partially repair the full known-broken all-target graph after every commit. Do not hide new failures behind an old failing aggregate command.

### Deferred gates

Full workspace, installed Windows, real Qdrant, fault injection, scale, resource, disclosure and release evidence remain their explicit late gates. Source presence and compilation do not satisfy them.

---

## 9. Review and PR reporting

The PR body is the single current status record. It contains:

```text
frozen scope
exact base/head
changed owner/path groups
delivered causal result
legacy path retained and deletion owner
follow-up issues and disposition
commands, exit codes and nonzero focused case counts
known baseline debt and candidate delta
evidence boundary
```

Do not post a new long status comment after every commit. Use commits for implementation history and update the PR body at stable checkpoints.

Before merge, at least one formal GitHub review must target the final head and verify the load-bearing invariants of the slice. A security-review badge, signature, source-guard PASS or author comment is not independent acceptance.

---

## 10. Closing issues and PRs

### Implementation issue

Close when its declared source slice is merged and its exact residual obligations are linked to current follow-ups. Do not keep a completed implementation issue open merely because the whole product is unfinished.

### Programme issue

May remain open until all selected source/integration/qualification obligations are delivered or explicitly deferred by a maintainer.

### Pull request

Open PR means merge candidate. Historical programme/tracking/gate packet PRs are closed after:

1. useful obligations are retained in current issues/docs;
2. a closure comment names the replacement owners;
3. no agent is instructed to branch from the old head.

Do not create replacement tracking PRs.

---

## 11. Immediate #266 / #344 application

PR #344 exceeded every default split trigger: 119 changed files, more than 11,000 additions and several owner families. Process issues #349 and #350 now control its disposition.

The manager must freeze the current branch and determine dependency-safe tranches. The commit history suggests these review units, subject to exact dependency verification:

1. **Root admission and explicit initialization** — typed modes, existing-only open, retained request/deadline/cancellation, required immediate caller cutover.
2. **Focused lifecycle/crash proof** — only fixtures and minimal production seams required to prove the first tranche.
3. **Durable catalog intent and read-only recovery observation** — retained inputs, exact release, named inspection/discovery; no effect reconciliation or cleanup authority.

The following stay separate unless a concrete B0 dependency is proved:

```text
#343 redb read-only inspection
#345 control-artifact unknown-outcome staging retention
#346 all-target fixture compilation debt
#347 native original-object unlink and late cleanup
#348 original request → durable effect reconciliation
```

Unsafe or unsupported recovery/cleanup remains unavailable and fail-closed. It does not have to be falsely completed in order to merge a safe admission tranche.

After the minimum typed root-admission API and its required normal callers are on `main`, `#235.core` may begin from the new main SHA. It does not wait for complete #266 programme closure.

---

## 12. Current merge-queue cleanup

The repository currently contains many open PRs whose own bodies say they are historical, tracking-only or never mergeable. They obscure the actual implementation queue.

Perform a controlled cleanup:

1. inventory every open PR;
2. mark the active code candidate(s);
3. ensure each historical packet's obligations have current issue/doc owners;
4. add one closure comment;
5. close the non-mergeable PR;
6. preserve its branch and discussion as history.

After cleanup, `is:pr is:open` must answer: **what can actually merge?**

---

## 13. Process acceptance

This protocol is effective when:

- active source work has a frozen scope and one causal result;
- follow-up findings no longer widen the PR by default;
- broad baseline debt is visible but does not block unrelated safe slices;
- exact-head review occurs before merge;
- source delivery and programme closure are distinct;
- the single writer advances through frequent bounded merges;
- the open PR queue contains merge candidates rather than historical planning artifacts;
- no product invariant, qualification threshold or authority boundary is weakened to gain throughput.
