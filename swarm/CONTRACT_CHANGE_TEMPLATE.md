# Contract change request

## Identity

- Requesting package:
- Contract-owning package:
- Base commit:
- Architecture sections, only when needed:

## Blocking problem

Describe the exact missing, contradictory or unusable field, operation or invariant. Do not propose a
local workaround.

## Producer and consumers

- Producer:
- Current consumers:
- Future compatibility surface:

## Proposed contract

Provide vendor-neutral types/semantics, validation rules, serialization/version behavior and failure
codes. Concrete Rust syntax is optional; behavior is mandatory.

## Impact

- Security/access:
- Currentness/epoch/publication:
- Source identity/readback:
- Residency/retention/purge:
- Protocol/wire compatibility:
- Migration/backward compatibility:
- Resource budget:

## Tests

List deterministic contract, property, compile-fail, fault or fixture changes required to prove the
proposal.

## Decision needed

Classify as one of:

- clarification inside the current contract;
- compatible contract extension;
- breaking contract revision;
- ADR-required implementation default;
- Architecture 8.4 revision required.
