# ELIOT Search — staged completion program

Audited source baseline: `a5abdf7ef0cb9d691000759494fd8829b2ba0b60`, 2026-09-05.

**This is a planning PR, not a product implementation or qualification receipt.** The 43 implementation drafts start as `NOT_STARTED`. No task, package, stage or capability is accepted by publishing this program. The normative Architecture 8.4 Part I remains unchanged.

## Outcome

Deliver one supported Rust daemon/client with immutable source revisions, canonical preparation, redb technical state and Qdrant-only indexed retrieval. DIRECT remains independently available when the index is unavailable, but must never masquerade as indexed success. Do not create a second search database, Python product service, hidden automatic upgrade, fabricated receipt or alternative mutable source owner.

The audit is `AUDIT.md`. The task index is `INDEX.md`; machine dependencies and package locks are `plan.json`. Each task PR contains its own packet under `docs/execution/2026-09-05/tasks/Txx.md`. Read the packet in that PR, not a stale issue or a moving unreviewed dependency branch.

## Milestones and stop conditions

| Milestone | Tasks | Required observable result |
|---|---|---|
| M0 — qualified baseline | T01–T04 | Actual package/target ownership reconciled; legacy targets isolated; stable Windows identity API; executed exact-head build and regression lanes. |
| M1 — safe owner/runtime | T05–T08 | Unknown catalog effects quarantine the runtime; bounded framing/child lifetime; admitted final-handle reads; one durable owner. |
| M2 — authoritative control | T09–T12 | Bounded real redb adapter; verified legacy migration; atomic primary cutover; effective configuration and truthful readiness. |
| M3 — durable DIRECT | T13–T17 | Canonical admission, full-residency immutable CAS, durable representations/units and restart-safe primary DIRECT. |
| M4 — authorized provider | T18–T21 | OS secret leases, canonical daemon/client protocol, live grants and authorized handles/continuations. |
| M5 — live indexed spine | T22–T30 | Exact Qdrant qualification, owned process, real transport, lexical/projection/publication/query/rebuild and executed source-to-Qdrant end-to-end tests. |
| M6 — baseline query breadth | T31–T36 | Live root management/currentness, exact Git sources, ephemeral overlays, frozen-denominator proof and structural/comparison recipes. |
| M7 — lifecycle and release | T37–T43 | Retention/purge/restore, measured leakage/resource bounds, Rust tooling, honest optional boundaries and one tested Windows distribution. |

T17 and T30 are mandatory vertical integration checkpoints. A green pure-model suite cannot substitute for either. T30 is not full release acceptance; T43 includes currentness, lifecycle, privacy and packaging requirements too.

## Execution order and concurrency

Follow `depends_on`, not merely numeric order. T22 can qualify the exact Qdrant artifact after T04 while storage proceeds. T41 can replace tooling after T01/T04. Independent work is permitted only when every dependency is accepted and all touched-package locks are disjoint. Most composition tasks touch `eliot-searchd` and therefore serialize. Do not launch 43 writers simultaneously.

One task, one writer, one worktree. Every touched package has at most one active writer. Integration owns cross-package changes, root manifests/lockfile, qualification and registries. A package writer cannot widen its scope or change a dependency contract. Proposed/new/moved filenames are resolved to an exact bounded context before assignment, using T01's ownership inventory and T02's accepted move map.

All planning branches start from the same audited baseline, with only their distinct task file added. They are not working copies kept current automatically. Before implementation, refresh/rebase on accepted `main`, materialize the exact dependency handoffs and record the new execution base. The baseline in the packet is provenance, not permission to overwrite newer code.

The initial M0 bootstrap must not deadlock on its own known compiler failures. T01–T03 may be independently reviewed with explicit existing-build blockers while their bounded changes are staged by the integration owner; no runtime/gate PASS is issued. M0 is complete only when T04 executes the combined exact head successfully. Outside this recorded bootstrap, a failed required predecessor blocks downstream implementation. Do not fake G0/W0/handoffs to make scheduling convenient.

These planning drafts are not quarantine or active writer leases. Do not open parallel worktrees for them now. On actual task completion, merge with independent review, or quarantine/delete the work branch. Quarantine retains at most five branches, with 24-hour TTL; preserve required evidence in durable reviewed artifacts before deletion. Never begin another task while owning an open worktree.

## Common task contract

