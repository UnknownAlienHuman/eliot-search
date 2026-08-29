# P18 advanced-scale supplement — `search-index-reclaimer`

**Status:** blocked. Existing exact retired-point reclaim contract remains normative.

## Required operations

```text
validate_retired_route_manifest(manifest, route_commit) -> CommittedRetiredRouteManifest
plan_old_route_reclaim(manifest, drain_snapshot, settings) -> RouteReclaimPlan
execute_old_route_batch(plan, batch, index_admin, context) -> RouteReclaimBatchReceipt
resume_old_route_reclaim(checkpoint, current_drain, index_admin, context) -> RouteReclaimReceipt
complete_old_route_reclaim(plan, receipts, final_readback) -> RouteReclaimReceipt
```

## Invariants

- only exact collection/point manifests from a committed guarded route switch are accepted;
- every old-route pin/drain watermark must permit deletion;
- current/candidate/rollback-protected routes are never deleted;
- timeout after delete is resolved by exact readback under stable operation identity;
- ordinary route reclaim cannot satisfy purge, CAS, backup or secure-erase receipts;
- rollback protection keeps the retained baseline route until the accepted plan releases it.

## Required evidence

Pinned old route never deletes; exact bounded batches; timeout/partial/conflict recovery; crash/resume;
rollback-retained route protection; final exact absence verification; ordinary-reclaim receipt separation.
