# search-source-reconcile

Turn observations of selected roots into current source/head changes. The source owner retains bytes;
publication makes them searchable. Reconciliation owns neither another object store nor an event history.

Read the [implementation guide](../../../docs/implementation/README.md) and the
[single source/operation map](FUNCTIONS.md).

At `c0fcf294`, `reconcile_source` is a pure comparison with known branch defects, not a ready-to-connect
correct implementation. Root currentness also confuses directory availability, pending hints and completed
content work. The actual sync entrypoint still returns `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`.

FUNCTIONS records the exact defects, donor limitations and replacement rules. Startup/explicit/periodic
inventory is the baseline; a watcher is optional. An omitted path is not necessarily deleted, a valid entry
can carry an ignore error, and an exact retained view is not a snapshot of the live filesystem.

No runtime, whole-workspace build or product-readiness claim follows from this static audit.
