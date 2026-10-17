# Function contract — `search-publication`

**Status:** vendor-neutral phase owner with typed effect/readback ports and private
validated proofs. Root supplies the real durable-control and Qdrant adapters.

At most one publication transaction is active globally. Prepared work may run concurrently, but a later
epoch cannot enter staging while an earlier intent is unresolved.

## Core operations

The signatures below describe logical operations. Concrete phase methods take
`PublicationEffectPort`, `PublicationReadbackPort` and the original operation
context. Readbacks are untrusted observations; the owner checks complete expected
inputs and readback before constructing a private proof. Uncertain effects retain
their original operation and must be reconciled before another phase can run.

### `submit(prepared, ports, context) -> Result<PublicationReceipt, PublicationError>`

Validates owner/source/membership/access/shadow/purge/profile guards and the exact immutable projection
manifest before reserving the next epoch.

### `persist_intent(prepared, next_epoch, control, mutation) -> Result<PublicationIntent, PublicationError>`

Durably records `INTENT_DURABLE` with exact manifest refs and operation identity. The epoch is consumed
even if later abandoned and is never reused.

### `stage_new_points(intent, index, context) -> Result<StageReceipt, PublicationError>`

Upserts exact new IDs at `valid_from=N` with no upper bound, then performs exact readback.

### `close_old_points(intent, index, context) -> Result<ClosureReceipt, PublicationError>`

Sets `valid_until_epoch_exclusive=N` on the exact old manifest ID list and verifies every closed ID and
count. Broad-filter closure is structurally unavailable.

### `verify_readback(intent, staged, closed) -> Result<ReadbackVerified, PublicationError>`

Checks IDs, full identity digests, payload/vector digests, epoch fields and absence of unexpected points.

### `commit_visible_epoch(verified, guards, control) -> Result<ControlCommit, PublicationError>`

One control compare-and-swap revalidates all load-bearing generations and atomically changes visible
epoch/manifest refs/shadows/receipt. Qdrant aliases are not the linearization point.

### `publish_control_snapshot(commit, snapshot_port, context) -> Result<SnapshotPublishReceipt, PublicationError>`

Acknowledgement waits for immutable in-memory snapshot publication. Failure is fail-closed and recovered
from the committed journal state.

### `emit_retired_manifest(commit) -> RetiredPointManifest`

Produces the exact committed retired-ID manifest consumed by the reclaimer. No physical deletion occurs
here.

## Recovery and doctor operations

### `recover(active_intent, ports, context) -> Result<PublicationRecoveryDecision, PublicationError>`

Uses durable state and exact readback to complete, compensate exact IDs, commit invalidation-only or
enter `PUBLICATION_BLOCKED`. It never guesses from collection presence.

### `compensate_exact(intent, index, context) -> Result<CompensationReceipt, PublicationError>`

Operates only on IDs recorded by the intent/manifests and verifies the resulting exclusion.

### `abandon(intent, fence, control) -> Result<AbandonReceipt, PublicationError>`

Allowed only after an `AbandonedPublicationFence` excludes the complete affected membership/partition
before retrieval and IDF. The skipped epoch remains unusable.

## Collection-generation migration

`create_candidate`, `build_base_at_r0`, `catch_up_change_log`, `enter_final_barrier`,
`validate_candidate_at_r1`, `commit_route_switch`, `drain_old_route`, and `mark_old_route_reclaimable`
implement the S13.7 state machine. Route switch is one guarded control transaction; active route pins
keep the old collection alive.

## Idempotency, cancellation and crash semantics

Each external mutation has one durable operation identity. Replaying identical input is safe; differing
input with the same identity is rejected. Cancellation before intent creation has no effect. After
intent durability, cancellation yields a recoverable unresolved intent, never silent rollback.
The package fixtures exercise pure phase/recovery behavior with synthetic ports.
Actual journal restart, lost-acknowledgement and Native/Qdrant qualification require
the integrated real adapters; fixture success does not establish those results.

## Required fixtures

Full H11/S13 kill matrix; guard race; exact readback mismatch; skipped epoch non-reuse; snapshot
publication failure; exact compensation; abandon fence; invalidation-only path; migration catch-up and
route-pin drain; fake journal/index ports proving adapter independence.
