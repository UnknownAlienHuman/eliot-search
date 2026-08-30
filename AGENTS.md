# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. Architecture Part I remains normative;
ordinary agents use bounded packets and accepted handoffs instead of loading the 145 KB master.

## Read set

A writer reads only:

1. this file and nearest family/package `AGENTS.md`;
2. `docs/handoff/AUTHORITY_MAP.md`;
3. its exact package entry in `swarm/crates.toml`;
4. its exact foundation/function entry in `swarm/function-packets.toml`;
5. its exact current stage entry in `swarm/stages.toml`;
6. when the package is reused after its earliest wave, its one exact override in
   `swarm/stage-readsets.toml`;
7. `swarm/ASSIGNMENT_PROTOCOL.md` and one package assignment;
8. the exact primary contract or package-local `FUNCTIONS.md` declared by the function registry;
9. the current stage `shared_read_set` and only the stage-override supplements/additional files;
10. accepted public dependency and prior-stage handoff/API/configuration/evidence digests named by the
    immutable ticket;
11. named package-local/shared fixture references owned by the applicable qualification registry.

At issuance, these sources are materialized at one exact base commit into the immutable writer-context
artifact bound by the ticket and lease. The writer does not browse the repository to assemble additional
context.

W0 foundation writers consume their exact P00 contract-pack entry rather than a package-local
`FUNCTIONS.md`. For a reused package, accepted public handoffs **replace previous-stage documents**;
prior stage packets and dependency implementation internals are not accumulated into the new context.

The architecture master is exception-only: a demonstrated contradiction or missing load-bearing field
stops work and uses `CONTRACT_CHANGE_TEMPLATE.md`.

## Exact dependencies, functions, stages and launch

- `swarm/crates.toml` is the exact package/path/dependency/earliest-wave/assignment/configuration/
  qualification registry.
- `swarm/function-packets.toml` is the exact primary function/contract packet and package write-scope
  registry for all 45 packages.
- `swarm/stages.toml` is the exact W0–W10 package set, shared stage context and gate/receipt ordering
  registry.
- `swarm/stage-readsets.toml` is the exact replacement-context registry for every package reused after
  its earliest wave.
- `swarm/orchestration.toml` is the exact issued-ticket/context/lease/submission/review/handoff state
  machine.
- Dependency prose in package instructions is explanatory and cannot override any machine registry.
- Cargo manifest and `swarm/crates.toml` dependency sets must match before merge.
- `swarm/launch-state.toml` alone decides whether an **issued ticket** may be considered for readiness.
- Presence in Cargo, a future stage, README, function packet, stage/read-set entry, qualification packet,
  assignment or ticket/context draft is not authorization.
- A package agent never selects an external artifact version or marks a qualification probe `PASS`
  without an integration-owned ticket and immutable executed evidence.

## Draft, ticket and lease rule

Files under:

```text
swarm/ticket-drafts/**
swarm/context-drafts/**
```

are non-claimable preparation only. An agent must not interpret `launch_class = AUTHORIZED` inside a
draft as an issued assignment. A valid implementation start requires all of:

```text
new issued immutable ticket
+ exact materialized context manifest/artifact
+ active writer lease
+ writer acknowledgement
+ launch/prerequisite checks
```

Drafts retain unresolved writer/reviewer/base/context/ticket identities and never create a lease. The
writer cannot edit ticket/context/lease/submission/review/handoff/launch records, add files to an
acknowledged context, substitute a moving branch or self-review.

`search-domain` and `search-ports` remain conditional even though drafts exist; their tickets cannot be
issued before the accepted `search-contracts` package/API handoff is bound.

## Stage-context rule

The integration owner builds one static package context from:

```text
root/package instructions
+ package and function registry entries
+ assignment and primary function/contract
+ current stage shared_read_set
+ one later-stage override when applicable
```

An ordinary package context is capped at sixteen declared source files before canonical materialization.
The sole P00 exception is `search-contracts`: its exact manifest-closed P00 contract pack may contain up
to twenty-four declared source files because one writer owns the shared schema freeze. That exception:

- must equal the exact P00 manifest plus the fixed integration instructions/registry fragments;
- must materialize to exactly one writer-visible artifact;
- may not add ad-hoc architecture, dependency-source or unrelated stage files;
- does not apply to `search-domain`, `search-ports` or any W1+ package.

A ticket may add only bounded exact accepted handoff receipts and named fixture references. It may not add
the architecture master, previous stage packets or another package's source tree.

A package first used at its earliest wave needs no override. A package reused later must have exactly one
`stage.package` override with:

```text
replace_previous_stage_context = true
accepted_prior_stage_handoff_only = true
dependency_implementation_reads_allowed = false
shared_registry_edits_allowed = false
```

Missing or contradictory context stops the ticket. The writer may not widen its own read set.

## Write ownership

- one writer, one Cargo package, one isolated worktree;
- writer edits only the exact `write_scope` from `swarm/function-packets.toml`;
- stage overrides never widen write scope;
- root Cargo/lockfile/toolchain/CI, architecture, contract pack, generated schemas, `swarm/`,
  `config/sections.toml`, qualification registries, shared fixtures and cross-package changes belong to
  the integration owner;
- a package that owns a configuration section implements the section validator/digest/change behavior
  inside the package but does not edit the central registry or another owner's settings;
- package agents do not repair/redefine dependencies or an accepted prior-stage API; they request a
  contract/port/configuration change.

## Global invariants

