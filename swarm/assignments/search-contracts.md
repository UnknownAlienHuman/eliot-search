# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED by launch state only

Read the full P00 pack and `AUTHORITY_MAP.md`.

## Mission

Implement provider-wire, shared-domain and server-record schemas so downstream agents never invent
fields, wrappers, bounds, visibility, snapshot axes, result bodies, handle boundaries or reasons.

## Owns

Strong IDs/digests/bounded wrappers; `ContractBoundsV1`; all type-registry entries marked
`ProviderWire`, `SharedDomain` or `ServerRecord`; exact `QuerySnapshotFence` and separate emission
fence; source/residency/view/grant/recipe/plan/result schemas; opaque handles and server records;
protocol/security/lifecycle; canonical JSON/CBOR and reason namespaces.

## Does not own

`PortSupport`, traits, I/O, clocks, randomness, mutable runtime state, adapters, vendor/native/client
types, raw primitive aliases, invalid evidence or generic digests replacing load-bearing fields.

## Required decisions

Owner generation vs epoch; occurrence `u64`; exact state spelling; tagged variants; exact eleven
requests/results; ambiguity excludes evidence; candidates validated only; wire handles opaque;
explicit S14 snapshot fields; planned snapshot separated from live emission fence; every support type
has owner/visibility/bound; deterministic CBOR and length-prefixed JSON.

## Exit evidence

Golden identities and query-snapshot fingerprint; exact bounds; all records round-trip; exact
registries; no unresolved aliases; invalid-tag/unknown-field rejection; snapshot field exactness and
no-hidden-axis tests; planned-vs-emission fence tests; ambiguity/candidate/handle tests; no membership
arrays/vendor/port-support types; final API digest and zero challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop 10,000 including local tests.
