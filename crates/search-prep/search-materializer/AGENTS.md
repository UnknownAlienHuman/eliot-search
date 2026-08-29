# Agent contract — search-materializer

You own only `crates/search-prep/search-materializer/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.5, S17, S24, H6, P04, P17.

## Mission

Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.

## Ownership

- materializer profile contracts
- raw text/source-code baseline materialization
- coordinate map and loss map production
- assurance ceiling classification
- provider qualification seam for optional documents

## Forbidden ownership

- selecting a PDF/Office/OCR provider without ADR
- authority or ranking
- executing macros, archive members or remote resources
- claiming exact coordinates after lossy transforms
- opening source stores directly instead of consuming immutable contract inputs

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `materialize(revision, profile, budget) -> Result<MaterializationOutput, MaterializeError>`
- `validate_coordinate_map(map, source, output) -> Result<(), MaterializeError>`
- `derive_assurance_ceiling(loss_map) -> AssuranceCeiling`
- `qualify_provider(descriptor, fixtures) -> ProviderQualification`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `MATERIALIZATION_LOSS`, `MATERIALIZER_UNAVAILABLE`, `COORDINATE_MAP_INVALID`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `raw text round trip preserves exact byte anchors`
- `CRLF/transcoding produces explicit coordinate map`
- `lossy output cannot claim exact_bytes`
- `malformed and oversized input is bounded`
- `provider absence does not block baseline text/code`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 baseline / P04; optional P17**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
