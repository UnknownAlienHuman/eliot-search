# `search-source-registry` implementation packet

**Path:** `crates/search-source/search-source-registry`  
**Capability:** C03  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until `search-source-identity` and `search-source-admission` handoffs are accepted  
**Trace:** S7.2.1, S7.4, S7.7-S7.8, S16.5-S16.6, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-source-identity`, `search-source-admission`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Own admitted roots, source bindings, memberships, reference portfolios, source views and namespace ownership transitions while persisting—not reimplementing—source-admission decisions.

## Owns

- root registration and binding to a versioned admission-policy fingerprint
- verification/persistence of `AdmissionReceipt` before membership creation
- SourceMembership and portfolio revisions
- SourceView and WorkspaceViewRevision resolution
- source-namespace owner state and cutover receipt verification

## Must not own

- filesystem/Git byte acquisition
- implementing or weakening source-admission rules
- materialization/indexing or client grant decisions
- a second mutable source catalogue
- implicit nearest-repository, HEAD or disk-wide scope
- concrete redb dependency; persistence is through a vendor-neutral port

## Logical primitives

- `RootRegistration`, `AdmissionReceiptRef`, `RegistrySnapshot`, `ReferencePortfolioRevision`, `SourceViewResolution`, `NamespaceOwnershipCommand`, `CutoverPreparation`, `CutoverVerification`

## Logical operations

1. `register_root(request, policy_fingerprint) -> Result<RootRegistration, RegistryError>`
2. `admit_source(identity, root, admission_receipt) -> Result<AdmittedSource, RegistryError>`
3. `bind_membership(source, corpus, role, policies) -> Result<SourceMembership, RegistryError>`
4. `resolve_source_view(request, snapshot) -> Result<ResolvedSourceView, RegistryError>`
5. `transition_namespace_owner(state, command) -> Result<SourceNamespaceOwnership, RegistryError>`
6. `verify_cutover_receipt(receipt, old_state, new_state) -> Result<CutoverReceipt, RegistryError>`

## Required invariants

- one admitted namespace has one active mutable owner
- old owner is fenced before new owner activation
- source-owner generation changes on fence/activation/incarnation replacement
- membership creation requires a matching current policy/observation receipt
- portfolio selection is explicit and versioned
- one compound query resolves one coherent source/workspace view

## Typed failure surface

- `SOURCE_NAMESPACE_OWNERSHIP_CONFLICT`
- `SOURCE_OWNER_CUTOVER_REQUIRED`
- `REFERENCE_SCOPE_EMPTY`
- `SOURCE_VIEW_AMBIGUOUS`
- `SOURCE_NOT_ADMITTED`
- `ADMISSION_RECEIPT_STALE`
- `CUTOVER_RECEIPT_MISMATCH`

## Exit tests / evidence

- `dual_active_owner_rejected`
- `cutover_state_machine_and_wire_digest_fixture`
- `membership_requires_matching_admission_receipt`
- `registry_cannot_weaken_admission_rules`
- `empty_reference_portfolio_reason`
- `source_view_never_implicit`
- `branch_or_index_change_creates_new_workspace_view_revision`

## Suggested internal modules

```text
search-source-registry/src/
  roots.rs
  admission_receipt.rs
  membership.rs
  portfolio.rs
  view.rs
  ownership.rs
  cutover.rs
  snapshot.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Ownership/cutover may split only after an independently replaceable protocol/runtime boundary is proven.
