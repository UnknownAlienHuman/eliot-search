# `search-source-registry` implementation packet

**Path:** `crates/search-source/search-source-registry`  
**Capability:** C03  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until search-source-identity handoff is accepted  
**Trace:** S7.2.1, S7.4, S7.7-S7.8, S16.5-S16.6, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-source-identity`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Own admitted roots, source bindings, memberships, reference portfolios, source views and namespace ownership transitions.

## Owns

- root registration and source admission records
- SourceMembership and portfolio revisions
- SourceView and WorkspaceViewRevision resolution
- source-namespace owner state and cutover receipt verification

## Must not own

- safe byte acquisition, materialization or indexing
- client authority or grant decisions
- a second mutable source catalogue
- implicit nearest-repository, HEAD or disk-wide scope

## Logical primitives

- RootRegistration, SourceAdmissionDecision, RegistrySnapshot, ReferencePortfolioRevision, SourceViewResolution, NamespaceOwnershipCommand, CutoverPreparation, CutoverVerification

## Logical operations

1. `register_root(request, policy) -> Result<RootRegistration, RegistryError>`
2. `admit_source(identity, root, admission_policy) -> SourceAdmissionDecision`
3. `bind_membership(source, corpus, role, policies) -> Result<SourceMembership, RegistryError>`
4. `resolve_source_view(request, registry_snapshot) -> Result<ResolvedSourceView, RegistryError>`
5. `transition_namespace_owner(state, command) -> Result<SourceNamespaceOwnership, RegistryError>`
6. `verify_cutover_receipt(receipt, old_state, new_state) -> Result<CutoverReceipt, RegistryError>`

## Required invariants

- one admitted namespace has one active mutable owner
- old owner is fenced before new owner activation
- source-owner generation changes on fence/activation/incarnation replacement
- portfolio selection is explicit and versioned
- one compound query resolves one coherent source/workspace view
- source admission policy is checked before materialization/indexing

## Typed failure surface

- `SOURCE_NAMESPACE_OWNERSHIP_CONFLICT`
- `SOURCE_OWNER_CUTOVER_REQUIRED`
- `REFERENCE_SCOPE_EMPTY`
- `SOURCE_VIEW_AMBIGUOUS`
- `SOURCE_NOT_ADMITTED`
- `CUTOVER_RECEIPT_MISMATCH`

## Exit tests / evidence

- `dual_active_owner_rejected`
- `cutover_state_machine_and_wire_digest_fixture`
- `empty_reference_portfolio_reason`
- `source_view_never_implicit`
- `policy_denied_source_not_bound`
- `branch_or_index_change_creates_new_workspace_view_revision`

## Suggested internal modules

```text
search-source-registry/src/
  roots.rs
  admission.rs
  membership.rs
  portfolio.rs
  view.rs
  ownership.rs
  cutover.rs
  snapshot.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Ownership/cutover may become a separate crate only if its protocol/runtime lifecycle proves independently replaceable; do not split simple registry records.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
