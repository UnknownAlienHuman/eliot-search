# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only by `swarm/launch-state.toml`  
**Trace:** Architecture Part I S3, S7, S10, S19-S26, S30.3, S32-S34; H3; P00

Read every file under `docs/contracts/p00/`.

## Mission

Implement the complete vendor-neutral v1 type system so downstream agents never rediscover a field,
variant, bound, recipe result, handle boundary or reason namespace.

## Owns

Strong IDs/digests/bounded wrappers; source/residency/view/admission records; grant/recipe/plan/result
records; all eleven result variants; opaque public handles; server-side handle/continuation records;
envelope/capability/security/lifecycle records; canonical JSON/CBOR and reason namespaces.

## Must not own

Ports, I/O, clocks, randomness, runtime state, persistence, vendor/native/client types, raw UUID/string
substitutions, opaque authority objects, invalid evidence candidates, or self-contained bearer tokens
that encode source/plan/currentness fields.

## Required decisions

- owner generation is BLAKE3-256; owner epoch is `NonZeroU64`;
- occurrence sequence remains `u64`;
- exact wire/state spelling is preserved;
- mutually exclusive states are tagged enums;
- v1 has exactly eleven request and result variants;
- ambiguity cannot coexist with resolved evidence;
- candidate lists contain validated candidates only;
- public source/continuation handles are opaque random bearer locators;
- detailed source, binding, plan, fence and residency fields live only in server records;
- durable source records cannot target unsaved bytes; durable continuation records cannot carry a
  process-local pin;
- identities use deterministic CBOR; provider frames use length-prefixed UTF-8 JSON.

## Required modules

```text
ids.rs canonical.rs source.rs residency.rs views.rs
recipes.rs access.rs query.rs result.rs recipe_results.rs
exact.rs protocol.rs handles.rs lifecycle.rs reason.rs
```

## Exit evidence

Golden identities; round-trip fixtures; exact recipe/result/reason registries; invalid-tag and
unknown-field rejection; ambiguity/evidence exclusion; validated-candidate-only tests; opaque-wire
handle non-disclosure; internal-record/provider-result separation; exact state casing; no membership
arrays/vendor types; final API/schema digest and zero unresolved challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
