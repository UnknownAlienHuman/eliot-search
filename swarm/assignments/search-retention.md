# `search-retention` implementation packet

**Path:** `crates/search-runtime/search-retention`  
**Capability:** C28  
**Delivery:** W7 / P13  
**Gate:** BLOCKED until W6 and all lifecycle dependency receipts are accepted  
**Trace:** S26, S28, H6.2, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-control-redb`, `search-revision-store`, `search-qdrant-bridge`, `search-epoch-pins`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Own retention, mark-and-sweep, purge and restore-revalidation decisions across Search-owned artifacts.

## Owns

- retention roots and leases
- crash-safe mark manifests and resumable sweep receipts
- purge fences/tombstones and truthful purge receipts
- paired recovery-manifest validation and restore quarantine

## Must not own

- claiming guaranteed physical secure erasure
- deleting client-owned canonical evidence
- refcount-only correctness
- restoring indexed service before identity, policy and source revalidation

## Logical primitives

- RetentionRoot, RetentionLease, SweepGeneration, MarkManifest, ProtectionSet, DeletionReceipt, PurgeRequest, PurgeReceipt, PairedRecoveryManifest, RestoreDecision

## Logical operations

1. `collect_durable_roots(snapshot) -> Result<ProtectionSet, RetentionError>`
2. `augment_with_active_pins(protection, pins) -> ProtectionSet`
3. `mark_reachable(roots) -> MarkManifest`
4. `sweep_unmarked(mark, inventory) -> Result<DeletionReceipt, RetentionError>`
5. `install_purge_fence(request) -> Result<PurgeFenceReceipt, RetentionError>`
6. `execute_purge(fence) -> Result<PurgeReceipt, RetentionError>`
7. `validate_restore(manifest, live_identity) -> RestoreDecision`

## Required invariants

- purge fence is observable before purge acknowledgement
- active publication, handle, legal-hold and pin roots are never swept
- interrupted sweep resumes from its recorded generation
- purge tombstones block reindex and restore resurrection
- receipt distinguishes logical denial, index deletion, cache deletion and physical limitations

## Typed failure surface

- `PURGED`
- `RESTORE_PENDING_REVALIDATION`
- `RETENTION_ROOT_INCOMPLETE`
- `SWEEP_GENERATION_MISMATCH`
- `PURGE_PARTIAL`
- `SECURE_ERASE_NOT_GUARANTEED`

## Exit tests / evidence

- `active_pin_prevents_reclamation`
- `interrupted_sweep_is_resumable`
- `membership_removal_preserves_shared_reachable_bytes`
- `purge_non_resurrection_after_restore`
- `mismatched_recovery_manifest_quarantined`
- `purge_receipt_never_overclaims_physical_erasure`

## Suggested internal modules

```text
search-retention/src/
  roots.rs
  leases.rs
  mark.rs
  sweep.rs
  purge.rs
  restore.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Purge and sweep remain together while purge dominates retention. Split restore acceleration only after a separately qualified recovery profile.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
