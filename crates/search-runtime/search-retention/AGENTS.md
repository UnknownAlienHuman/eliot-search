# Maintenance implementation notes

This is a documentation-only directory. The old retention crate, lifecycle implementation and Cargo/feature
entries are removed. Do not add a new package, compatibility API or placeholder state machine here.

[FUNCTIONS.md](FUNCTIONS.md) owns the selected offline collection/archive/import behavior. Implement it through
the actual store/control owners; [publication](../../search-index-qdrant/search-publication/FUNCTIONS.md) owns
live Qdrant cleanup. Deletion of the old platform is not evidence that these operations exist.

Preserve original files, archives and sole retained copies. Run minimal pinned check/Clippy on changed production
owners and immediate consumers when implementing them. Record unavailable execution honestly; do not turn an
old library check, source inspection or archive marker into runtime qualification.
