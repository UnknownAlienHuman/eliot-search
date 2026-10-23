# Working on source reconciliation

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [local recipe](FUNCTIONS.md).
FUNCTIONS owns observation, selection, freshness and scheduling rules; do not copy another algorithm here.

Trace the actual source reader/current-row writer and sync entrypoint. The old comparator is not correct
merely because it is pure: its missing-root check, combined key/content update, same-path identity policy
and failure reasons need the recorded cutover. Directory availability and a nonempty observation receipt
do not establish file inventory or completed synchronization.

Use one daemon scheduler and current typed rows. Startup/explicit/periodic reconciliation works before the
optional watcher. Keep root IDs stable across catalog changes, preserve newly arriving dirty work, and use
captured current-row/registration checks when applying results. Do not build a watcher journal, supervisor,
lease, accepted-observation wrapper or receipt archive.

Use the selected donor with its actual error/filter behavior. Preserve useful current-scope and gap semantics
from the old design, not its port/API catalogue. Inspect both iterator errors and entry-attached errors;
never treat a filtered omission as confirmed physical absence or a dirty path as a user override.

Apply the source identity and preparation owners' rules rather than recreating them. Remove obsolete ledger,
receipt-only inputs and their exclusive fixtures with callers. Minimal pinned compiler/Clippy follows code;
behavior checks follow the implementation, and every result records the exact revision/targets.
