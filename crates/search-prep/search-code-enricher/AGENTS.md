# Agent contract — search-code-enricher

You own only `crates/search-prep/search-code-enricher/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S17.1, S17.3, S21.2, S22, P10.

## Mission

Produce provider-qualified Rust definitions, references, tests and documentation facts without claiming compiler truth.

## Ownership

- Rust structural profile
- definition/reference/test/doc role extraction
- configuration predicates
- provider assurance and parser identity
- structural relation manifest

## Forbidden ownership

- compiler-grade certainty from tolerant parsing
- running build scripts or language-server builds
- ranking or final normative comparison
- vendor parser types in public APIs
- opening source stores directly instead of consuming immutable contract inputs

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `enrich_code(representation, profile, budget) -> Result<StructuralFacts, EnrichError>`
- `classify_evidence_role(node) -> EvidenceRole`
- `derive_configuration_predicate(node, context) -> Option<Predicate>`
- `assurance_for(parse_state, provider) -> ProviderAssurance`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `STRUCTURAL_PROVIDER_DEGRADED`, `MALFORMED_SOURCE`, `CONFIGURATION_UNKNOWN`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `malformed Rust yields bounded tolerant facts`
- `cfg variants remain distinguishable`
- `tests/docs/callers are role-tagged`
- `non-UTF8 input is rejected or mapped explicitly`
- `no compiler-truth overclaim in assurance`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W5 / P10**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
