# `search-contracts` implementation packet

**Path:** `crates/search-contracts`  
**Capability:** C00 shared schemas  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED only by `swarm/launch-state.toml`  
**Trace:** Architecture Part I S3, S7, S10, S19-S26, S30.3, S32-S34; H3; P00  
**Direct public handoffs:** none

Read all files in `docs/contracts/p00/`.

## Mission

Implement the complete vendor-neutral v1 type system and canonical serialization surface. Downstream
agents must not need the architecture master to discover a field, variant, bound or reason namespace.

## Owns

- strong IDs, digests, versions, bounded opaque wrappers and tagged variants
- source/ownership/residency/view/admission records
- grant, recipe, budget, plan, result, exact-proof and coverage records
- provider envelope, capability, handle, security and lifecycle records
- canonical JSON/CBOR inputs and four error/reason namespaces

## Must not own

- port traits, I/O, clocks, random generation, process/runtime state or persistence
- Qdrant/redb/Windows/parser/model/client-vendor types
- raw UUID/string substitution for a declared identity
- opaque `object` fields that hide load-bearing authority/currentness state

## Required decisions

- `SourceOwnerGeneration` is BLAKE3-256; `OwnerEpoch` is `NonZeroU64`
- nullable variant records become tagged enums with exact legal fields
- v1 exposes exactly eleven recipes
- only `SearchReasonCodeV1` enters candidate/coverage results
- canonical fingerprint inputs use domain-separated deterministic CBOR
- provider frames remain length-prefixed UTF-8 JSON

## Required modules

```text
ids.rs              strong identities and revisions
canonical.rs        JSON/CBOR and domain separation
source.rs           source graph, ownership and admission
residency.rs        complete residency closure
views.rs            tagged source/workspace views
recipes.rs          exact registry and request bodies
access.rs           grants/security fences
query.rs            budget/task-plan records
result.rs           candidates/coverage/comparison outputs
exact.rs            anchors, predicates, plans and reports
protocol.rs         envelope/capabilities
handles.rs          source/continuation handles
lifecycle.rs        publication/purge/restore records
reason.rs           public/protocol/contract namespaces
```

## Exit evidence

- golden canonical bytes/digests for every load-bearing identity domain
- schema/roundtrip fixture for every public record
- exact recipe and public-reason registries
- invalid tagged combinations and unknown load-bearing fields fail closed
- membership arrays and vendor types are structurally impossible
- API/schema digest and a field inventory with zero unresolved challenges

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