1. Qdrant is the only search/index database; redb is never a searchable corpus.
2. Original bytes or an immutable admitted revision are source truth.
3. Retrieval proposes candidates; clients own interpretation and admission.
4. One point has one `ProjectionMembership`; membership arrays are forbidden.
5. Access/currentness apply before retrieval, IDF, facets, counts and traces.
6. Indexed top-k never narrows an exact-proof denominator.
7. Restrictive access and purge fences override snapshots immediately.
8. Uncommitted epochs are never current and are never reused.
9. Publication is serialized and acknowledged/read back before control commit.
10. Unsaved bytes remain memory-only until explicit admission.
11. A workspace is not current across an observation gap.
12. One source namespace has one active mutable identity/revision owner.
13. Possessing a handle never grants access; expansion reauthorizes live state.
14. Ordinary retired-point reclaim and security/legal purge have separate owners and receipts.
15. Partial/degraded outcomes remain typed data and are never relabeled success.
16. A configuration snapshot becomes effective only after every required live/barrier/restart/rebuild/
    generation/gate receipt succeeds; mixed partial configuration is never published.
17. Indexed mode requires one exact qualified server/client/artifact/profile set; automatic upgrade or
    silent provider switching is forbidden.
18. Mutation timeout/cancellation after a possible external write is `OUTCOME_UNKNOWN` until exact
    readback/recovery resolves it.
19. Paths are locators, not source identity; final opened handle/object must remain inside an admitted
    root before bytes can be accepted.
20. Source admission, identity, registry, reads, revision storage, materialization and unitization retain
    separate owners and immutable handoffs.
21. A later-stage package consumes the accepted prior public API/configuration/evidence receipt, never a
    replayed earlier implementation packet or dependency internals.
22. W7 lifecycle completion is a separate prerequisite receipt; it is not silently equated with a
    central gate.
23. A package draft, structural validator or review candidate is not an issued assignment, accepted
    handoff, gate or wave receipt.

## Layer ownership

```text
search-contracts  shared records, IDs, schemas and reason registries
  ├─ search-domain  pure meaning
  ├─ search-ports   shared vendor-neutral operations
  └─ search-config  pure configuration mechanics
       ↑ capability-owned settings and behavior
       ↑ concrete adapters
       ↑ eliot-searchd composition
```

Vendor/native types, credentials, raw collection names, point IDs and generic vendor strings do not
cross public boundaries. Concrete adapters are constructed only by daemon composition.

## Function contract rule

Every non-foundation package has exactly one primary package-local `FUNCTIONS.md` in
`swarm/function-packets.toml`. The three P00 foundation packages have exact primary contract-pack files.
Each operation defines:

- validated inputs and sole state owner;
- preconditions and successful postconditions;
- typed failures and retryability;
- idempotency/mutation identity;
- cancellation and deadline behavior;
- crash or unknown-outcome recovery;
- finite resource/content/disclosure bounds;
- configuration interaction;
- deterministic, negative, property, fault and qualification fixtures.

The function packet specifies behavior, not mandatory Rust spelling. A later-stage supplement may add
narrow obligations but cannot weaken or replace the accepted base operation contract. A writer may
improve internal module layout but cannot add a second owner, widen context/write boundaries or infer
unspecified behavior from another package's implementation.

## Configuration rule

- `config/sections.toml` names one semantic owner and one packet per section.
- `search-config` parses/layers/redacts/plans but owns no capability setting.
- The section owner supplies compiled defaults, typed validation, digest and change planning.
- Only fields classified `APPLY_LIVE` may use package-local live application.
- Security, restart, rebuild, collection-generation and optional-gate obligations are composed by the
  daemon and may coexist; one dominant enum must not erase required steps.
- Plaintext secrets, automatic artifact download/upgrade and optional-profile self-authorization fail
  closed.

## Size and implementation rules

- normal target ≤7,500 hand-written `src/` lines or the lower package target;
- split review before 8,500 total hand-written lines;
- hard stop at 10,000 including local tests;
- no forwarding-only or crate-per-type shells;
- begin with failing contract/property/fault tests;
- no `todo!()`, fake receipt, placeholder success, silent fallback or unbounded queue;
- Windows x64 is first qualified runtime;
- no wildcard/floating git dependency or baseline Python/Node runtime;
- preserve exact commands, artifacts and unavailable checks in the submission/handoff.

## GitHub connector access

Before claiming GitHub is read-only, reload the full catalog without a query filter, verify push
permission and use an unattached blob probe when needed. VM network and connector API access are
separate.

## GitHub Actions policy

Automatic GitHub Actions runs are disabled.

- Every workflow may use only `on: workflow_dispatch`.
- Never create, restore, enable, or retain `push`, `pull_request`, `pull_request_target`, `merge_group`,
  `schedule`, `workflow_run`, `repository_dispatch`, `workflow_call`, release, issue, discussion,
  branch, tag, package, page-build, status, watch, or any other automatic trigger.
- Never add a temporary, PR-only, audit, export, validation, packaging, merge, or release workflow with
  an automatic trigger.
- Package and integration verification runs locally. A GitHub-hosted workflow runs only after an
  explicit manual dispatch by a person.
- Do not enable CodeQL default setup, Dependabot schedules, Pages builds, release automation, or status
  bots by convention.

## Submission and handoff

The writer submits through `SUBMISSION_TEMPLATE.md`; independent review uses
`REVIEW_RECEIPT_TEMPLATE.md`. An accepted review allows the integration owner to publish an immutable
package/API handoff following `PACKAGE_HANDOFF_TEMPLATE.md` and `REVIEW_CHECKLIST.md`.

A package submission/review cannot self-accept, advance launch state or satisfy a wave/gate receipt.
Published handoffs must give downstream and later-stage work exact API/port/configuration digests without
implementation internals, prior-stage documents or the architecture master.
