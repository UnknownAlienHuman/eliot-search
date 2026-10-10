# ELIOT Search bounded execution protocol

**Authority:** root `AGENTS.md`, Architecture Part I, accepted ADRs, the exact source issue, process issue #350 and programme issue #352.  
**Applies to:** all remaining implementation, migration, integration, qualification and release work.  
**Topology:** one manager, one writer worktree, one dependency/`Cargo.lock` integrator, 5–10 bounded read/research/review subagents.

This protocol fixes an execution defect, not a product invariant. Previous instructions prevented conflicting writers but did not stop one valid source task from absorbing every newly discovered concern. The result was expanding draft PRs, repeated scope amendments, blocked independent work and review that never reached a stable final SHA.

The manager optimizes for **small accepted source increments that delete duplicate authority**, not for completing an entire programme in one branch.

## Current queue authority

The sources of truth have different jobs:

1. Architecture Part I, ADRs and package contracts define product semantics.
2. `PROJECT_COMPLETION.md` is the long-range obligation/dependency graph. Its dated “next task”, base SHA or local stage wording is not the live writer queue.
3. Programme issue #352 and `docs/audit/AGENT_LAUNCH_GATE_2026-10-09.md` define the current writer sequence.
4. The exact active issue/PR and current source define the current slice.

When the long-range map is stale, follow #352, the launch gate and current merged artifacts; update the map separately. Never repeat delivered work because a dated map row still looks active.

---

## 1. Work-item classes

Every active item has exactly one class.

### PROGRAM

A capability or product tracker. It lives as an issue and in the completion map. It may remain open across many source deliveries. It never serves as a code merge base and never accumulates implementation commits.

### SLICE

One causal source result delivered by one mergeable PR. A slice has one primary state/effect owner, explicit narrow adapters, declared reverse consumers, one compatibility/deletion boundary and exact minimum gates.

### FOLLOW-UP

A defect discovered outside the frozen slice. It names the actual owner, affected path/symbol, consequence and dependency edge. Finding a follow-up is progress; absorbing it into the current PR by default is not.

### GATE

Evidence or qualification against exact immutable candidate bytes. A gate does not fix product source. Failures return to source-owner slices and create a new candidate.

**Do not create new programme, tracking or gate pull requests. Open pull requests are merge candidates.**

---

## 2. Definition of Ready

Before source edits, record in the issue or PR body:

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

A slice is not ready when owner, compatibility rule, reverse consumers or minimum gates are still being invented during implementation.

### Path ownership

One slice owns:

- one primary package or one cohesive daemon composition owner;
- at most two narrowly named adapter families;
- only direct reverse consumers required to compile and cut over the result.

A glob such as `bins/eliot-searchd/src/**` is not a useful slice boundary.

### Persisted identities

Before changing persisted bytes, digests, profile identity, schema or generation semantics, declare exactly one disposition:

```text
BYTE_COMPATIBLE
VERSIONED_REBUILD
LEGACY_DECODE_ONLY
UNSUPPORTED_REQUIRES_MIGRATION
```

A later review finding cannot silently change this disposition inside the same PR.

---

## 3. Scope freeze

Publish a `SCOPE_FROZEN` block no later than the second source commit:

```text
exact base SHA
exact head SHA at freeze
allowed production paths
declared public/private API delta
reverse-consumer list
legacy path retained and deletion owner
minimum source gates
out-of-scope owners
```

After `SCOPE_FROZEN`, a comment cannot add another caller family, schema, lifecycle protocol, recovery owner or package.

A necessary scope change requires one of:

1. revert to the last frozen head and redefine the same slice before continuing;
2. finish the coherent frozen result, file the new work under its owner and merge the current PR; or
3. define dependency-ordered tranches, but open and merge them **strictly one at a time**. Extract/open only the first tranche. After it merges, publish the new `main` SHA, create the successor branch from that new `main`, then open the next PR. Stacked unmerged source PRs are forbidden.

Repeated “narrow continuation” comments are not a substitute for scope freeze.

---

## 4. Finding triage

