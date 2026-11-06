# Root owner

The plain root implementation exposes read-only inspection, explicit initialization, current-root open and
strict discard of recognized empty interrupted initialization. All four operations receive the original
deadline and cancellation flag. A returned PlainRootGuard retains the stable root.lock handle for its lifetime
and exposes the root, object-directory and control-database paths.

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
No build, tests or native Windows qualification are claimed for this change.

[FUNCTIONS.md](FUNCTIONS.md) owns the API, classification, lock, initialization, publication and discard rules.
