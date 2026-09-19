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
- exact pure schema and codec for the legacy `control/source-roots.v1`
  registration catalog

The legacy compatibility models are pure and filesystem-free.
`eliot-searchd` supplies qualified digest primitives and owns data-root locking,
platform path validation, safe reads, durable publication, exact readback and
crash recovery. For `source-roots.v1`, this crate owns only the frozen header,
line framing, duplicate detection, and finite file/root bounds; locator contents
remain opaque until daemon path qualification. This keeps daemon composition
from becoming a second source-registry schema owner without giving the registry
filesystem authority.

## Must not own

- byte acquisition or identity derivation
- admission-rule implementation
- access authority, ranking or Qdrant transport
- concrete redb access
- data-root filesystem traversal or mutation
- platform path canonicalization, overlap checks or journal/catalog publication

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
