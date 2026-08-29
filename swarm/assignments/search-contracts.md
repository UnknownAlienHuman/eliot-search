# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only when launch-state active_wave = 0  
**Trace:** S3, S7, S10, S19-S26, S30.3, S32, S34, H3-H4, P00  
**Direct public handoffs:** `none`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Define the complete vendor-neutral contract surface shared by every other package.

## Owns

- strong identifiers/newtypes; protocol and schema versions
- source, membership, residency, view, grant, plan, result, handle and lifecycle records
- the exact v1 recipe registry and the closed reason-code registry
- canonical serialization inputs and validation rules that are pure schema concerns

## Must not own

- I/O, clocks, process state or persistence
- Qdrant, redb, Windows, parser, model or client-system types in public APIs
- raw UUID/string substitution where a domain identity exists
- silently accepting unknown load-bearing scope, security, budget or version fields

## Logical primitives

- IDs: InstallationId, InstallationIncarnationId, CollectionGenerationId, OwnerEpoch, Epoch, CorpusId, ReferencePortfolioId, PortfolioRevision, SourceNamespaceId, SourceOwnerGeneration, SourceId, SourceMembershipId, ProjectionMembershipId, SourceRevisionId, RepresentationId, UnitId, AccessPartitionId, ScoringPartitionId, ScoringDocumentId, ScopeDomainId, AccessDomainId, ConfidentialityDomainId, EncryptionKeyDomainId, RetentionDomainId, ErasureDomainId
- source graph: SourceNamespaceOwnership, SourceOwnerCutoverReceipt, SourceIdentity, PathBinding, SourceRevision, SourceMembership, Materialization, Representation, ProjectionMembership, UnitOccurrence
- views/security: SourceView, WorkspaceViewRevision, SearchObjectResidencyKey, SourceResidencyProfileRef, SearchReadGrantClaims, LiveDenySnapshotRef, SecurityMutationBarrierState
- query: RecipeId, SearchTaskPlan, QueryExecutionBudget, PlanFingerprint, SearchCandidateSet, Coverage, NativeAnchor, ExactScanPlan, ExactExecutionReport
- edge: ProviderEnvelope, SearchProviderCapabilityDescriptor, SearchSourceHandle, ContinuationHandle

## Logical operations

1. `Epoch::new(value) -> Result<Epoch, ContractError>`
2. `Epoch::checked_next() -> Result<Epoch, ContractError>`
3. `RecipeId::parse_versioned(value) -> Result<RecipeId, ContractError>`
4. `validate_source_view(view) -> Result<(), ContractError>`
5. `validate_residency_key(key) -> Result<(), ContractError>`
6. `validate_grant_shape(claims) -> Result<(), ContractError>`
7. `canonical_plan_fingerprint_input(plan_without_fingerprint) -> CanonicalBytes`
8. `validate_provider_envelope(envelope, negotiated_limits) -> Result<(), ContractError>`
9. `validate_candidate_set_binding(result, plan) -> Result<(), ContractError>`

## Required invariants

- v1 exposes exactly eleven recipe IDs and no aliases
- Epoch is signed i64 with 0 <= value < i64::MAX; no numeric infinity sentinel
- SourceIdentity contains no membership, corpus role or access policy
- ProjectionMembership binds exactly one SourceMembership; point payload schemas contain no membership arrays
- unknown load-bearing fields fail closed under the negotiated protocol version
- all wire fields that carry owner generation, view, security or budget identity are explicit

## Typed failure surface

- `EPOCH_OUT_OF_RANGE`
- `EPOCH_EXHAUSTED`
- `UNKNOWN_LOAD_BEARING_FIELD`
- `CONTRACT_VERSION_MISMATCH`
- `INVALID_CONTRACT_SHAPE`

## Exit tests / evidence

- `recipe_set_exact_test`
- `forbidden_epoch_sentinel_test`
- `membership_array_schema_rejection_test`
- `canonical_round_trip_fixture_for_every_public_record`
- `unknown_security_scope_budget_field_fails_closed`
- `vendor_type_dependency_guard`

## Suggested internal modules

```text
search-contracts/src/
  ids.rs
  version.rs
  source.rs
  residency.rs
  views.rs
  access.rs
  query.rs
  exact.rs
  result.rs
  protocol.rs
  lifecycle.rs
  reason.rs
  canonical.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep one crate while these modules share one versioned public schema. Request a split only if wire-version lifecycle or dependency policy becomes independently replaceable.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
