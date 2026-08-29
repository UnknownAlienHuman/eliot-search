# AGENTS.md — ELIOT Search swarm contract

This repository is implementation-scaffolded, not implemented. The authoritative architecture remains
`docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`, but ordinary package agents MUST NOT load
that 145 KB master. The required implementation slice is copied into the nearest `AGENTS.md`.

## Read set

A writer reads only:

1. this file;
2. the nearest family `AGENTS.md`, when present;
3. its package `AGENTS.md`;
4. README/API documentation of direct dependencies;
5. the package's assigned issue and accepted dependency handoffs.

Load the architecture master only when these sources contain a demonstrable contradiction or omit a
load-bearing field. Record the exact section and proposed resolution in a contract-change request.

## Write ownership

- One writer agent owns exactly one Cargo package and one isolated worktree.
- The writer edits only that package directory.
- Root `Cargo.toml`, `Cargo.lock`, toolchain, CI, `docs/generated/`, `swarm/` and cross-package changes
  belong to the integration owner.
- A read-only reviewer may inspect one package but does not broaden its scope.
- Package agents never opportunistically repair a dependency. They submit a typed contract request.
- Shared control-corpus fixtures are owned by `search-eval`; other packages request fixture additions.

## Global invariants

1. Qdrant is the only search/index database. redb is a bounded control journal and never a searchable corpus.
2. Original bytes or an immutable admitted revision are source truth. Qdrant payload text is never evidence.
3. Retrieval proposes candidates; clients own interpretation and admission.
4. One point belongs to exactly one `ProjectionMembership`. Membership arrays are forbidden.
5. Access/currentness filters apply before candidate generation and IDF statistics.
6. Indexed top-k never narrows an exact-proof denominator.
7. Restrictive access and purge fences override query snapshots immediately.
8. An uncommitted epoch is never current; an epoch number is never reused.
9. Publication is globally serialized and acknowledged/read back before the control-journal commit.
10. Unsaved bytes are memory-only until explicit snapshot admission.
11. Vendor types cannot cross public package ports.
12. Optional model/document work is blocked until P15 product acceptance and an ADR.

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

Capability packages depend on ports and contracts, not on concrete redb/Qdrant/process implementations.
The CLI, client adapters and workers never open redb, CAS or Qdrant directly. No dependency cycle or
reverse authority edge is acceptable.

## Size and split rule

- Normal target: `src/` below 7,500 hand-written lines.
- Package-specific soft targets are in each package `AGENTS.md`.
- Hard review threshold: 10,000 hand-written Rust lines including package-local tests.
- Generated or vendored code does not justify a larger package and must not hide ownership.
- Split only on a real dependency, replacement, test, security, runtime or agent-context boundary.
- Never create a forwarding-only crate or crate-per-type shell.

## Implementation protocol

- Start only the wave authorized in `docs/handoff/IMPLEMENTATION_WAVES.md`.
- Public interfaces use `search-contracts` types or package-owned opaque types; no vendor structs.
- Start with failing contract/property tests, then implement the smallest behavior that closes them.
- Windows is the first qualified runtime. Keep ports portable where semantics are platform-neutral.
- No Python or Node production dependency in the baseline.
- No wildcard or unpinned git dependency. A new vendor/artifact requires an ADR and exact qualification.
- No `todo!()`, placeholder success, silent fallback, fake receipt or fake green acceptance.
- Every degraded path returns a typed reason and truthful coverage.
- Preserve raw command output in the package handoff; compilation alone is insufficient.

## Handoff

Use `swarm/PACKAGE_HANDOFF_TEMPLATE.md`. The handoff reports changed files, public surface, invariant
tests, command outputs, reason-code behavior, dependency changes, residual risks and exact contract
requests. Review follows `swarm/REVIEW_CHECKLIST.md`.
