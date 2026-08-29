# search-source-registry

**C03 — Source Registry.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own roots, memberships, reference portfolios and coherent source/workspace views. Admission policy is
evaluated by `search-source-admission`; this crate stores policy bindings and verified receipts.

## Owns

- root registration and policy binding
- SourceMembership lifecycle
- ReferencePortfolio revisions
- SourceView and WorkspaceViewRevision resolution

## Must not own

- identity derivation or source reads
- admission-rule implementation
- access authority, ranking or Qdrant transport
- concrete redb access

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
