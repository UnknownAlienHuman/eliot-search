# Agent contract — search-runtime-owner

You own only `crates/search-runtime/search-runtime-owner/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.1, S27.1, S33, H1, P01.

## Mission

Guarantee that exactly one process incarnation owns one data root and expose a fenced lifecycle to the daemon.

## Ownership

- data-root lease and owner epoch
- standalone/managed mode fence
- crash/reopen ownership recovery
- clean shutdown and drain state

## Forbidden ownership

- retrieval semantics
- Qdrant schema or query operations
- source catalog or access policy

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `DataRootOwner::acquire(path, mode) -> Result<OwnerGuard, OwnerError>`
- `OwnerGuard::owner_epoch() -> OwnerEpoch`
- `OwnerGuard::begin_drain() -> DrainToken`
- `OwnerGuard::release_cleanly() -> Result<ShutdownReceipt, OwnerError>`
- `recover_abandoned_owner(record) -> RecoveryDecision`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `DATA_ROOT_ALREADY_OWNED`, `OWNER_EPOCH_MISMATCH`, `MODE_TRANSITION_REQUIRES_RESTART`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `second_owner_denied`
- `crash_reopen_advances_or_fences_owner_epoch`
- `managed_and_standalone_modes_cannot_coown_root`
- `mode_transition_requires_drain_fence_and_restart`
- `owner guard releases on orderly shutdown`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W1 / P01**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
