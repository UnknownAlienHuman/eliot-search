# Publication source and implementation

This is a map of current Rust APIs and the work needed to make them one production
path. It does not mandate the present phase/receipt model.

## Current API

| Source API | Actual work |
| --- | --- |
| [`validate_prepared_generation`](src/admission.rs#L390) | Reads owner projections and observations, checks membership/unit scope, reconstructs manifests and builds a private `PreparedGeneration`. |
| [`PublicationCoordinator::submit`](src/machine.rs#L143) | Admits a proposal, occupies the local slot as `Prepared`, advances the local reservation and returns its epoch. |
| [`persist_intent`](src/machine.rs#L191) | Accepts an operation ID and receipt label and changes local state. It performs no durable write. |
| [`stage_new_points`, `close_old_points`](src/machine.rs#L212) | Check supplied exact-ID receipts against the admitted change set. They do not execute the mutations. |
| [`verify_readback`](src/machine.rs#L246) | Compares supplied digest fields with those receipts; it does not fetch or inspect Qdrant bytes. |
| [`commit_visible_epoch`](src/machine.rs#L282), [`publish_control_snapshot`](src/machine.rs#L316) | Accept supplied control/snapshot observations. Neither performs its named external effect. |
| [`emit_retired_manifest`](src/machine.rs#L337), [`complete`](src/machine.rs#L364) | Derive retired IDs after the accepted commit/snapshot and clear a successfully completed local transaction. |
| [`restore_inflight`](src/machine/restore.rs#L95), [`recover`](src/recovery.rs#L22) | Re-admit supplied preparation and decide from supplied recovery observations; no journal or index I/O occurs here. |
| [Compensation/abandon methods](src/machine.rs#L388), [abort finalization](src/machine/abort.rs#L138) | Account for supplied cleanup and control results. They do not provide a discard operation for `Prepared`. |

## Changes to make with callers

**Make local preparation cancellable.** `submit` occupies the slot before a real
intent exists. Compensation and abandon exclude `Prepared`; `complete` requires
`SnapshotPublished`. A cancellation between submit and persistence therefore
leaves this coordinator busy. Keep preparation as an ordinary local result until
the pending write is recorded, or add a simple discard of never-dispatched work.
Retain recovery state only for a persisted or possibly dispatched operation.

**Make the pending intent recoverable.** The shared `PublicationIntent` currently persists only a
`prepared_manifest_ref: ReceiptRef`; tests commonly use sentinel strings. The full old/new manifests and
diff remain in `PreparedGeneration` memory, and `persist_intent` merely accepts another receipt label.
A process restart therefore has no defined owner that resolves that string into the exact bounded work.

Write the prepared operation artifact before the redb intent references it. Store ordered manifest/diff
pages through the existing artifact owner, plus the physical route/generation, selected vector/profile,
old/new manifest roots, reserved epoch and the next bounded effect/readback cursor. redb keeps only the
small current/pending record and artifact reference; it must not contain source text, whole point batches
or model vectors. Recovery loads and validates those pages, reads Qdrant/control state and resumes or
classifies the next operation. An opaque label without a real resolver and retention rule is not durable
recovery evidence. Remove completed pages/checkpoints after the visible view and any retained old pins no
longer need them; do not recreate an eternal operation archive.

**Bound the actual representation.** The planner advertises 100,000 points and
64 MiB, while [manifest encoding](../search-projection-planner/src/manifest.rs)
uses the shared [4,096-item / 8 MiB limits](../../search-contracts/src/bounds.rs).
These cap one membership manifest, not the entire corpus. The coordinator's
1,000,000-point default cannot overcome them.
[`prepared_digest`](src/admission.rs#L501) then copies old and new canonical
manifests into one more 8 MiB preimage, so separately valid manifests can fail
together. Use a paged or streamed ordered manifest over bounded batches; do not
solve this by increasing a global codec limit.

**Keep each projection once.** [Planner reconstruction](../search-projection-planner/src/plan.rs)
and [diffing](../search-projection-planner/src/diff.rs) materialize/copy full inputs;
admission reconstructs them again and builds more ID maps. Reuse the checked
projection and walk sorted entries, as the existing
[`changed_point_id_is_reused`](src/machine.rs#L556) already does. Remove the
whole-preparation digest when actual content identity and the pending operation
already identify the work. Check cancellation between bounded pieces of real work;
the current admission checkpoints bracket whole-manifest processing.

**Wire actual effects once.** Use the existing
[redb publication owner](../../search-control-redb/src/persistent/publication.rs)
and [Qdrant bridge](../search-qdrant-bridge/src/real/mutations.rs) through daemon
composition. A real committed view and unresolved index changes determine recovery.
Caller-matched receipt fields cannot establish that either effect happened.
Keep useful shared logic while removing the daemon's duplicate machine; do not
turn the API table above into another mandatory orchestration chain.
