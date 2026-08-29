# Agent contract — search-index-reclaimer

You own only `crates/search-index-qdrant/search-index-reclaimer/`. Do not edit another package, the
root workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S13.7, S14.2, H11-H12, P07, P13.

## Mission

Reclaim ordinary retired index points only when a committed exact-ID manifest and every active pin
watermark prove deletion safe.

## Ownership

- validation of committed retired-point manifests
- route/epoch watermark eligibility
- bounded exact-ID deletion plans
- idempotent batch receipts and crash-safe resume cursor
- separation of ordinary reclamation from security purge

## Forbidden ownership

- deciding publication visibility or retiring a point
- acquiring or extending query pins
- deleting by broad payload filter on a correctness path
- deleting an uncommitted, current or pinned point
- CAS/object-store deletion
- acknowledging purge or secure erase

## Allowed dependencies

`search-contracts`, `search-domain`, `search-epoch-pins`. Index deletion is invoked through an injected
vendor-neutral `SearchIndexAdminPort`; this crate must not depend on `search-qdrant-bridge`.

## Required logical surface

- `plan_reclaim(manifest, watermark, budget) -> Result<ReclaimPlan, ReclaimError>`
- `execute_reclaim(plan, index_admin) -> Result<ReclaimReceipt, ReclaimError>`
- `resume_reclaim(checkpoint, index_admin) -> Result<ReclaimReceipt, ReclaimError>`
- `verify_reclaim_receipt(plan, receipt) -> Result<(), ReclaimError>`

## Failure surface

Relevant reasons include `RECLAIM_PINNED`, `RECLAIM_MANIFEST_UNCOMMITTED`,
`RECLAIM_READBACK_MISMATCH`, `QDRANT_UNAVAILABLE` and `RECLAIM_INCOMPLETE`.

## Test seams and exit evidence

- `current or pinned epoch is never deleted`
- `exact manifest IDs only; broad filter path is impossible`
- `crash between batches resumes idempotently`
- `route migration waits for final old-route pin`
- `missing/duplicate/unexpected delete receipt fails closed`
- `security purge does not reuse ordinary reclaim acknowledgement`

## Size and split guard

- Delivery wave: **W3 / P07; purge interaction hardened W7**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Keep pin ownership in `search-epoch-pins` and publication ownership in `search-publication`.

## Definition of done

Every deletion is justified by committed exact IDs plus a safe watermark, resumes after faults, and
cannot be confused with purge or publication.
