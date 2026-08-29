# Agent contract — search-contracts

You own only `crates/search-contracts/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S3, S7, S10, S19, S20, S23-S26, S30.3, S32, S34, H3-H4, P00.

## Mission

Define the complete vendor-neutral wire and domain contract surface used by every other package.

## Ownership

- newtypes and identifiers
- recipes and reason codes
- source/view/membership/residency schemas
- grants, plans, budgets and candidate/result schemas
- anchors, handles, protocol envelopes and capability descriptors

## Forbidden ownership

- runtime state or I/O
- redb, Qdrant, Windows or client-vendor types
- implicit string/UUID substitution at domain boundaries
- silently ignored security, scope or budget fields

## Allowed dependencies

none. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `Epoch::new(i64) -> Result<Epoch, ContractError>`
- `Epoch::checked_next(Epoch) -> Result<Epoch, ContractError>`
- `RecipeId::parse_versioned(&str) -> Result<RecipeId, ContractError>`
- `SourceView::validate() -> Result<(), ContractError>`
- `SearchReadGrantClaims::validate_shape() -> Result<(), ContractError>`
- `SearchTaskPlan::canonical_fingerprint_input() -> CanonicalBytes`
- `ProviderEnvelope::validate_version_and_limits() -> Result<(), ContractError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `EPOCH_OUT_OF_RANGE`, `EPOCH_EXHAUSTED`, `UNKNOWN_LOAD_BEARING_FIELD`, `CONTRACT_VERSION_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `recipe_set_exact_test exposes exactly the eleven v1 recipes`
- `forbidden_epoch_sentinel_test rejects negative and i64::MAX epochs`
- `membership_array_compile_or_schema_rejection_test`
- `unknown_load_bearing_field_fails_closed`
- `canonical_round_trip fixtures for every public schema`
- `vendor_type_dependency_guard`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W0 / P00**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
