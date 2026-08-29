# Agent contract — search-eliot-adapter

You own only `crates/search-eliot-adapter/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S1.3, S32.3, H16.3-S16.5, P14.

## Mission

Map ELIOT external-provider contracts to generic Search contracts as a disabled-by-default leaf package.

## Ownership

- WorkScope/disclosure to grant mapping
- SourceView/StateFence mapping
- capability pulse projection
- Search result to ELIOT provider-result translation
- binding/session mapping

## Forbidden ownership

- ELIOT canonical DB credentials or writes
- memory/admission/finish dispositions
- Qdrant/redb types
- importing ELIOT internals into Search core crates
- creating a new authority surface

## Allowed dependencies

`search-contracts`, `search-domain`, `search-provider-protocol`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `map_eliot_scope(input) -> Result<SearchReadGrantClaims, AdapterError>`
- `map_state_fence(input) -> Result<SearchRequestFence, AdapterError>`
- `map_capability_descriptor(descriptor) -> EliotProviderPulse`
- `map_search_result(result) -> EliotProviderResult`
- `prove_no_reverse_authority(config) -> Result<(), AdapterError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `CLIENT_ADAPTER_AUTHORITY_VIOLATION`, `ADAPTER_MAPPING_MISMATCH`, `INCOMPLETE_COVERAGE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `no canonical database access or credentials`
- `no ELIOT memory disposition in Search response`
- `generic request-plan-result round trip preserves generations/coverage`
- `provider failure narrows coverage without blocking unrelated work`
- `feature disabled by default`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W8 / optional P14 profile**
- Soft `src/` target: **5,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Gate

This package is optional. Do not implement or enable it before the stated gate and ADR.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
