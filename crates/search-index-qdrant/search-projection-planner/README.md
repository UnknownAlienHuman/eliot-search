# search-projection-planner

**C13 — Projection planning.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Plan the exact rebuildable point set and immutable manifests for one projection membership without performing vendor I/O.

## Owns

- projection profile and input-descriptor validation
- one-membership-per-projection enforcement
- point/vector/payload plan construction from prepared contract artifacts
- old/new manifest diff
- expected payload/vector digests

## Must not own

- Qdrant transport
- source truth or access authority
- broad closure filters when exact IDs exist
- sharing retrieval points across memberships
- depending directly on parser, enricher or encoder implementations

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
