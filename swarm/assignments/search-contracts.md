# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only by `swarm/launch-state.toml`  
**Trace:** Architecture Part I S3, S7, S10, S19-S26, S30.3, S32-S34; H3; P00  
**Direct public handoffs:** none

Read every file under `docs/contracts/p00/`.

## Mission

Implement the complete vendor-neutral v1 type system. Downstream agents must not rediscover a field,
variant, bound, recipe result or reason namespace from the architecture master.

## Owns

- strong IDs, digests, versions, bounded opaque wrappers and tagged variants;
- source/ownership/residency/view/admission records;
- grant, recipe, budget, plan, validated-candidate, coverage-gap and exact-proof records;
- field-level outputs and exact tagged result union for all eleven recipes;
- envelope, capability, handle, security and lifecycle records;
- canonical JSON/CBOR inputs and four error/reason namespaces.

## Must not own

- port traits, I/O, clocks, random generation, runtime state or persistence;
- vendor/native/client types;
- raw UUID/string substitution for declared identities;
- opaque objects hiding authority/currentness;
- stale/unreadable/inaccessible entries in emitted candidate lists.

## Required decisions

- `SourceOwnerGeneration` is BLAKE3-256; `OwnerEpoch` is `NonZeroU64`;
- `SourceRevision.occurrence_sequence` remains architectural `u64`;
- exact wire/state enum spelling is preserved where Part I specifies it;
- nullable variants become tagged enums;
- v1 exposes exactly eleven recipes and eleven matching result variants;
- `SearchCandidateSet.candidates` contains validated candidates only;
- failed validation is a non-evidence coverage gap;
- identity/fingerprint inputs use domain-separated deterministic CBOR;
- provider frames remain length-prefixed UTF-8 JSON.

## Required modules

```text
ids.rs canonical.rs source.rs residency.rs views.rs
recipes.rs access.rs query.rs result.rs recipe_results.rs
exact.rs protocol.rs handles.rs lifecycle.rs reason.rs
```

## Exit evidence

- golden canonical bytes/digests for every load-bearing identity domain;
- schema/roundtrip fixture for every public record;
- exact recipe request/result and public-reason registries;
- invalid tagged combinations/unknown load-bearing fields fail closed;
- emitted candidate is always validated and every gap contains no evidence excerpt;
- exact wire-state casing fixtures;
- membership arrays/vendor types are structurally impossible;
- API/schema digest and zero unresolved challenges.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
