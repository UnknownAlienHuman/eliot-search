# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED by launch state only

Read the full P00 pack and `AUTHORITY_MAP.md`.

## Mission

Implement provider-wire, shared-domain and server-record schemas so downstream agents never invent
fields, wrappers, bounds, visibility, result bodies, handle boundaries or reason namespaces.

## Owns

Strong IDs/digests/bounded wrappers; `ContractBoundsV1`; all type-registry entries marked
`ProviderWire`, `SharedDomain` or `ServerRecord`; source/residency/view records; grant/recipe/plan/result
schemas; eleven results; opaque handles and server records; protocol/security/lifecycle; canonical
JSON/CBOR and reasons.

## Does not own

Entries marked `PortSupport`, port traits, I/O, clocks, randomness, mutable runtime state, adapters,
vendor/native/client types, raw primitive aliases, invalid evidence candidates or self-contained
source/plan-bearing tokens.

## Required decisions

- owner generation digest vs owner epoch integer;
- occurrence sequence `u64`;
- exact wire/state spelling;
- tagged mutually exclusive states and exact eleven request/result variants;
- ambiguity excludes evidence; candidates are validated only;
- public handles opaque; server details non-wire;
- every support type has owner/visibility/representation/bound;
- exact immutable `ContractBoundsV1` table/digest;
- deterministic CBOR identities and length-prefixed JSON transport.

## Exit evidence

Golden identities; exact bounds; every record round-trips; exact registries; no unresolved helper
aliases; invalid-tag/unknown-field rejection; ambiguity/evidence and validated-candidate tests;
wire-handle non-disclosure; state casing; no membership arrays/vendor/port-support types; final API
digest and zero challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop 10,000 including local tests.
