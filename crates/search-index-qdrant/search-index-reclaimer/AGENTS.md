# Working on search-index-reclaimer

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and local [cleanup map](FUNCTIONS.md).

This package is a consolidation/removal target. Ordinary Qdrant cleanup is the terminal part of one
publication operation after the relevant published-view guards drain.

Allowed work:

- move the useful exact ID delete/readback helper into publication or the bridge;
- bind every delete/readback to the actual route/generation/deadline;
- store only the unfinished cleanup cursor in the existing publication artifact;
- delete caller-created manifest proofs, watermarks, plan digests, checkpoints and receipt history;
- remove this package after callers switch.

Same-generation cleanup deletes exact retired IDs in bounded pages. A replaced generation deletes its whole
old collection after its last view guard. Timeout after possible deletion is resolved by readback with the
same operation; partial/foreign results block progress.

Do not build another pin permit, reclaim journal, security purge, source-deletion service or proof graph.
Verify old query survival, impossible old-view reacquisition after switch, last-guard cleanup and uncertain
same-route readback through real publication callers.
