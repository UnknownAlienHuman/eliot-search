# search-index-reclaimer — delete after moving the exact effect

Current source is about 25 KiB of production Rust. It builds caller-shaped retired manifests/plans,
custom digests, checkpoints and receipts around one useful operation: exact Qdrant deletion/readback.
No production `execute_batch` caller or durable checkpoint owner is connected.

Read the [publication map](../search-publication/FUNCTIONS.md),
[view manager](../search-epoch-pins/FUNCTIONS.md), [implementation guide](../../../docs/implementation/README.md)
and local [source map](FUNCTIONS.md).

## Replacement

Publication persists finite `RetiredCleanup` rows when a committed view leaves physical work:

- same generation: exact IDs closed at `valid_until = N`, deletable only after no live same-generation view
  has sequence below N;
- replaced generation: one old collection, deletable only after no live view uses it.

The row points to immutable cleanup pages and carries the only effect/readback cursor. After eligibility,
publication calls the bridge's exact delete/absence readback. Unknown outcome keeps the row and retries.
Completed rows/pages disappear.

This is not a second publication journal. It is current unfinished external state, bounded and empty in
steady state.

## Important correction

Waiting for the immediate predecessor's last guard is insufficient. If v1 and v2 both reference a point and
v3 closes it, v1 can remain live after v2 is gone. Cleanup uses every live view's route/generation/sequence.
A sampled watermark or predecessor receipt cannot prove safety.

## Remove

Move any useful private exact-ID/collection delete/readback helper into publication or the Qdrant bridge,
then delete:

- `PublicationCommitProof` and caller retired-manifest validation;
- public plan/batch/checkpoint/receipt types and custom digests;
- separate operation ports/journal and sampled watermark inputs;
- P18/W7 reclaim/purge/restore interaction machinery;
- package/process/shape fixtures preserving those APIs;
- this package and workspace dependencies after no caller remains.

Ordinary cleanup makes no legal/security purge or physical-erasure claim. Unknown deletion leaks safely.

## Completion

A real publication commits a view and durable cleanup obligation in one transaction, old queries continue,
point/generation deletion waits for every relevant live view, crash cannot forget the cleanup row, exact
readback resolves ambiguous effects and no separate reclaimer state machine remains.
