# Function contract — `search-index-reclaimer`

**Status:** W3/P07 ordinary index-reclaim contract.

This package deletes rebuildable retired Qdrant points. It does not perform CAS retention, legal/security
purge, publication visibility or secure erase.

## Operations

### `validate_retired_manifest(manifest, publication_receipt) -> Result<CommittedRetiredManifest, ReclaimError>`

Requires exact IDs, collection generation, retirement epoch, manifest digest and a matching committed
publication receipt. Uncommitted or broad-predicate manifests are rejected.

### `plan(manifest, watermark, settings, budget) -> Result<ReclaimPlan, ReclaimError>`

Requires the route/epoch watermark to prove invisibility to every active pin. Produces deterministic,
finite exact-ID batches and a plan digest.

### `execute_batch(plan, batch_index, index_admin, mutation, context) -> Result<ReclaimBatchReceipt, ReclaimError>`

Deletes only the batch's exact IDs using the index-admin port, then verifies expected missing IDs and
rejects unexpected/mismatched acknowledgements.

### `checkpoint(plan, receipts) -> ReclaimCheckpoint`

Records only plan/manifest digests, completed batch indices and exact receipts. It never stores source
content or a broad deletion filter.

### `resume(checkpoint, manifest, watermark, index_admin, context) -> Result<ReclaimReceipt, ReclaimError>`

Revalidates manifest, publication receipt and current watermark before continuing. Already confirmed
batches are idempotently skipped/read back.

### `complete(plan, receipts) -> Result<ReclaimReceipt, ReclaimError>`

Succeeds only when every exact ID is confirmed absent. The receipt is explicitly
`ordinary_retired_point_reclaim` and cannot satisfy a purge acknowledgement.

## Configuration operations

Implements `config/sections/index_reclaim.md`. Batch/slice tuning is live and bounded, but never alters
manifest or watermark eligibility.

## Cancellation, retry and crash semantics

Cancellation stops between batches and returns a resumable checkpoint. A timeout after delete is an
unknown outcome resolved by exact readback. Same batch mutation identity plus same IDs is retry-safe;
different IDs with the same identity are rejected.

## Required fixtures

Pinned/current/uncommitted never delete; exact IDs only; crash/timeout between batches; duplicate
replay; unexpected receipt fail-closed; old route waits for final pin; ordinary receipt cannot satisfy
security purge.
