# Root owner

The plain root implementation exposes read-only inspection, explicit initialization, current-root open and
strict discard of recognized empty interrupted initialization. All four operations receive the original
deadline and cancellation flag. A returned PlainRootGuard retains the stable root.lock handle for its lifetime
and exposes the root, object-directory and control-database paths.

The separate open_plain_with_recovery capability permits maintained CONTROL engine recovery under that guard.
Index is its current consumer; watch owns successive Index children. It validates a recovered owned temporary
copy before writable recovery of the rechecked original, then clean-closes/inspects. It never resets or copies
candidate bytes back. Ordinary inspection/open/query remain non-repairing. See
[guarded writable open](FUNCTIONS.md#guarded-writable-open).

Initialization writes the exact init marker before stores and publishes the final manifest last through a
same-parent tempfile no-clobber stage. The control database is initialized and validated by
search-control-redb at control/control.redb; the root owner does not parse redb bytes. Current-root
validation checks only the readonly format/version and does not enumerate tables, probe emptiness, cap database
size, or scan retained object contents. The bounded emptiness probe is used only for unfinished initialization
while the root lock is held.

The owner recipe defines the two fixed metadata stage basenames and their exact byte/layout rules. Explicit
initialization can retry interrupted marker/manifest writes and clean redundant completed stage links; unknown
files remain untouched. Incomplete control validation uses the empty-only probe directly so a proven empty
preformat database can be finished only by explicit initialization under the root lock and exact marker.

Only src/lib.rs and src/plain_root.rs remain in this crate's source. The legacy lifecycle modules and their
inline fixtures are physically deleted; the direct dependencies are search-control-redb and tempfile.
Root and later connected-command qualification scopes are recorded in the
[registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status). Q1 uses this same guard through view/output;
the [operator-storage guidance](FUNCTIONS.md#operator-qdrant-storage) records the bounded Windows observation.

[FUNCTIONS.md](FUNCTIONS.md) owns the API, classification, lock, initialization, publication and discard rules.
