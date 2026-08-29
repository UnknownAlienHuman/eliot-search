# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. The authoritative architecture remains
`docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`, but ordinary package agents MUST NOT load
that 145 KB master. Each writer receives a bounded assignment under `swarm/assignments/`.

## Authoritative read set

A package writer reads only:

1. this file;
2. the nearest family `AGENTS.md`, when present;
3. its package `AGENTS.md`;
4. `swarm/ASSIGNMENT_PROTOCOL.md`;
5. `swarm/assignments/<package-name>.md`;
6. accepted public API/handoff receipts of direct dependencies;
7. the immutable assignment issue/base commit and package-local fixtures.

The architecture master is exception-only. Open it only after demonstrating that the bounded packet and
accepted dependency contracts contradict each other or omit a load-bearing field. Submit the exact
section and proposed resolution through `swarm/CONTRACT_CHANGE_TEMPLATE.md`.

## Launch authority

`swarm/launch-state.toml` is the only current launch authority. A package being present in Cargo,
`swarm/crates.toml`, a README or a future wave does not authorize implementation.

- Start only the active wave.
- Start a package only after every direct dependency handoff is accepted.
- Optional model/document work remains physically blocked until the stated P15 decision and ADR.
- A package may reappear in a later wave for hardening, but never has two concurrent writers.

## Write ownership

- One writer agent owns exactly one Cargo package and one isolated worktree.
- The writer edits only that package directory.
- Root `Cargo.toml`, `Cargo.lock`, toolchain, CI, `docs/generated/`, `swarm/`, architecture, shared
  fixtures and cross-package changes belong to the integration owner.
- A read-only package reviewer does not broaden write scope.
- Package agents never opportunistically repair a dependency. They submit a typed contract request.
- Shared fixtures follow `tests/CRATE_FIXTURE_OWNERS.md`.

## Global invariants

1. Qdrant is the only search/index database. redb is a bounded control journal and never a searchable corpus.
2. Original bytes or an immutable admitted revision are source truth. Qdrant payload text is never evidence.
3. Retrieval proposes candidates; clients own interpretation and admission.
4. One point belongs to exactly one `ProjectionMembership`. Membership arrays are forbidden.
5. Access/currentness filters apply before candidate generation, IDF, facets, counts and traces.
6. Indexed top-k never narrows an exact-proof denominator.
7. Restrictive access and purge fences override query snapshots immediately.
8. An uncommitted epoch is never current; an epoch number is never reused.
9. Publication is globally serialized and acknowledged/read back before the control-journal commit.
10. Unsaved bytes are memory-only until explicit snapshot admission.
11. Vendor types cannot cross public package ports.
12. Optional model/document work is blocked until P15 product acceptance and an ADR.
13. A workspace is never called current across an unresolved observation gap.
14. A source namespace has exactly one active mutable identity/revision owner.
15. Partial or degraded behavior is returned with typed coverage/reasons; it is never relabeled success.

## Dependency direction

```text
search-contracts
    ↑
search-domain and vendor-neutral capability packages
    ↑
storage/index/transport adapters and bounded orchestration
    ↑
runtime composition and binaries
```

Capability packages consume accepted ports and contracts. The CLI, client adapters and workers never
open redb, CAS or Qdrant directly. No dependency cycle, vendor-type leak or reverse authority edge is
acceptable.

## Size and split rule

- Ordinary implementation target: `src/` at or below 7,500 hand-written lines.
- A package assignment may set a smaller target.
- Mandatory design/split review occurs before 8,500 total hand-written Rust lines.
- Hard stop: 10,000 hand-written Rust lines including package-local tests.
- Generated or vendored code does not justify a larger package and must not hide ownership.
- Split only on a real dependency, replacement, test, security, runtime or agent-context boundary.
- Never create a forwarding-only crate or crate-per-type shell.

## Implementation protocol

- Start with failing contract/property/fault tests, then implement the smallest behavior that closes them.
- Public interfaces use `search-contracts` types or package-owned opaque types; no vendor structs.
- Windows x64 is the first qualified runtime. Keep ports portable where semantics are platform-neutral.
- No Python or Node production dependency in the baseline.
- No wildcard or floating git dependency. New vendor artifacts require exact qualification and an ADR
  when load-bearing.
- No `todo!()`, placeholder success, silent fallback, fake receipt or fake green acceptance.
- Every degraded path returns a typed reason and truthful coverage.
- Preserve raw command output and exact artifact/environment identity in the package handoff.

## Integration owner

The non-package integration role is defined in `swarm/INTEGRATION_OWNER.md`. It alone may pin the
toolchain, update root dependency policy, generate `Cargo.lock`, change the workspace graph, publish
generated schemas, enable CI, advance launch state and merge accepted package handoffs.

## Handoff

Use `swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Review follows `swarm/REVIEW_CHECKLIST.md`. The completed
handoff must let downstream agents consume the public package contract without reading its internals or
the architecture master.
