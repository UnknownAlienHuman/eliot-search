# Agent contract — search-unitizer

You own only `crates/search-prep/search-unitizer/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.5-S7.6, S11.3, S17, S24, P04, P06.

## Mission

Turn a materialization into deterministic unit occurrences and an immutable unit manifest.

## Ownership

- unitizer profiles
- UnitOccurrence creation
- native anchor preservation
- ordinal/structural identity rules
- unit manifest digest and determinism

## Forbidden ownership

- ranking
- assuming unit stability across arbitrary reparses
- compiler certainty
- Qdrant point transport
- opening source stores directly instead of consuming immutable contract inputs

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `unitize(materialization, profile) -> Result<UnitManifest, UnitizeError>`
- `derive_unit_occurrence(representation, unit) -> UnitOccurrence`
- `validate_anchor(unit, maps) -> Result<(), UnitizeError>`
- `manifest_digest(units, profile) -> Digest`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `UNITIZATION_NONDETERMINISTIC`, `ANCHOR_MAP_INVALID`, `MATERIALIZATION_LOSS`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `same input/profile yields byte-identical manifest`
- `unit ordinals and anchors are deterministic`
- `lossy maps cap assurance`
- `empty and malformed representations remain bounded`
- `unit IDs are occurrence-specific, not assumed globally stable`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P04-P06**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
