# Reclaim source and implementation

The current public records and helpers are implementation material. Their names
do not establish durable commit, query lifetime or crash recovery.

## Current API

| Source API | Actual work |
| --- | --- |
| [`validate_retired_manifest(manifest, publication)`](src/manifest.rs#L71) | Checks nonempty sorted unique IDs and matching fields in a supplied `PublicationCommitProof`. It neither hashes those IDs nor reads committed control state. |
| [`plan(manifest, watermark, settings, budget)`](src/plan.rs#L64) | Checks supplied pin counts and size limits, keeps the whole manifest, hashes the plan and copies IDs into all batches. |
| [`execute_batch(plan, batch_index, admin, mutation)`](src/execute.rs#L122) | Checks the operation ID, calls exact delete, then exact readback; absence can resolve an unknown delete. It receives no context, deadline or current pin state. |
| [`checkpoint(plan, receipts)`](src/receipt.rs#L90) | Validates and stores full completed receipts in an in-memory map. No write to storage occurs. |
| [`resume(checkpoint, plan, watermark)`](src/receipt.rs#L115) | Rechecks supplied fields/counts and returns cloned unfinished batches. It does not execute or reopen anything. |
| [`complete(plan, receipts)`](src/receipt.rs#L169) | Checks coverage of all planned IDs and returns an ordinary-reclaim summary. |

## Replace the missing effects and redundant records

**Read committed retirement once.** The validator compares two caller-supplied
digest/receipt labels; changing the ordered point-ID list without changing those
labels still passes its checks. Obtain the exact list from the actual committed
publication/manifest owner and remove the parallel `PublicationCommitProof`.
A third proof object would leave the same missing connection.

**Join final drain to deletion.** The daemon's
[`authorize_reclaim`](../../../bins/eliot-searchd/src/rebuild_composition/kernel/reclaim.rs#L32)
samples pins once. A later `execute_batch` does not recheck them; `resume` only
accepts another unbound watermark value. Coordinate old-view admission and final
guard release with [the pin owner](../search-epoch-pins/FUNCTIONS.md), then execute
the committed list. Add the actual maintenance caller and adapter; existing
`IndexAdmin` implementations are tests, not an installed background worker.

**Keep one list and a position.** IDs currently remain in the manifest, batch
vectors and each completed receipt; resume clones the unfinished batch vectors
again. Read bounded slices from one committed retirement list. Repeating deletion
of the same retired IDs and checking their absence is enough to retry; persist a
compact position only when needed to avoid repeated work. Discard completed
per-batch receipts and the custom plan digest when they serve no remaining retry
purpose. Preserve the unfinished list across restart.

**Use one bounded Qdrant operation path.** The already adopted
[Qdrant client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs)
provides `delete_points` and `get_points`. The existing
[bridge delete](../search-qdrant-bridge/src/real/mutations/delete.rs#L13) uses an
exact ID selector, `wait=true`, and a following `GetPoints`. The
[live fixture adapter](tests/live_rebuild.rs#L191) calls that method and then the
reclaimer requests another readback. Consolidate this into one exact absence
check, including the path where delete timed out and may already have happened.
Use the caller's remaining deadline across calls and check cancellation between
bounded batches. SDK acknowledgement alone does not settle an unknown operation.

These are source-level integration findings; this documentation does not claim
a live deletion incident or a completed Rust fix.
