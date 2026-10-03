# Archived package-assignment protocol

> **Historical only.** This file described a repository-local multi-agent campaign and is superseded as
> implementation authority by ADR 0005 and ADR 0006. ELIOT Search does not require assignment tickets,
> materialized prompt contexts, writer leases, acknowledgements, submission/review records or launch-state
> transitions before code may be changed.

## Current working rule

A maintainer request, issue or PR plus one non-overlapping branch/worktree is sufficient. Read
Architecture Part I, accepted product ADRs, the nearest package instructions and the current issue/PR.
Use one active writer per overlapping package scope and run the smallest applicable Cargo check/Clippy
gate.

Useful ideas retained from the old protocol are ordinary engineering practices:

- exact base revision;
- bounded, non-overlapping write scope;
- explicit dependency/public-contract compatibility;
- one worktree per manager;
- independent review for load-bearing changes;
- exact-head evidence and honest unavailable checks.

They do not require a Search-local controller or persistent actor/lease state.

## No active authority

The following are advisory/historical and cannot authorize or block product work:

```text
swarm/launch-state.toml
swarm/orchestration.toml
swarm/ticket-drafts/**
swarm/context-drafts/**
swarm/tickets/**
swarm/context-manifests/**
swarm/leases/**
swarm/submissions/**
swarm/reviews/**
swarm/handoffs/**
```

Do not create new records in these directories for ordinary Search implementation. Do not port their
state machines into Rust tooling. Residual controller tooling is removed under issue #214.

## Product ownership

- shared Search records and wire schemas: `search-contracts`;
- pure product meaning: `search-domain`;
- vendor-neutral ports: `search-ports`;
- configuration mechanics: `search-config`;
- capability state/behavior: owning product package;
- Qdrant vendor translation: `search-qdrant-bridge` only;
- platform/vendor composition: `eliot-searchd`;
- standalone user surface: `eliot-search`;
- optional external integrations: leaf adapters only.

Generic Tasks, WorkScopes, Attempts, GM/Governor state, mailboxes, schedulers, agent roles, controller
credentials, review acceptance and finish authority belong to `eliot-swarm-controller` / `eliot-memory-os`.

Historical details remain available in Git history. This active file intentionally does not reproduce the
obsolete ticket/lease state machine.
