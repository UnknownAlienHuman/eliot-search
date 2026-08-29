# Agent contract — search-epoch-pins

You own only `crates/search-index-qdrant/search-epoch-pins/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S13.7, S14.2-S14.3, S26.3, H12, P07.

## Mission

Protect active query snapshots and old collection routes in memory without writing ordinary query leases.

## Ownership

- RAII epoch/route pin registry
- pin quotas and cancellation release
- reclamation watermark
- bounded continuation pin integration
- route-drain observation

## Forbidden ownership

- durable QueryFenceLease for normal reads
- query result storage
- publication or deletion decisions
- indefinite pins

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `EpochPinRegistry::acquire(route, epoch, owner) -> Result<PinGuard, PinError>`
- `EpochPinRegistry::watermark(route) -> ReclamationWatermark`
- `EpochPinRegistry::release_owner(owner) -> ReleaseCount`
- `EpochPinRegistry::can_reclaim(route, retired_epoch) -> bool`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SNAPSHOT_EXPIRED`, `PIN_LIMIT_EXCEEDED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `pinned visible points cannot be reclaimed`
- `guard releases on success, cancellation and disconnect`
- `daemon crash requires no durable lease cleanup`
- `continuation TTL bounds pin lifetime`
- `old route drains only after final pin release`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P07**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
