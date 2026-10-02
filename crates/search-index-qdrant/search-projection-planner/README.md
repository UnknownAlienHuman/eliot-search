# search-projection-planner

**C13 — Exact Qdrant projection planning.**

**Status:** canonical S9.5/S11 producer implemented; downstream composition and live publication qualification remain separate gates.

This package plans the exact rebuildable point set and immutable manifest for one `ProjectionMembership`. It performs no Qdrant, redb, filesystem, CAS, admission, or access-authority I/O.

## Canonical boundary

- `ProjectionScope` carries the authoritative one-to-one `ProjectionMembership -> SourceMembership` control binding.
- `MinimalPointPayload` is the exact closed S9.5 payload. Source membership, ACLs, paths, source/query text, display names, payload digests, and vector digests are not representable in Qdrant payload.
- `search-point-identity` owns the exact S11.1 canonical key, full BLAKE3-256 digest, and compact Qdrant address.
- `ProjectionDigestPort` supplies standard BLAKE3-256 for versioned canonical payload, vector, and manifest bytes. The planner owns those encodings and expected digests without adding another hashing implementation.
- `ProjectionManifest` contains exact point IDs, full identity digests, unit IDs, expected vector names/digests, payload digests, and the immutable membership/profile scope.
- `diff_manifests` returns exact create/retain/retire point lists. Broad-filter closure is absent.

## Invariants

- one plan has exactly one projection membership and one immutable access/scoring partition;
- every point carries exactly one projection membership;
- every unit-role and scoring-document identity is unique inside the plan;
- every point contains the exact vector set required by the accepted profile set;
- access/scoring policy changes mint new immutable partition/membership identities before planning;
- new points use `valid_from_epoch = N` and no upper bound;
- Qdrant payload is never source evidence.

## Dependencies

Public behavior uses only `search-contracts`, package-owned types, and the pure `search-point-identity` owner. Vendor types and Qdrant SDK dependencies are forbidden.

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
