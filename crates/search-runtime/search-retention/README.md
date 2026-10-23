# search-retention — implementation removed

R017 removed the generic lease/purge/sweep/restore implementation and daemon `restore_composition`. The crate now
exports no API. Its manifest, workspace member and lockfile record remain only until a dedicated Cargo pruning cut
can update all three together.

The selected behavior is defined in [FUNCTIONS.md](FUNCTIONS.md): one stopped-root local-object mark/sweep, one
completed offline archive and one fresh-destination import. Qdrant cleanup remains with
[publication](../../search-index-qdrant/search-publication/FUNCTIONS.md).

Do not reconstruct the removed platform, add forwarding types or treat this empty package as the owner of new
code. Implement the small operations in the real store/control/publication owners, then remove this package,
`wave7-lifecycle` and its stale dependency/lock entries. Original files, archives and sole retained copies remain
user data.