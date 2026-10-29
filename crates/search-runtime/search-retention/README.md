# Former retention package — implementation notes only

R017 removed the generic lease/purge/sweep/restore implementation. R020 removes its empty crate, workspace
membership, daemon dependency, lockfile record and `wave7-lifecycle`/`full-baseline` features. This directory now
contains documentation only; it is not a Cargo package or a placeholder for another service.

[FUNCTIONS.md](FUNCTIONS.md) retains the single recipe for stopped-root local-object collection, completed
archives and fresh-destination import. Existing links remain valid. These operations are still unimplemented;
removing the old platform does not deliver them. Implement them with the actual store/control/publication owners.
[Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md) owns live Qdrant cleanup.

Do not recreate a crate, public compatibility types or a lifecycle registry here. Preserve original files,
archives and sole retained copies.