Every new source/review finding receives exactly one disposition.

### B0 — same-slice blocker

Without the fix, the declared result:

- does not compile for a declared immediate reverse consumer;
- mints false authority or accepts caller-shaped proof;
- corrupts or ambiguously rewrites data;
- widens access or disclosure;
- enables an unsafe path that cannot remain fail-closed or unavailable.

A B0 report names exact path, symbol, caller and causal chain. “Related to the programme” is insufficient.

### F1 — same-owner follow-up

The finding belongs to the same package, but the frozen result remains correct while affected behavior stays unavailable, legacy-only or fail-closed. File a follow-up and merge the slice.

### F2 — adjacent-owner follow-up

The finding belongs to another package, state/effect owner, integration phase or qualification lane. File it under that owner. Never edit the adjacent owner merely to keep the programme moving.

### D — baseline/debt

The failure existed on the accepted base and the candidate adds no normalized diagnostic, caller break or behavior regression. Record it once under the debt owner. It is not the slice gate.

### Q — qualification

The finding requests installed/native/live-Qdrant/fault/scale/release evidence. Route it to #215, #264 or the named gate. Do not expand source scope to simulate acceptance.

A finding becomes B0 only when the manager demonstrates why the frozen result cannot safely remain fail-closed, unavailable or legacy-only. The burden is on widening the PR.

---

## 5. Default review budget

These are mandatory split triggers, not permission to omit correctness:

```text
primary owners:              1
narrow adapter families:     <= 2
changed production files:    <= 30
changed production lines:    <= 2,500
persisted migrations:        <= 1
cross-owner cutovers:         <= 1
```

Focused fixtures, exact generated goldens and source-ledger rows are reported separately from production totals.

When a threshold is exceeded:

1. stop adding feature work;
2. publish the exact reason and path groups;
3. select the first dependency-safe tranche only;
4. open, review and merge that tranche;
5. create each successor from the newly merged `main`, never as a stacked PR;
6. obtain review on every final exact head.

A review-budget exception requires written justification accepted before additional source changes. A broad issue does not itself justify a broad PR.

---

## 6. One-manager merge train

One writer remains mandatory. Throughput comes from smaller merges, not overlapping writers.

```text
select dependency-ready slice
→ freeze source scope
→ run bounded subagent research
→ manager implements/integrates
→ compile declared owners and reverse consumers
→ run focused causal fixtures
→ exact-head independent review
→ merge
→ publish new main SHA
→ create next writer branch from new main
```

### Permitted parallelism

While the current PR is in final review, subagents may research the next non-overlapping slice and return read-only maps. They may not create a source branch/PR, mutate shared manifests or assume the current PR will merge unchanged.

### Programme closure is not a merge prerequisite

A broad programme issue remains open after a coherent source slice merges. The next independent owner may begin when its actual API/artifact prerequisite is on `main`; it does not wait for every follow-up, optional profile or final qualification obligation of the preceding programme.

---

## 7. Subagent assignments

Use only roles relevant to the active slice.

| Role | Required output |
|---|---|
| Authority/caller map | exact owner, callers, reverse consumers, duplicate paths |
| Donor/supply-chain | exact source/version/checksum/license/MSRV/features/advisories |
| Compatibility/deletion | persisted-byte disposition and legacy deletion owner |
| Recovery/security red-team | B0 or follow-up findings with exact causal evidence |
| Bounds/allocation | pre-allocation/effect ceilings and deadline/cancellation gaps |
| Focused fixtures | smallest causal cases, no broad test programme |
| Diff/API review | public/private delta, unused compatibility and scope escapes |
| Final exact-head review | blocking findings or scoped `APPROVE_SOURCE` at one SHA |

Rules:

- one pre-code research pass and one final exact-head review pass;
- no repeated full-repository audit after every commit;
- no subagent-created architecture or scope amendment;
- every finding ends with `B0`, `FOLLOW_UP(owner)` or `NO_BLOCKER`;
- duplicate reports are consolidated;
- the manager independently verifies load-bearing claims;
- a later load-bearing commit expires prior review.

---

