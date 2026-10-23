# search-retention — delete the lifecycle platform

The generic lease/purge/sweep/restore crate is not the local product. The selected concrete behavior is in
[FUNCTIONS.md](FUNCTIONS.md): one stopped-root local-object mark/sweep, one completed offline archive and one
fresh-destination import. Qdrant cleanup remains with
[publication](../../search-index-qdrant/search-publication/FUNCTIONS.md).

Do not reuse the legacy append-log GC, restore phases, purge receipts or key-migration machinery. The old GC
keeps every revision ever recorded yet misses preparation/publication roots; the old restore expects the same
physical installation. Normal query output must not run their full storage inventories.

Move only useful filesystem/object safety primitives into the concrete store/control owner, then remove this
crate, daemon `restore_composition`, `wave7-lifecycle` and exclusive fixtures. Original files, archives and sole
retained copies remain user data.
