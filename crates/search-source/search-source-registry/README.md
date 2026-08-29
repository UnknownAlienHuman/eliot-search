# search-source-registry

**C03 — Source registry and scope resolution.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own admitted roots, source memberships, reference portfolios and coherent SourceView/WorkspaceViewRevision resolution.

## Owns

- root registration and admission-policy binding
- SourceMembership lifecycle
- ReferencePortfolio revisions and precedence
- SourceView and WorkspaceViewRevision resolution
- opaque membership metadata for authorized projection

## Must not own

- physical/logical identity derivation
- filesystem reads
- access authorization decisions
- ranking or Qdrant transport
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
