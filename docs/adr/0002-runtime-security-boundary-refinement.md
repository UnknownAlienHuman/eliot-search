# ADR 0002 — Refine runtime, security and lifecycle ownership before implementation

- **Status:** accepted
- **Date:** 2026-08-28
- **Scope:** implementation packaging and dependency direction only
- **Architecture:** ELIOT Search 8.4, especially S5, S16.6, S17, S26-S28, S31, H4, H8, H12-H16
- **Supersedes:** no architecture decision; refines ADR 0001 package ownership

## Context

ADR 0001 converted the broad architecture families into one-agent/one-package capability crates. A
second pre-implementation audit found five load-bearing owners that were either absent or hidden inside
packages with a different dependency/security boundary:

1. OS-bound secret storage was implicit inside Qdrant/provider process code.
2. Qdrant process qualification/lifecycle and Qdrant data-plane transport were one package despite
   different platform dependencies, credentials and fault tests.
3. C17 exposed an epoch-pin registry but no package owned ordinary retired-point reclamation.
4. Source/result handle creation, expansion, TTL, durable retention eligibility and revocation had no
   state owner; result projection and continuation only emitted or referenced handles.
5. Source admission policy was evaluated partly by the registry and partly by the safe reader.

The audit also found concrete adapter edges from query/lifecycle packages to Qdrant, redb and revision
storage packages. That contradicted the root rule that composition happens in `eliot-searchd` through
vendor-neutral ports.

## Decision

Add five focused support packages:

- `search-os-secrets` — opaque OS-user/incarnation-bound secret references; no plaintext API.
- `search-source-admission` — pure, versioned deny-by-default source-admission evaluation.
- `search-qdrant-supervisor` — exact artifact, process, ACL, loopback, Job Object and restart ownership.
- `search-index-reclaimer` — exact retired-point deletion below route/epoch pin watermarks.
- `search-handles` — ephemeral and durable source/result handle state and expansion authorization.

Refine existing packages:

- `search-qdrant-bridge` owns only the qualified vendor data plane and capability/schema probes.
- `search-safe-reader` owns stable no-execute byte acquisition, not admission policy.
- `search-source-registry` stores policy bindings and admission receipts, but does not implement policy.
- `search-result-projector` requests handles from `search-handles`; it does not store them.
- `search-retention` owns CAS retention/purge/restore coordination, not ordinary retired-point deletion
  or handle storage.
- publication, execution, exact and validation packages consume Search ports rather than depending on
  concrete redb/Qdrant/process/revision adapters.

The workspace therefore contains 39 library packages and 4 binary packages. The Architecture 8.4
embedded body and SHA-256 remain unchanged: this ADR changes implementation ownership, not product
authority, wire semantics or invariants.

## Dependency rule

```text
contracts/domain ports
        ↑
pure capability and orchestration packages
        ↑
redb / Qdrant transport / process / OS adapters
        ↑
eliot-searchd composition
```

Horizontal capability dependencies are allowed only when they consume the producer's public,
vendor-neutral contract. A query package cannot depend directly on `search-qdrant-bridge`; lifecycle
packages cannot open redb/CAS/Qdrant themselves.

## Consequences

- Five more agents can work against independent security/fault seams.
- Qdrant transport tests no longer require Windows process-lifecycle fixtures, and vice versa.
- Reclamation cannot be accidentally treated as a publication cleanup detail or as security purge.
- Handle expansion/revocation has one owner and the `expand_handle@1` recipe has an implementation home.
- Admission decisions are deterministic, reusable and testable without performing I/O.
- Cargo has more packages, but no forwarding-only package was introduced.

## Explicit non-splits

The audit does **not** split every possible sub-function:

- `search-exact` retains exact-plan compilation and execution because C20 has one proof owner; split only
  if concrete regex/structural providers or measured line growth create a real boundary.
- `search-retention` retains CAS mark/sweep, purge receipts and restore quarantine because C28 owns one
  monotonic lifecycle policy; ordinary index reclamation alone is separated.
- `search-safe-reader` retains filesystem/Git stable-read semantics for the baseline; backend crates are
  added only when dependencies or line count justify them.
- `search-publication` retains commit and crash recovery in one state-machine owner.

This avoids crate-per-function fragmentation while preserving the under-10k target.