## 8. Development gates

For a source slice:

1. locked Rust 1.98 check for changed owners;
2. immediate reverse-consumer compilation for changed public APIs;
3. strict Clippy for the same production closure;
4. focused causal fixtures required by the changed boundary;
5. affected repository source guards.

### Baseline failures

Capture one complete baseline diagnostic fingerprint and compare once at final head.

```text
candidate adds diagnostic/signature/caller break → BLOCK
candidate preserves identical unrelated debt     → D follow-up
```

Do not repeatedly repair a known-broken all-target graph inside an unrelated branch. Do not hide new failures behind an old failing aggregate command.

### Deferred gates

Full workspace, installed Windows, real Qdrant, fault injection, scale, resource, disclosure and release evidence remain their explicit late gates. Source presence and compilation do not satisfy them.

---

## 9. Review and PR reporting

The PR body is the single current status record:

```text
frozen scope
exact base/head
changed owner/path groups
delivered causal result
legacy path retained and deletion owner
follow-up issues and disposition
commands, exits and nonzero focused case counts
known baseline debt and candidate delta
evidence boundary
```

Do not add a long status comment after every commit. Use commits for implementation history and update the PR body at stable checkpoints.

Before merge, at least one formal GitHub review targets the final head and verifies the load-bearing invariants. A security-review badge, signature, source-guard PASS or author comment is not independent acceptance.

---

## 10. Closing issues and PRs

### Implementation issue

Close when its declared source slice is merged and residual obligations point to current follow-ups. Do not keep a completed implementation issue open merely because the whole product is unfinished.

### Programme issue

May remain open until selected source/integration/qualification obligations are delivered or explicitly deferred by a maintainer.

### Pull request

Open PR means merge candidate. Historical programme/tracking/gate packet PRs are closed after useful obligations are retained in current issues/docs and no agent is instructed to branch from the old head.

Do not create replacement tracking PRs.

---

## 11. Immediate #266 / #344 application

PR #344 exceeded every default split trigger: 119 changed files, more than 11,000 additions and several owner families. Process issues #349/#350 control its disposition.

Freeze the current branch and derive sequential review units, subject to exact dependency verification:

1. **Root admission and explicit initialization** — typed modes, existing-only open, retained request/deadline/cancellation and required immediate caller cutover.
2. **Focused lifecycle/crash proof** — only fixtures and minimal production seams required to prove tranche 1.
3. **Durable catalog intent and read-only recovery observation** — retained inputs, exact release, named inspection/discovery; no effect reconciliation or cleanup authority.

Open only tranche 1. After its merge, rebase/extract tranche 2 from the new `main`; after tranche 2 merges, create tranche 3 from the next `main`. Do not open stacked PRs.

These remain separate unless a concrete B0 dependency is proved:

```text
#343 redb read-only inspection
#345 control-artifact unknown-outcome staging retention
#346 all-target fixture compilation debt
#347 native original-object unlink and late cleanup
#348 original request → durable effect reconciliation
```

Unsafe or unsupported recovery/cleanup remains unavailable and fail-closed. It need not be falsely completed to merge safe admission.

After the minimum typed root-admission API and required normal callers are on `main`, `#235.core` may begin from the new `main`. It does not wait for complete #266 programme closure.

---

## 12. Merge-queue cleanup

Historical open PRs whose own bodies say “tracking only”, “programme only” or “do not merge” obscure the actual queue. Preserve their useful obligations in current issues/docs, add a closure pointer and close them without merge. Their branches and discussions remain history.

After cleanup, `is:pr is:open` must answer: **what can actually merge?**

---

## 13. Process acceptance

The protocol is effective when:

- active source work has a frozen scope and one causal result;
- follow-up findings no longer widen the PR by default;
- broad baseline debt remains visible but does not block unrelated safe slices;
- exact-head review occurs before merge;
- source delivery and programme closure are distinct;
- every successor branch starts from the newly merged `main`;
- the open PR queue contains merge candidates rather than historical packets;
- no product invariant, qualification threshold or authority boundary is weakened for throughput.
