# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only by `swarm/launch-state.toml`  
**Trace:** Architecture Part I S3, S7, S10, S19-S26, S30.3, S32-S34; H3; P00

Read every P00 contract-pack file.

## Mission

Implement the complete vendor-neutral v1 type system so downstream agents never rediscover fields,
helper types, bounds, visibility, variants, result bodies, handle boundaries or reason namespaces.

## Owns

Strong IDs/digests/bounded wrappers; the closed supporting type registry; source/residency/view records;
grant/recipe/plan/result schemas; all eleven results; opaque wire handles; server handle records;
protocol/security/lifecycle records; canonical JSON/CBOR and reason namespaces.

## Must not own

Ports, I/O, clocks, randomness, runtime state, persistence, vendor/native/client types, raw
UUID/string/Vec aliases, opaque authority objects, invalid evidence candidates or self-contained tokens
encoding source/plan/currentness fields.

## Required decisions

- owner generation is BLAKE3-256; owner epoch is nonzero integer;
- occurrence sequence is `u64`;
- explicit wire/state spelling is preserved;
- mutually exclusive states are tagged;
- exactly eleven request/result variants;
- ambiguity excludes resolved evidence;
- candidate lists contain validated candidates only;
- wire handles are opaque bearer locators; detailed records are server-only;
- every helper type has visibility, representation and bounds from `TYPE_REGISTRY.md`;
- identities use deterministic CBOR; provider frames use length-prefixed UTF-8 JSON.

## Required modules

```text
ids.rs bounds.rs canonical.rs support.rs source.rs residency.rs views.rs
recipes.rs access.rs query.rs result.rs recipe_results.rs exact.rs
protocol.rs handles.rs lifecycle.rs reason.rs
```

## Exit evidence

Golden identities; exact bounds table; round-trip fixtures; exact recipe/result/reason registries;
helper-type inventory with no unresolved aliases; invalid-tag/unknown-field rejection;
ambiguity/evidence and validated-candidate tests; wire-handle non-disclosure; exact state casing; no
membership arrays/vendor types; final API/schema digest and zero challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
