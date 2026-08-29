# `search-retention` implementation packet

**Path:** `crates/search-runtime/search-retention`  
**Capability:** C28  
**Delivery:** W7 / P13  
**Gate:** BLOCKED until W6 and lifecycle dependency handoffs are accepted  
**Trace:** S26, S28, H6.2, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-epoch-pins`, `search-index-reclaimer`, `search-handles`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Own crash-safe CAS retention, monotonic purge and restore quarantine through vendor-neutral ports while keeping ordinary retired-point reclamation and handle state in their own packages.

## Owns

- durable retention roots and leases
- crash-safe mark manifests and resumable CAS sweep receipts
- purge fences/tombstones and truthful multi-layer purge receipts
- paired recovery-manifest validation and restore quarantine
- lifecycle invalidation requests for handles and index state

## Must not own

- claiming guaranteed physical secure erasure
- deleting client-owned canonical evidence
- refcount-only correctness
- restoring/reindexing before identity, policy and source revalidation
- ordinary retired-point reclaim owned by `search-index-reclaimer`
- handle record storage/authorization owned by `search-handles`
- concrete redb, revision-store, Qdrant or process dependencies

## Logical primitives

- `RetentionRoot`, `RetentionLease`, `SweepGeneration`, `MarkManifest`, `ProtectionSet`, `DeletionReceipt`, `PurgeRequest`, `PurgeReceipt`, `PairedRecoveryManifest`, `RestoreDecision`, `LifecycleInvalidationSet`, `ObjectStoreAdminPort`, `ControlLifecyclePort`

## Logical operations

1. `collect_durable_roots(snapshot, ports) -> Result<ProtectionSet, RetentionError>`
2. `augment_with_active_pins(protection, pins) -> ProtectionSet`
3. `mark_reachable(roots, object_store) -> Result<MarkManifest, RetentionError>`
4. `sweep_unmarked(mark, object_store) -> Result<DeletionReceipt, RetentionError>`
5. `install_purge_fence(request, control) -> Result<PurgeFenceReceipt, RetentionError>`
6. `execute_purge(fence, ports) -> Result<PurgeReceipt, RetentionError>`
7. `validate_restore(manifest, live_identity) -> RestoreDecision`
8. `emit_invalidation_set(change) -> LifecycleInvalidationSet`

## Required invariants

- purge fence is durable/live before acknowledgement
- active publication, durable handle, legal hold and pin roots are never swept
- interrupted sweep resumes from recorded root generation and mark manifest
- purge tombstones block reindex/restore resurrection
- ordinary reclaim receipts cannot satisfy purge acknowledgement
- purge receipt distinguishes logical denial, index deletion, cache/CAS deletion, backup state and physical limitations
- all concrete storage/index operations occur behind ports

## Typed failure surface

- `PURGED`
- `RESTORE_PENDING_REVALIDATION`
- `RETENTION_ROOT_INCOMPLETE`
- `SWEEP_GENERATION_MISMATCH`
- `PURGE_PARTIAL`
- `SECURE_ERASE_NOT_GUARANTEED`

## Exit tests / evidence

- `active_pin_and_durable_handle_prevent_sweep`
- `interrupted_sweep_is_resumable`
- `membership_removal_preserves_shared_reachable_bytes`
- `purge_non_resurrection_after_restore`
- `mismatched_recovery_manifest_quarantined`
- `ordinary_reclaim_cannot_satisfy_purge_ack`
- `purge_receipt_never_overclaims_physical_erasure`
- `fake_ports_prove_no_concrete_adapter_dependency`

## Suggested internal modules

```text
search-retention/src/
  roots.rs
  leases.rs
  mark.rs
  sweep.rs
  purge.rs
  restore.rs
  invalidation.rs
  receipt.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- CAS retention, purge and restore stay together while one monotonic lifecycle policy owns them; split an independently replaceable backup/object provider by ADR.
