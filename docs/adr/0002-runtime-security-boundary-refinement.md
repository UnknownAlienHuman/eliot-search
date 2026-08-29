# ADR 0002 — Refine runtime, security and lifecycle ownership before implementation

- **Status:** accepted
- **Date:** 2026-08-29
- **Scope:** implementation packaging and dependency direction only
- **Architecture:** ELIOT Search 8.4, especially S5, S16.6, S26-S28, S31, H4, H8, H12-H16
- **Supersedes:** no architecture decision; refines ADR 0001

## Context

The first one-agent-per-crate scaffold mapped C00-C30 but left five load-bearing owners absent or
hidden inside packages with different security, platform or lifecycle seams:

1. OS-bound secret storage was implicit in daemon/Qdrant/provider code.
2. Qdrant artifact/process containment and Qdrant data-plane translation were one implementation task.
3. C17 produced pin watermarks but no owner executed ordinary retired-point deletion.
4. `expand_handle@1`, source-handle records, TTL, durable eligibility and expansion authorization had no mutable-state owner.
5. Source-admission policy was evaluated partly by the registry and partly by the safe reader.

Several query/lifecycle packages also depended directly on concrete redb/Qdrant/revision adapters,
contradicting the vendor-neutral port boundary.

## Decision

Add five focused support packages:

- `search-os-secrets`
- `search-source-admission`
- `search-qdrant-supervisor`
- `search-index-reclaimer`
- `search-handles`

Refine existing owners:

- `search-qdrant-bridge` owns only schema/capability/data-plane translation.
- `search-safe-reader` acquires stable bytes but does not decide admission policy.
- `search-source-registry` stores policy bindings and verified admission receipts.
- `search-publication` emits committed retired manifests; it does not delete points.
- `search-result-projector` requests handles; it does not store or authorize them.
- `search-retention` owns CAS retention/purge/restore policy, not ordinary index reclaim or handle state.
- query/lifecycle orchestration consumes vendor-neutral ports rather than concrete adapters.

The workspace becomes 39 library packages plus 4 binaries. Architecture 8.4 and its embedded SHA-256
remain unchanged because product authority, wire semantics and storage topology are unchanged.

## Consequences

- One agent owns each missing security/failure state.
- Qdrant Windows containment can be tested independently from vendor RPC semantics.
- Admission decisions are deterministic and reusable without performing I/O.
- Retired-point reclamation cannot be mistaken for publication cleanup or legal purge.
- Handle expansion/revocation has one state owner.
- Concrete adapters are composed only by `eliot-searchd`.

## Explicit non-splits

`search-publication`, `search-exact`, `search-retention` and the baseline `search-safe-reader` remain
unified while each has one causal owner. Their assignments contain mandatory split triggers before the
line limit or before an independently replaceable provider enters the package.
