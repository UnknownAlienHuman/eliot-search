# `search-materializer` implementation packet

**Path:** `crates/search-prep/search-materializer`  
**Capability:** C08  
**Delivery:** W2 baseline / P04; W10 optional P17  
**Gate:** BASELINE text/code work blocked until revision-store receipt; document depth blocked until accepted P15 plus ADR  
**Trace:** S17, S24, H13, P04, P17  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Convert one immutable source revision into a canonical representation with explicit coordinate and loss semantics.

## Owns

- baseline text/code decoding and canonical representation descriptors
- materializer profile identity and assurance ceiling
- coordinate/loss map production contracts
- provider-neutral document materializer port and qualification boundary

## Must not own

- selecting PDF/OCR/Office/archive provider in baseline
- executing macros, remote resources or archive contents
- claiming fidelity above the loss map
- Python/Node production dependency without ADR

## Logical primitives

- MaterializationRequest, MaterializerProfileDescriptor, CanonicalRepresentation, CoordinateMap, LossMap, MaterializationReceipt, AssuranceCeiling, ProviderQualification

## Logical operations

1. `materialize_text_or_code(revision, profile, budget) -> Result<MaterializationReceipt, MaterializeError>`
2. `decode_with_declared_profile(bytes, profile) -> Result<CanonicalRepresentation, MaterializeError>`
3. `build_coordinate_and_loss_maps(native, canonical) -> MapBundle`
4. `validate_provider_qualification(descriptor, evidence) -> Result<(), MaterializeError>`

## Required invariants

- baseline supports raw text/source code without optional workers
- every transformation identifies profile/version and maps
- lossy output lowers assurance explicitly
- unsaved bytes are never materialized durably without snapshot admission
- optional provider removal returns to baseline behavior

## Typed failure surface

- `MATERIALIZATION_UNSUPPORTED`
- `MATERIALIZATION_LOSS`
- `MATERIALIZATION_BUDGET_EXHAUSTED`
- `PROVIDER_NOT_QUALIFIED`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`

## Exit tests / evidence

- `utf8_and_declared_encoding_materialization`
- `crlf_coordinate_map`
- `lossy_decode_lowers_assurance`
- `optional_provider_absent_by_default`
- `no_execute_fixture`
- `provider_removal_fallback`

## Suggested internal modules

```text
search-materializer/src/
  profile.rs
  text.rs
  decode.rs
  coordinate.rs
  loss.rs
  provider_port.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- A selected document provider belongs behind the port and worker; split only after an accepted P17 ADR establishes a real replaceable boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
