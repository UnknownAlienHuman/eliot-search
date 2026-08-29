# Agent contract — search-source-registry

You own only `crates/search-source/search-source-registry/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.2, S7.4, S7.7-S7.8, S16.5-S16.6, S19.3, H3.3, P03.

## Mission

Own admitted roots, source memberships, reference portfolios and coherent SourceView/WorkspaceViewRevision resolution.

## Ownership

- root registration and admission-policy binding
- SourceMembership lifecycle
- ReferencePortfolio revisions and precedence
- SourceView and WorkspaceViewRevision resolution
- opaque membership metadata for authorized projection

## Forbidden ownership

- physical/logical identity derivation
- filesystem reads
- access authorization decisions
- ranking or Qdrant transport
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

## Allowed dependencies

`search-contracts`, `search-domain`, `search-source-identity`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `register_root(command) -> Result<RootRegistration, RegistryError>`
- `admit_membership(command) -> Result<SourceMembership, RegistryError>`
- `revise_reference_portfolio(command) -> Result<ReferencePortfolioRevision, RegistryError>`
- `resolve_source_view(request, snapshot) -> Result<ResolvedSourceView, RegistryError>`
- `resolve_workspace_view(workspace, snapshot) -> Result<WorkspaceViewRevision, RegistryError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `REFERENCE_SCOPE_EMPTY`, `SOURCE_ADMISSION_DENIED`, `WORKSPACE_VIEW_DRIFT`, `SOURCE_VIEW_UNAVAILABLE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `membership is separate from SourceIdentity`
- `reference scope requires explicit immutable portfolio revision`
- `empty portfolio returns REFERENCE_SCOPE_EMPTY`
- `branch/index/overlay drift creates a new workspace view revision`
- `source admission exclusions are enforced before preparation`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
