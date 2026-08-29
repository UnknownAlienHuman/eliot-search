# search-source-registry

**C03 — Source Registry.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own roots, memberships, reference portfolios, coherent source/workspace views and namespace ownership.
Admission rules are evaluated by `search-source-admission`; this crate verifies and stores receipts.

## Owns

- root registration and policy binding
- SourceMembership and ReferencePortfolio lifecycle
- SourceView / WorkspaceViewRevision resolution
- source-owner/cutover state

## Must not own

- byte acquisition or identity derivation
- admission-rule implementation
- access authority, ranking or Qdrant transport
- concrete redb access

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
