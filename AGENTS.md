# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. Architecture Part I remains normative;
ordinary agents use bounded packets and accepted handoffs instead of loading the 145 KB master.

## Read set

A writer reads only:

1. this file and nearest family/package `AGENTS.md`;
2. `docs/handoff/AUTHORITY_MAP.md`;
3. its exact package entry in `swarm/crates.toml`;
4. `swarm/ASSIGNMENT_PROTOCOL.md` and one package assignment;
5. relevant port-catalog entries;
6. accepted public dependency handoffs/API digests;
7. immutable assignment issue/base commit and local fixtures.

W0 writers additionally read their assigned P00 contract-pack files. The architecture master is
exception-only: a demonstrated contradiction or missing load-bearing field stops work and uses
`CONTRACT_CHANGE_TEMPLATE.md`.

## Exact dependencies and launch

- `swarm/crates.toml` is the only exact dependency/path/assignment registry.
- Dependency prose in package instructions is explanatory and cannot override the registry.
- Cargo manifest and registry dependency sets must match before merge.
- `swarm/launch-state.toml` alone decides whether a package may run now.
- Presence in Cargo, a future wave, README or assignment is not authorization.

## Write ownership

- one writer, one Cargo package, one isolated worktree;
- writer edits only its package path;
- root Cargo/lockfile/toolchain/CI, architecture, contract pack, generated schemas, `swarm/`, shared
  fixtures and cross-package changes belong to the integration owner;
- package agents do not repair/redefine dependencies; they request a contract/port change.

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

## Layer ownership

```text
search-contracts  shared records, IDs, schemas and reason registries
  ├─ search-domain  pure meaning
  └─ search-ports   shared vendor-neutral operations
       ↑ capabilities and adapters
       ↑ eliot-searchd composition
```

Vendor/native types, credentials, raw collection names, point IDs and generic vendor strings do not
cross public boundaries. Concrete adapters are constructed only by daemon composition.

## Size and implementation rules

- normal target ≤7,500 hand-written `src/` lines;
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

## Handoff

Use `PACKAGE_HANDOFF_TEMPLATE.md`; review follows `REVIEW_CHECKLIST.md`. Publish an immutable public
API/port digest sufficient for downstream work without implementation internals or the architecture
master.
