# `search-index-reclaimer` implementation packet

**Path:** `crates/search-index-qdrant/search-index-reclaimer`  
**Capability:** C17 reclaim executor  
**Delivery:** W3 / P07; purge interaction hardening W7  
**Gate:** BLOCKED until publication-manifest and epoch-pin handoffs are accepted  
**Trace:** S13.7, S14.2-S14.3, H11-H12, P07, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-epoch-pins`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Reclaim ordinary retired index points only when a committed exact-ID manifest and every active pin watermark prove deletion safe.

## Owns

- committed retired-point manifest validation
- route/epoch watermark eligibility
- bounded exact-ID delete planning
- idempotent batch receipts and crash-safe resume cursor
- distinction between ordinary reclaim and monotonic security purge

## Must not own

- deciding publication visibility or retiring points
- acquiring/extending query pins
- broad payload-filter deletion on correctness paths
- deleting current, uncommitted or pinned points
- CAS/object deletion or purge acknowledgement
- direct dependency on Qdrant vendor types

## Logical primitives

- `RetiredPointManifest`, `ReclaimWatermark`, `ReclaimPlan`, `ReclaimBatch`, `ReclaimCheckpoint`, `ReclaimReceipt`, `IndexAdminPort`

## Logical operations

1. `plan(manifest, watermark, budget) -> Result<ReclaimPlan, ReclaimError>`
2. `execute(plan, index_admin) -> Result<ReclaimReceipt, ReclaimError>`
3. `resume(checkpoint, index_admin) -> Result<ReclaimReceipt, ReclaimError>`
4. `verify_receipt(plan, receipt) -> Result<(), ReclaimError>`

## Required invariants

- manifest must be committed and bind exact retired point IDs
- current or pinned route/epoch is never deleted
- broad-filter correctness deletion is structurally unavailable
- batch replay is idempotent and unexpected/missing acknowledgements fail closed
- security purge never treats an ordinary reclaim receipt as purge completion

## Typed failure surface

- `RECLAIM_PINNED`
- `RECLAIM_MANIFEST_UNCOMMITTED`
- `RECLAIM_READBACK_MISMATCH`
- `RECLAIM_INCOMPLETE`
- `INDEX_ADMIN_UNAVAILABLE`

## Exit tests / evidence

- `current_or_pinned_epoch_never_deleted`
- `exact_manifest_ids_only`
- `crash_between_batches_resumes_idempotently`
- `old_route_waits_for_final_pin`
- `unexpected_delete_receipt_fails_closed`
- `ordinary_reclaim_cannot_satisfy_purge_ack`

## Suggested internal modules

```text
search-index-reclaimer/src/
  manifest.rs
  watermark.rs
  plan.rs
  execute.rs
  checkpoint.rs
  receipt.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Pin ownership remains in `search-epoch-pins`; publication ownership remains in `search-publication`.
