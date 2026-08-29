# ADR 0003 — Executable swarm orchestration contract

- **Status:** accepted
- **Date:** 2026-08-29
- **Scope:** implementation orchestration and evidence control only
- **Architecture:** ELIOT Search 8.4
- **Supersedes:** no product or capability decision

## Context

The repository already has one-agent-per-package ownership, bounded assignments, dependency waves and
an active launch gate. Those artifacts still leave several orchestration decisions implicit:

- when a package is actually ready rather than merely present in Cargo;
- which base commit, assignment revision and dependency API digests a writer received;
- how one active writer is enforced without relying on chat history;
- how a package handoff becomes immutable accepted input for downstream agents;
- which raw evidence is required before a wave can advance;
- how stale, rejected or superseded work is recorded without mutating an accepted receipt.

Without a single state machine, two orchestrators could launch the same package from different bases,
consume unreviewed dependency APIs, or treat a successful compile as a gate receipt.

## Decision

1. `swarm/orchestration.toml` is the machine-readable assignment and handoff state-machine contract.
2. `swarm/gates.toml` is the machine-readable gate/evidence registry. It defines evidence IDs; it does
   not claim that any evidence has been executed.
3. `swarm/launch-state.toml` remains the sole current launch authority and points to both registries.
4. Every writer starts from an integration-owned assignment ticket binding:
   - package and exact write scope;
   - base commit;
   - assignment path and digest;
   - accepted dependency handoff/API digests;
   - wave/stage and feature profile;
   - one unique lease ID.
5. One package has at most one active writer lease. A second writer is blocked until the first lease is
   accepted, rejected, revoked or superseded by an integration-owner receipt.
6. A package handoff is accepted only after review reproduces its declared tests, validates ownership
   and dependency direction, and publishes an immutable API/schema digest.
7. Downstream work consumes accepted handoffs by exact commit and API digest. It never consumes a
   mutable branch head or an implementation worktree.
8. A wave advances only through an integration-owned wave receipt containing every required package
   handoff and every required gate-evidence reference. Missing or unavailable evidence remains explicit.
9. Accepted receipts are append-only. A corrected API, package or wave produces a new receipt that
   supersedes the old one; history is not rewritten.
10. Product code, vendor selection and runtime claims remain outside this ADR.

## Consequences

- The swarm can be launched reproducibly by another orchestrator without reading prior chat history.
- Package writers receive a bounded context with immutable dependency surfaces.
- Concurrent duplicate ownership and stale-base merges become mechanically rejectable.
- Gate advancement is evidence-based rather than prose-based.
- The repository gains more control metadata, but no runtime implementation or new authority surface.

## Rejected alternatives

- **Use Cargo membership as readiness:** Cargo expresses build topology, not authorization or accepted
  dependency state.
- **Use GitHub issues alone:** issue state does not canonically bind API digests, gate evidence and
  dependency receipts.
- **Let writers update launch state:** this allows self-authorization and cross-package writes.
- **Rewrite accepted handoffs in place:** downstream builds would become irreproducible.
- **Create a new service/database for orchestration:** unnecessary before the repository metadata model
  is proven; Git commits and append-only receipts are sufficient.
