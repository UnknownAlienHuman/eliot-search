# `search-publication` implementation packet

**Path:** `crates/search-index-qdrant/search-publication`  
**Capability:** C16  
**Delivery:** W3 / P07  
**Gate:** BLOCKED until qdrant bridge, projection planner, point identity and epoch-pin receipts are accepted  
**Trace:** S13-S14, H11, P07  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`, `search-qdrant-bridge`, `search-epoch-pins`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Linearize projection publication through one globally serialized commit actor with exact manifests, readback, compensation and crash recovery.

## Owns

- PublicationIntent/Receipt and publication state machine
- global serialization and micro-batch admission
- exact new upsert/old closure/readback/control-commit sequence
- failpoints, compensation, invalidation-only and abandoned-publication fencing

## Must not own

- parallel unresolved publication epochs
- wait=false publication writes
- broad filter closure when exact point IDs exist
- making Qdrant alias change the currentness commit point
- reusing skipped/aborted epoch numbers

## Logical primitives

- PreparedPublication, PublicationIntent, PublicationState, PublicationGuardSet, PublicationReceipt, CompensationPlan, RecoveryDecision, AbandonedPublicationFence

## Logical operations

1. `submit(prepared) -> Result<PublicationReceipt, PublicationError>`
2. `advance_intent(intent, observed_phase) -> Result<PublicationState, PublicationError>`
3. `verify_exact_readback(manifest, points) -> Result<(), PublicationError>`
4. `commit_visible_epoch(intent, guards) -> Result<PublicationReceipt, PublicationError>`
5. `recover_unresolved_intent(intent, observed_external_state) -> RecoveryDecision`
6. `compensate_exact(intent) -> Result<CompensationReceipt, PublicationError>`
7. `abandon_with_verified_fence(intent, fence) -> Result<AbandonReceipt, PublicationError>`

## Required invariants

- at most one active commit transaction globally
- uncommitted epoch is never current
- all external writes are acknowledged and exact-readback verified before control commit
- control CAS rechecks owner/source/membership/access/shadow/purge guards atomically
- matching shadow is released only with committed publication
- aborted/skipped epoch is never reused

## Typed failure surface

- `PUBLICATION_BLOCKED`
- `PUBLICATION_READBACK_MISMATCH`
- `PUBLICATION_GUARD_CHANGED`
- `PUBLICATION_COMPENSATION_FAILED`
- `PUBLICATION_ABANDON_FENCE_REQUIRED`
- `EPOCH_EXHAUSTED`

## Exit tests / evidence

- `all_H11_5_process_kill_failpoints`
- `guard_race_between_external_recheck_and_control_tx`
- `no_staged_visibility`
- `exact_old_point_closure_only`
- `no_pinned_reclamation`
- `abandon_requires_exclusion_fence`
- `cache_publication_failure_is_fail_closed`

## Suggested internal modules

```text
search-publication/src/
  actor.rs
  intent.rs
  state.rs
  guard.rs
  commit.rs
  readback.rs
  compensate.rs
  recover.rs
  failpoint.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep commit/recovery/compensation in one crate because one state machine and serialization boundary own them. Split internal modules before nearing 8,500 lines.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
