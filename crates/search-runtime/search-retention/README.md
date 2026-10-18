# search-retention — legacy lifecycle package

This package is not the target owner for the new application.

Current source contains a process-local lease catalogue plus general CAS sweep, multi-plane purge and
restore coordination. The normal daemon does not connect those mechanisms; restore composition is behind
`wave7-lifecycle` and marked dead code, while purge/sweep are mainly exercised by their own fixtures.
The package therefore preserves a historical architecture rather than completing the local search path.

## Replacement

- Current/pending published views and live query-window guards keep manifests and referenced artifacts.
- Publication owns exact retired-point or retired-generation cleanup after old guards drain.
- Source/control stores delete immutable artifacts only when current, pending and pinned views no longer
  reference them.
- User source removal is a new published view followed by ordinary GC, not a security-purge protocol.
- Baseline backup/restore is an offline copy of one closed current-format root; Qdrant rebuilds.

See [implementation](../../../docs/implementation/README.md),
[published-view guards](../../search-index-qdrant/search-epoch-pins/FUNCTIONS.md) and
[index cleanup](../../search-index-qdrant/search-index-reclaimer/FUNCTIONS.md).

## Removal boundary

After active callers adopt the view model, remove this crate, `wave7-lifecycle`, daemon
`restore_composition`, old lifecycle process fixtures/config packets and module-shape guards. Preserve
original user data and any useful exact-delete/readback helper in its real owner. Do not replace the package
with another retention, legal-hold, purge or migration framework.
