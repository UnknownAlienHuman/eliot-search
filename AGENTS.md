# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. Architecture Part I remains normative;
ordinary agents use bounded packets and accepted handoffs instead of loading the 145 KB master.

## Read set

A writer reads only:

1. this file and nearest family/package `AGENTS.md`;
2. `docs/handoff/AUTHORITY_MAP.md`;
3. its exact package entry in `swarm/crates.toml`;
4. its exact foundation/function entry in `swarm/function-packets.toml`;
5. `swarm/ASSIGNMENT_PROTOCOL.md` and one package assignment;
6. the exact primary contract or package-local `FUNCTIONS.md` declared by the function registry;
7. every package configuration packet declared by `swarm/crates.toml`, when present;
8. the package qualification/stage packet declared by the registries/ticket, when present;
9. relevant port-catalog entries;
10. accepted public dependency handoffs/API digests;
11. immutable assignment ticket/base commit and named package-local/shared fixtures.

W0 foundation writers consume their exact P00 contract-pack entry rather than a package-local
`FUNCTIONS.md`. The architecture master is exception-only: a demonstrated contradiction or missing
load-bearing field stops work and uses `CONTRACT_CHANGE_TEMPLATE.md`.

## Exact dependencies, functions and launch

- `swarm/crates.toml` is the exact package/path/dependency/wave/assignment/configuration/qualification
  registry.
- `swarm/function-packets.toml` is the exact primary function/contract packet and package write-scope
  registry for all 45 packages.
- Dependency prose in package instructions is explanatory and cannot override either registry.
- Cargo manifest and `swarm/crates.toml` dependency sets must match before merge.
- `swarm/launch-state.toml` alone decides whether a package may run now.
- Presence in Cargo, a future wave, README, function packet, qualification packet or assignment is not
  authorization.
- A package agent never selects an external artifact version or marks a qualification probe `PASS`
  without an integration-owned ticket and immutable executed evidence.

## Write ownership

- one writer, one Cargo package, one isolated worktree;
- writer edits only the exact `write_scope` from `swarm/function-packets.toml`;
- root Cargo/lockfile/toolchain/CI, architecture, contract pack, generated schemas, `swarm/`,
  `config/sections.toml`, qualification registries, shared fixtures and cross-package changes belong to
  the integration owner;
- a package that owns a configuration section implements the section validator/digest/change behavior
  inside its package but does not edit the central registry or another owner's settings;
- package agents do not repair/redefine dependencies; they request a contract/port/configuration change.

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

The function packet specifies behavior, not mandatory Rust spelling. A writer may improve internal
module layout but cannot weaken the operation contract, add a second owner, widen its read/write boundary
or infer unspecified behavior from another package's implementation.

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
- preserve exact commands, artifacts and unavailable checks in the handoff.

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

## Handoff

Use `PACKAGE_HANDOFF_TEMPLATE.md`; review follows `REVIEW_CHECKLIST.md`. Publish an immutable public
API/port/configuration digest sufficient for downstream work without implementation internals or the
architecture master.
