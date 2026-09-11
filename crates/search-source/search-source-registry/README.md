# search-source-registry

**C03 — Source Registry.**

Own roots, memberships, reference portfolios, coherent source/workspace views
and namespace ownership. Admission rules are evaluated by
`search-source-admission`; this crate verifies and stores receipts.

## Owns

- root registration and policy binding
- `SourceMembership` and `ReferencePortfolio` lifecycle
- `SourceView` / `WorkspaceViewRevision` resolution
- source-owner/cutover state
- deterministic schema, replay validation and append planning for the legacy
  DIRECT source-event journal while migration remains incomplete

The legacy journal compatibility model is pure and filesystem-free.
`eliot-searchd` supplies the qualified digest primitive and owns data-root
locking, safe reads, durable append, exact readback and quarantine. This keeps
daemon composition from becoming a second source-registry implementation.

## Must not own

- byte acquisition or identity derivation
- admission-rule implementation
- access authority, ranking or Qdrant transport
- concrete redb access
- data-root filesystem mutation or journal file publication

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
