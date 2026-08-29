# `search-unitizer` implementation packet

**Path:** `crates/search-prep/search-unitizer`  
**Capability:** C09  
**Delivery:** W2 / P04-P06  
**Gate:** BLOCKED until materializer and revision-store contracts are accepted  
**Trace:** S7.5-S7.6, S24, P04-P06  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Produce deterministic unit occurrences and native anchors from one representation without ranking or indexing.

## Owns

- unitizer profile descriptors
- deterministic unit boundaries and ordinals
- unit manifest generation
- native-anchor and configuration-predicate attachment

## Must not own

- ranking, lexical weighting or Qdrant points
- claiming unit stability across incompatible reparses
- compiler-level semantic assurance
- source revision storage

## Logical primitives

- UnitizerProfile, UnitizationRequest, UnitOccurrenceDraft, UnitManifest, UnitDigest, UnitBoundary, StructuralIdentityHint

## Logical operations

1. `unitize(representation, profile, budget) -> Result<UnitManifest, UnitizeError>`
2. `derive_unit_id(representation, ordinal, kind, anchor) -> UnitId`
3. `validate_unit_anchor(unit, representation_maps) -> Result<(), UnitizeError>`
4. `canonicalize_unit_manifest(units) -> CanonicalBytes`

## Required invariants

- same representation/profile yields identical ordered manifest
- unit is an occurrence in one representation
- anchors always name coordinate basis and revision lineage
- empty/oversized units follow explicit profile rules
- unitizer emits no ranking score or vendor payload

## Typed failure surface

- `UNITIZATION_FAILED`
- `UNITIZATION_NONDETERMINISTIC`
- `ANCHOR_MAPPING_FAILED`
- `UNIT_BUDGET_EXHAUSTED`
- `UNSUPPORTED_REPRESENTATION`

## Exit tests / evidence

- `deterministic_manifest_golden`
- `unit_order_and_ids_stable`
- `anchor_with_revision_digest_required`
- `oversized_input_budget`
- `no_ranking_or_vendor_dependency`
- `changed_profile_changes_manifest_identity`

## Suggested internal modules

```text
search-unitizer/src/
  profile.rs
  boundary.rs
  unit.rs
  anchor.rs
  manifest.rs
  canonical.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Language-specific structural enrichment belongs in enrichers; keep general deterministic unitization cohesive.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
