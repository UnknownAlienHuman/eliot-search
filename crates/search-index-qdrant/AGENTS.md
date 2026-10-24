# Working on indexing and publication

Read the [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Qdrant stores rebuildable projections; source bytes
and committed control state remain with their actual owners.

Connect one configured Qdrant endpoint through one bridge/runtime before adding process management. The
current supervisor is disconnected and cannot itself apply the Windows containment it claims. Treat managed
Qdrant as an optional convenience: replace the supervisor with a small actual child/platform owner or delete
it. Artifact/probe/secret/containment receipts are not runtime authority.

Use one publication owner and one pending record for real unfinished external writes. Preparation is local
until the intent is durably recorded. Remove duplicate daemon/bridge journals, supplied commit proofs and
completed-operation history.

Publish one coherent immutable view. Queries can pin only current and windows clone that guard. Fold exact
point/generation cleanup into publication; delete sampled pin/reclaimer/retention platforms. Unknown external
delete leaks safely and retries.

A planner, point-identity, bridge, publication or view-manager name does not require a permanent crate. Keep
a boundary only for a real dependency/effect or substantial algorithm. Change direct callers and delete the
replaced mechanism in the same slice.
