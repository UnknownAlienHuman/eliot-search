# Working on search-retention

This package is a legacy removal target. Read the root
[architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).

Allowed work:

- switch the last real caller to current/pending/cleanup/live-view lifetime;
- add the small process-local object-GC gate in the actual immutable object/control owner;
- move useful exact enumeration/delete helpers to their real owner;
- delete lease, sweep, purge, restore, migration, fixture and feature code;
- keep unsupported old roots untouched.

Object producers hold a shared GC guard from final object publication/reuse through their first durable redb
reference. Bounded GC holds the exclusive guard while snapshotting current source/prepared, current view,
pending publication, finite cleanup and live-view roots and deleting one exact unreferenced page. Unknown
roots/inventory/deletion retain bytes.

Qdrant cleanup belongs to publication's finite `RetiredCleanup` rows. Point IDs wait for every older live
same-generation view; old collections wait for every view using them. Crash drops interactive guards but not
cleanup obligations.

Do not add a durable lease catalogue, generic graph/sweep journal, legal-hold/security-purge planes,
tombstones, live restore/root cutover/re-encryption, new proofs/receipts/packages or compatibility readers.
The safe failure is a leak and retry.

After code, run minimal Rust 1.98 compiler/Clippy for object/control/publication/GC consumers and focused
finalize-reference/GC race, restart and malformed-root behavior. Delete historical lifecycle tests with the
APIs instead of maintaining them.
