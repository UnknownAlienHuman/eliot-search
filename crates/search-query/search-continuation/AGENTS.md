# Agent contract — search-continuation

You own only `crates/search-query/search-continuation/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S14.3-S14.4, S26, H12, P08, P13.

## Mission

Own bounded opaque continuation state without exposing vendor cursors or pinning snapshots indefinitely.

## Ownership

- ephemeral in-memory continuation windows
- durable replan checkpoints
- TTL/count/binding quotas
- issued-ID suppression
- security/view/route revalidation

## Forbidden ownership

- raw Qdrant offsets or score cursors
- silent continuation on a newer corpus
- indefinite epoch pins
- durable continuation containing unsaved bytes

## Allowed dependencies

`search-contracts`, `search-domain`, `search-query-planner`, `search-access`, `search-epoch-pins`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `ContinuationStore::create(state, durability, limits) -> Result<ContinuationHandle, ContinuationError>`
- `ContinuationStore::resume(handle, latest_state) -> Result<ResumePlan, ContinuationError>`
- `ContinuationStore::invalidate(scope) -> InvalidationCount`
- `ContinuationStore::expire(now) -> ExpiredSet`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SNAPSHOT_EXPIRED`, `ACCESS_REVOKED`, `PURGED`, `RESOURCE_EXHAUSTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `raw vendor cursor is never serialized`
- `expired fence returns SNAPSHOT_EXPIRED`
- `revocation and purge invalidate affected continuations`
- `ephemeral continuation releases pin on expiry/disconnect`
- `durable checkpoint replans instead of trusting process-local pin`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08; hardened P13**
- Soft `src/` target: **6,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
