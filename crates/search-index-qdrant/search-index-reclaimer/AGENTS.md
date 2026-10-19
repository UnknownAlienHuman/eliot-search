# Working on search-index-reclaimer

Read the root [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [cleanup map](FUNCTIONS.md),
[publication map](../search-publication/FUNCTIONS.md) and
[view manager](../search-epoch-pins/FUNCTIONS.md).

This package is a consolidation/removal target. Move only the useful exact Qdrant ID/collection delete and
absence-readback helper into publication or the bridge.

Do not repair caller-created manifest proofs, plan digests, watermarks, checkpoints, receipts or the separate
operation journal. Publication persists one finite `RetiredCleanup` row per genuinely unfinished external
delete and removes it after completion.

Eligibility must inspect every live view:

- point IDs closed at sequence N wait until no live same-route/generation view has sequence < N;
- a collection waits until no live view uses its route/generation.

Waiting for the immediate predecessor's last guard is wrong when an earlier view remains live. Restart drops
old guards but keeps cleanup rows, so work is not forgotten.

Bind every delete/readback to the exact route/generation and one absolute context. Unknown outcome keeps the
same row/cursor and re-observes actual state. Broad-filter deletion, legal/security purge and secure-erasure
claims remain absent.

Delete this package and its P18/W7/shape/process fixtures after callers move. After code, run minimal Rust
1.98 compiler/Clippy for publication/bridge/control/view consumers and focused threshold/restart/unknown-
outcome cases.
