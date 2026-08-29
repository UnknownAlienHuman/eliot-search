# ADR 0003 — P00 contract projection and shared port ownership

- **Status:** accepted
- **Date:** 2026-08-29
- **Scope:** P00 implementation projection and package ownership
- **Architecture:** ELIOT Search 8.4 Part I is normative; Part II handoff is subordinate

## Context

The bounded swarm scaffold still left two P00 hazards:

1. `search-contracts` named many records but ordinary agents lacked compact field-level schemas,
   canonical encoding rules and public/internal reason-code separation.
2. H4 listed shared vendor-neutral ports, while no Cargo package owned their traits. Leaving them in
   prose would cause capability agents to invent incompatible local interfaces or import concrete
   adapters.

A load-bearing handoff inconsistency also exists: Part I S7.2.1 defines
`source_owner_generation` as a BLAKE3-256 digest, while Part II H3.1 sketches
`SourceOwnerGeneration(NonZeroU64)`. Part I is the normative architecture body and therefore wins.

## Decision

- Add `search-ports`, depending only on `search-contracts`, as the owner of shared vendor-neutral port
  traits and conformance interfaces.
- Keep shared serialized records, IDs, reason registries and canonicalization inputs in
  `search-contracts`.
- Keep pure state transitions, ordering and coverage meaning in `search-domain`.
- Publish a compact P00 contract pack under `docs/contracts/p00/`; it is a mechanically reviewable
  implementation projection of Architecture 8.4, not a second product architecture.
- Define `SourceOwnerGeneration` as a dedicated BLAKE3-256 digest newtype. `OwnerEpoch` remains the
  independent `NonZeroU64` process-owner epoch.
- Separate public provider reason codes from contract-validation, protocol and package-local fault
  namespaces. Package-local codes require an explicit mapping before provider emission.
- Use deterministic CBOR with domain separation for load-bearing identity/fingerprint inputs; the
  named-pipe transport remains length-prefixed UTF-8 JSON.

## Dependency direction

```text
search-contracts
  ├─ search-domain
  └─ search-ports
       ↑ capability packages and adapters
       ↑ eliot-searchd composition
```

`search-ports` has no concrete implementation and no dependency on `search-domain`; a port exposes
validated contract shapes, while the implementing capability applies its own accepted domain rules.

## Consequences

- P00 may run `search-domain` and `search-ports` in parallel only after the contracts handoff is
  accepted.
- Future packages consume one accepted port API digest rather than defining local traits.
- `search-contracts` remains bounded enough to focus on schemas instead of mixing adapter interfaces.
- The Architecture 8.4 embedded body and hash remain unchanged because this ADR resolves a subordinate
  handoff typo and implementation ownership without changing Part I behavior or invariants.