Each packet names the observed gap, owner, dependency IDs, write boundary, bounded additional read set, required behavior, discriminating tests, commands and exit condition. Read root/nearest `AGENTS.md`, `docs/handoff/AUTHORITY_MAP.md`, exact registry fragments and accepted public dependency handoffs. The packet is not an issued immutable ticket/context/lease.

Start with failing discriminating tests. Keep existing regressions, especially protected-before-publication writes, no-clobber object storage, raw-plaintext rejection, lost-catalog refusal, monotone snapshots and poisoned-channel rejection. Reuse existing kernels; do not reimplement them solely because an old issue still says scaffold.

A task's changed files must implement its stated causal result, not a broad adjacent refactor. Mechanical extraction precedes behavior changes. If an approved extraction cannot fit package size/review bounds, integration narrows its move map into reviewable commits or a documented successor task before assigning the writer. No vague wildcard permits unrelated changes.

No mock-as-live proof. Native API tests run on native Windows; Qdrant tests run against the exact real artifact. Synthetic codecs and reference models remain useful unit tests, but are labelled as such. No `todo!()`, placeholder success, fabricated IDs/digests, SHA-256 relabelled BLAKE3, skipped required scenarios or hidden fallback.

## Required verification

Run against the actual proposed head with the repository toolchain and unchanged lockfile unless an approved task changes it:

```sh
rustc +1.98.0 -Vv
cargo +1.98.0 -V
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --workspace --all-targets --all-features --locked
cargo +1.98.0 clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.98.0 doc --workspace --all-features --no-deps --locked
```

Set `RUSTDOCFLAGS=-Dwarnings` in the executing shell for the documentation gate. Run the task's explicitly named new negative/property/fault/process scenarios too; the broad commands alone do not prove a gated native or live integration test ran. Optional unavailable capabilities must remain unavailable; enabling `--all-features` does not authorize them. Do not run destructive fixtures outside disposable roots/accounts created by the fixture.

Record base/head SHA, toolchain, target/OS, exact commands and exit codes, named tests and nonzero executed counts, skipped/ignored tests and reasons, changed-path proof, fixture/artifact IDs, package line counts and compatibility impact. Zero tests, missing tools, timeouts and unavailable Windows/Qdrant lanes are not PASS. Do not use an old Actions rerun as evidence for a new head.

GitHub Actions stay `workflow_dispatch` only, read-only against exact SHA. No push/PR/schedule/workflow_run workaround, generated product code or source/lockfile writes in CI. Execution is local or explicitly human-dispatched; this program did not dispatch a run.

## Acceptance and merging

Task PRs initially contain only task text. Keep them draft until the implementation and executed evidence are present. **Never merge a packet-only task PR as completed implementation.** This coordinator may be reviewed as a planning document independently; its merge accepts no product capability.

A distinct reviewer checks the actual proposed head and all mandatory negative/fault cases. Integration accepts the appropriate package/API handoffs, merges the exact reviewed head, verifies post-merge readback and updates the execution index. Dependency release is explicit. Do not let writers self-accept or edit their lease/review/evidence authority.

Allowed task states: `NOT_STARTED`, `BLOCKED`, `IN_PROGRESS`, `REVIEW`, `ACCEPTED`. `ACCEPTED` needs implementation, executed required evidence, independent review and exact merged-head verification. Track failures and unresolved findings in the same task; no success based on file presence or claim wording.

## Legacy assignment reconciliation

T01 crosswalks all open assignments without bulk-closing unfinished work. Examples requiring reconciliation: #23/#24 and historical foundation PRs; #51/#56/#65 configuration duplicates; #53/#59/#79 and #60/#81 obsolete path variants; #70/#85 redb duplicates; #71/#86 owner; #72/#87 secrets; #73/#89 pairing; #74/#90 transport conflict; #83 cipher prescription; #84 normalization prescription; #93 historical run failure. Read the current Part I, actual code and exact evidence before choosing a disposition. Existing issue titles are not design authority.

## Current truth

The audit is static. The audited SHA had zero Actions runs; no Rust compiler was available in the authoring environment. Planning checks validate task coverage, dependency acyclicity, distinct packet paths and conservative package locks only. They are not compiler, runtime, security or release tests. The master release task T43 transitively depends on all other 42 tasks. New findings must be assigned an owner and blocking relation before a milestone advances.
