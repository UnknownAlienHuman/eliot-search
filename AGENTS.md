# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. The authoritative architecture remains
`docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`. Ordinary package agents use bounded
assignments and accepted handoffs instead of loading that 145 KB master.

## Authoritative read set

A package writer reads only:

1. this file;
2. nearest family and package `AGENTS.md`;
3. `swarm/ASSIGNMENT_PROTOCOL.md`;
4. exactly one `swarm/assignments/<package>.md`;
5. relevant `docs/handoff/PORT_CATALOG.md` entries;
6. accepted public dependency handoffs/API digests;
7. immutable assignment issue/base commit and package-local fixtures.

W0 writers additionally read the exact files assigned to them in `docs/contracts/p00/README.md`.
The architecture master is exception-only. A demonstrated contradiction or missing load-bearing field
uses `swarm/CONTRACT_CHANGE_TEMPLATE.md` and stops affected work.

## Launch and write ownership

- `swarm/launch-state.toml` is the only current launch authority.
- One writer owns one Cargo package and one isolated worktree.
- Writers edit only their package directory.
- Root workspace, lockfile, toolchain, CI, architecture, contract pack, generated schemas, shared
  fixtures, assignments and launch state belong to the integration owner.
- A package never repairs or redefines a dependency; it requests a contract/port change.

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

## Dependency direction

```text
search-contracts
  ├─ search-domain
  └─ search-ports
       ↑ capability and orchestration packages
       ↑ concrete adapters
       ↑ eliot-searchd composition
```

- Shared records come from `search-contracts`.
- Shared vendor-neutral traits come from `search-ports`.
- Pure reusable meaning comes from `search-domain`.
- Concrete adapters are constructed only by `eliot-searchd`.
- Vendor/native types and generic string errors never cross public boundaries.

## Size and implementation rules

- ordinary target: ≤7,500 hand-written `src/` lines;
- split review before 8,500 total hand-written lines;
- hard stop at 10,000 including local tests;
- no forwarding-only or crate-per-type shells;
- start with failing contract/property/fault tests;
- no `todo!()`, fake receipt, placeholder success, silent fallback or unbounded queue;
- Windows x64 is first qualified runtime;
- no wildcard/floating git dependency or baseline Python/Node runtime;
- preserve exact commands, artifact identities and unavailable checks in the handoff.

## GitHub connector access

Before claiming GitHub is read-only, reload the full catalog with
`list_resources(paths=["GitHub"])` without a query filter, verify repository push permission, and use a
harmless unattached blob probe when necessary. VM network access and connector API access are separate.

## Handoff

Use `swarm/PACKAGE_HANDOFF_TEMPLATE.md`; review follows `swarm/REVIEW_CHECKLIST.md`. The handoff must
publish the public API/port digest and let downstream agents work without reading implementation
internals or the architecture master.
