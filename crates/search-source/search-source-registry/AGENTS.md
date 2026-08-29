# Agent contract — search-source-registry

You own only `crates/search-source/search-source-registry/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S7.2, S7.4, S7.7-S7.8, S16.5-S16.6, S19.3, H3.3, P03.

## Mission

Own admitted roots, source memberships, reference portfolios and coherent SourceView /
WorkspaceViewRevision resolution while delegating policy evaluation to one admission owner.

## Ownership

- root registration and binding to a versioned admission-policy fingerprint
- persistence/validation of `AdmissionReceipt` before membership creation
- SourceMembership lifecycle
- ReferencePortfolio revisions and precedence
- SourceView and WorkspaceViewRevision resolution
- opaque membership metadata for authorized projection

## Forbidden ownership

- physical/logical identity derivation
- filesystem/Git reads
- implementing source-admission rules
- access authorization decisions
- ranking or Qdrant transport
- depending on concrete redb; durable state is reached through a vendor-neutral port

## Allowed dependencies

`search-contracts`, `search-domain`, `search-source-identity`, `search-source-admission`.
The registry may call the pure evaluator or verify its receipt; it cannot fork the rule set.

## Required logical surface

- `register_root(command, policy_fingerprint) -> Result<RootRegistration, RegistryError>`
- `admit_membership(command, admission_receipt) -> Result<SourceMembership, RegistryError>`
- `revise_reference_portfolio(command) -> Result<ReferencePortfolioRevision, RegistryError>`
- `resolve_source_view(request, snapshot) -> Result<ResolvedSourceView, RegistryError>`
- `resolve_workspace_view(workspace, snapshot) -> Result<WorkspaceViewRevision, RegistryError>`
- `verify_admission_receipt(receipt, observation) -> Result<(), RegistryError>`

## Failure surface

Relevant reasons include `REFERENCE_SCOPE_EMPTY`, `SOURCE_ADMISSION_DENIED`,
`ADMISSION_RECEIPT_STALE`, `WORKSPACE_VIEW_DRIFT` and `SOURCE_VIEW_UNAVAILABLE`.

## Test seams and exit evidence

- `membership is separate from SourceIdentity`
- `membership cannot be created without matching policy/observation receipt`
- `reference scope requires explicit immutable portfolio revision`
- `empty portfolio returns REFERENCE_SCOPE_EMPTY`
- `branch/index/overlay drift creates a new workspace view revision`
- `registry cannot reimplement or weaken admission rules`

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**

## Definition of done

Root and membership state has one owner, every admission is receipt-bound, and policy evaluation
remains isolated in `search-source-admission`.
