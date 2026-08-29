# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only by `swarm/launch-state.toml`  
**Trace:** Architecture Part I S3, S7, S10, S19-S26, S30.3, S32-S34; H3; P00

Read every file under `docs/contracts/p00/`.

## Mission

Implement the complete vendor-neutral v1 type system. Downstream agents must not rediscover a field,
variant, bound, recipe result or reason namespace from the architecture master.

## Owns

- strong IDs, digests, versions, bounded wrappers and tagged variants;
- source/ownership/residency/view/admission records;
- grant, recipe, budget, plan, validated-candidate, coverage-gap and exact-proof records;
- field-level outputs and exact tagged result union for all eleven recipes;
- envelope, capability, handle, security and lifecycle records;
- canonical JSON/CBOR inputs and four error/reason namespaces.

## Must not own

Ports, I/O, clocks, randomness, runtime state, persistence, vendor/native/client types, raw UUID/string
substitutions, opaque authority/currentness objects, or invalid evidence candidates.

## Required decisions

- owner generation is BLAKE3-256; owner epoch is `NonZeroU64`;
- revision occurrence sequence remains `u64`;
- exact wire/state spelling is preserved;
- all mutually exclusive states are tagged enums;
- v1 has exactly eleven request and result variants;
- inspection/exploration/comparison ambiguity cannot coexist with resolved evidence;
- candidate lists contain validated candidates only; failures are non-evidence gaps;
- canonical identities use domain-separated deterministic CBOR;
- provider frames use length-prefixed UTF-8 JSON.

## Required modules

```text
ids.rs canonical.rs source.rs residency.rs views.rs
recipes.rs access.rs query.rs result.rs recipe_results.rs
exact.rs protocol.rs handles.rs lifecycle.rs reason.rs
```

## Exit evidence

Golden canonical identities; round-trip schema fixture for every record; exact recipe/result/reason
registries; invalid-tag and unknown-field rejection; ambiguity/evidence mutual-exclusion tests;
validated-candidate-only tests; exact wire-state casing; no membership arrays/vendor types; final
API/schema digest and zero unresolved challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
