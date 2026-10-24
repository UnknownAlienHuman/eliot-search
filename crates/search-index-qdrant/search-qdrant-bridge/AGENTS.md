# Working on search-qdrant-bridge

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [local recipe](FUNCTIONS.md).
The local recipe owns mutation, schema, scope and readback rules; do not copy another version here.

Use the workspace-pinned SDK privately. Trace the real publication/query caller before adding an operation.
One daemon-owned client/runtime serves both paths. Do not build an SDK-method catalogue, a second write
runtime, an acceptance-token replacement or a forwarding facade around the retired interfaces.

`QualifiedGate` has already been removed from runtime construction. Keep useful live probes as diagnostic
behavior, not an activation prerequisite. The remaining supervisor/lease ceremony, fixed build admission,
relative deadlines and mutation ledger are separate unfinished cuts; removing the gate did not remove them.

Published retrieval state is immutable under the publication recipe. Known empty scope is not unknown scope.
Observed point state is not proof that a timed-out writer has stopped. Native inference coefficients are not
known client-computed vectors. Follow the concrete distinctions in FUNCTIONS rather than weakening them to
satisfy old fixtures or restoring the former README's generic upsert/update list.

After a source change, run minimal pinned compiler/Clippy for the changed owner and actual consumers. Record
which revision/targets ran in the analysis registry; do not reuse an older successful run as evidence for a
new cut. Broad live/native/recovery work follows the implementation it exercises.
