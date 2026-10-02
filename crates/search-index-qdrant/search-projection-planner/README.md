# search-projection-planner

**C13 — Exact Qdrant projection planning.**

**Status:** package implementation aligned to S8, S9.5 and S11; downstream publication and daemon
composition remain pending.

The package converts one admitted source-to-projection membership binding and prepared unit/vector
contracts into an exact Qdrant point set and immutable manifest. It performs no vendor I/O.

## Owns

- one-membership projection scope and immutable profile validation;
- exact S9.5 opaque payload construction;
- membership-independent `ScoringDocumentId` derivation;
- planner-owned payload and named-vector BLAKE3 digests;
- S11 identity consumption from `search-point-identity`;
- deterministic exact-ID manifest construction and old/new diff;
- provider-neutral collection schema requirements.

## Must not own

- Qdrant transport, process supervision or qualification receipts;
- source truth, access decisions or publication visibility;
- source membership inside Qdrant payload;
- ACL arrays, display paths, raw source/query text or vendor metadata;
- broad-filter closure when exact manifest IDs exist;
- mutable identity registries or another search database.

## Point boundary

Every point contains exactly one typed `ProjectionMembershipId` and the immutable access/scoring
partition IDs. The source membership remains only in the exact manifest/control mapping for
authoritative post-readback resolution. Policy/scoring changes mint new partition identities and a new
projection publication rather than mutating payload meaning.

Expected payload/vector digests are computed from the exact planner output; callers cannot attach an
unrelated digest. The manifest stores exact UUIDs, full identity digests, unit IDs, vector names/digests
and payload digest. Manifest replacement is exact-ID only.

`ScoringDocumentId` excludes membership. Equivalent copies in separate memberships receive distinct
S11 point IDs while retaining one scoring-document identity for safe duplicate collapse before IDF.

## Dependency boundary

`blake3 = 1.8.2` is implementation-private with pure/std/zeroize features and is tracked in #208. No
BLAKE3 vendor type crosses the public API.

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
