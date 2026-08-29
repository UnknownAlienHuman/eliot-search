# `search-model-provider` implementation packet

**Path:** `crates/search-model-provider`  
**Capability:** C12  
**Delivery:** W10 / P16  
**Gate:** HARD BLOCK: no implementation, dependency or artifact selection before accepted P15 and a dedicated ADR  
**Trace:** S17.2, S21.2, S29, P16  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

After product acceptance only, host vendor-neutral dense, multivector and rerank contracts behind an isolated optional provider.

## Owns

- optional model profile descriptors
- bounded encode/rerank request and response contracts
- provider capability/health and removal semantics
- measurement hooks for incremental benefit over P15

## Must not own

- baseline dependency or hidden fallback
- model selection before ADR
- canonical decisions, synthesis or admission
- direct redb/Qdrant ownership
- training or caching unsaved/source content outside explicit policy

## Logical primitives

- ModelProfileDescriptor, DenseEncodeRequest, DenseVectorBatch, RerankRequest, RerankResult, ModelProviderCapability, ModelProviderState, RemovalReceipt

## Logical operations

1. `probe_provider(descriptor) -> ProviderQualificationResult`
2. `encode(request, budget) -> Result<DenseVectorBatch, ModelError>`
3. `rerank(request, budget) -> Result<RerankResult, ModelError>`
4. `shutdown_and_remove() -> Result<RemovalReceipt, ModelError>`

## Required invariants

- package remains removable and disabled by default
- unavailable model narrows optional coverage and never breaks P15 lexical/code behavior
- profile/artifact/dimensions/quantization are immutable identity
- unsaved bytes never enter provider cache/telemetry/training
- no generative model is required on hot path

## Typed failure surface

- `MODEL_PROVIDER_DISABLED`
- `MODEL_PROVIDER_UNAVAILABLE`
- `MODEL_PROFILE_MISMATCH`
- `MODEL_BUDGET_EXHAUSTED`
- `OPTIONAL_DEPTH_NOT_ACCEPTED`

## Exit tests / evidence

- `feature_absent_by_default`
- `P15_behavior_without_provider`
- `exact_artifact_profile_fixture`
- `provider_removal_test`
- `unsaved_content_non_persistence`
- `measured_gain_gate`

## Suggested internal modules

```text
search-model-provider/src/
  profile.rs
  capability.rs
  encode.rs
  rerank.rs
  budget.rs
  removal.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Do not split or implement during baseline. A future provider-specific adapter may split after ADR qualification.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
