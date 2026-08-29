# Agent contract — search-model-provider

You own only `crates/search-model-provider/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S9.4, S21.2, S29, S35, P16.

## Mission

Define the isolated optional dense, rerank and multivector provider boundary; no model is selected in the scaffold.

## Ownership

- versioned model profile descriptors
- worker request/response contracts
- health, cancellation and resource accounting
- dense/rerank result validation
- uninstall/fallback-to-baseline proof seam

## Forbidden ownership

- baseline dependency on a model
- canonical decisions or generative answers
- implicit downloads or network calls
- starting before P15 acceptance and an ADR

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `ModelProvider::profile() -> ModelProfileDescriptor`
- `ModelProvider::embed(batch, budget) -> Result<DenseBatch, ModelError>`
- `ModelProvider::rerank(query, candidates, budget) -> Result<RerankOutput, ModelError>`
- `ModelProvider::cancel(request_id) -> CancelOutcome`
- `qualify_model_provider(descriptor, evaluation) -> QualificationDecision`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `OPTIONAL_PROVIDER_UNAVAILABLE`, `MODEL_PROFILE_MISMATCH`, `RESOURCE_EXHAUSTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `crate remains feature-disabled before P15 acceptance`
- `provider removal returns system to P15 behavior`
- `profile changes are versioned and non-silent`
- `cancellation and memory limits are enforced`
- `no content persists in provider cache without explicit policy`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W10 / P16 after accepted P15**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Gate

This package is optional. Do not implement or enable it before the stated gate and ADR.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
