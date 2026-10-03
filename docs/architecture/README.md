# Architecture entry point

The current combined master is:

```text
ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md
```

It contains multiple historical layers in one file. Read them with the following authority boundary.

## Current authority

### Part I — ELIOT Search Architecture 8.4

Part I is the normative product architecture. Its embedded byte range and recorded SHA-256 remain the
source of truth for:

- source/revision/materialization/projection identity;
- Qdrant, redb and immutable-storage responsibilities;
- access, currentness, publication and query invariants;
- portfolios, retrieval, comparison, exact proof, handles and lifecycle;
- standalone/provider behavior and qualification requirements.

### Part II — Codex Handoff 2.7

Part II is historical implementation scaffolding. Its P00-only entry sequence predates the current
workspace and accepted product-boundary corrections. It may explain package intent, but it is **not
current implementation authorization** and cannot require ticket issuance, launch-state, writer leases or
replay of an obsolete wave plan.

When Part II conflicts with current source, an accepted product ADR, accepted public contract or a current
maintainer issue/PR, use the current product source/decision while preserving Part I invariants.

### Audit registers and later appendices

Audit registers record historical findings and intended checks. They are neither runtime behavior nor
proof that a capability has been implemented or qualified.

## Accepted product clarifications

- [ADR 0005](../adr/0005-standalone-search-product-and-controller-boundary.md) — Search is standalone and
  does not own agent orchestration.
- [ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md) — Search is a Qdrant-backed
  large-corpus analysis/navigation framework for agents, not only a locator.

These ADRs clarify implementation ownership and product interpretation without weakening Part I's
source-truth, access, currentness, Qdrant, exact-proof or qualification requirements.

## Status and permanent cleanup

Use the [current implementation status](../product/IMPLEMENTATION_STATUS.md) to distinguish contract,
source presence, integration, compile evidence, qualification and public enablement.

The permanent documentation fix is tracked by #220:

- extract byte-identical Part I into a standalone normative file;
- preserve and verify its recorded SHA-256;
- archive Part II and audit appendices explicitly;
- reconcile package status documents.

Until that work lands, do not quote the combined file's top-level `Codex entry point: P00 only` as a
current instruction.

## Working rule

For implementation work, read:

1. relevant Part I sections;
2. accepted ADRs;
3. the accepted public/package contract;
4. the current issue/PR and compiled source reality;
5. the implementation-status matrix for non-authoritative orientation.

Do not restart from P00 merely because Part II says `Entry point: P00 only`. Do not implement generic
repository ticket/lease/controller machinery in Search. Continue the actual product spine around Qdrant,
source evidence, query analysis and standalone delivery.
