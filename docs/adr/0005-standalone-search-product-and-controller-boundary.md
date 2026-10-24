# ADR 0005 — Standalone Search product and controller boundary

- **Status:** accepted
- **Date:** 2026-10-03; revised 2026-10-22
- **Scope:** product ownership and optional ELIOT integration

## Context

ELIOT Search must work as a local search product. Earlier repository coordination material expanded into tickets, leases, approval receipts and agent-control state inside Search. That duplicated external orchestration products and distracted from the working path:

```text
selected source → retained bytes → preparation → Qdrant → retained evidence → CLI
```

## Decision

1. `eliot-searchd` and `eliot-search` must be useful without Memory OS, a Governor or an agent controller.
2. Qdrant is the only indexed backend. redb stores small current or unfinished technical state; retained source and derived evidence remain outside Qdrant.
3. ELIOT integration is an optional leaf adapter over the same Search handlers. It cannot create a second source, access, publication or query authority.
4. Search does not own generic tasks, attempts, agents, schedulers, mailboxes, writer leases, review acceptance or finish authority.
5. Repository work is coordinated by a maintainer request or issue, one non-overlapping branch or worktree, commits and review. Git history is the archive; no ticket or receipt control plane is required.
6. Compiler output, behavior checks and executed runtime experiments are evidence. Planning documents, source presence, past probe reports and generated status files are not constructor tokens or readiness credentials.
7. Boundary violations are removed rather than completed merely because their internal code is consistent.

## Consequences

- Search can be released and operated independently.
- External orchestration can call Search without importing Search internals.
- Current documentation describes product behavior and selected mechanisms, not agent administration.
- Optional adapters fail independently and never block the baseline local path.
- Product claims still require direct executed evidence, but that evidence is not a runtime approval database.
