# Maintenance — target and legacy source

[FUNCTIONS.md](FUNCTIONS.md) owns stopped-root local collection, non-runnable archives and fresh-destination
import. These are target operations, not capabilities established by the old generic retention crate.
[Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md) owns online Qdrant generation cleanup.

At main `623160a7`, `search-retention` still appears in Cargo. R017/R020 removed it only on the unmerged
refactor branch. Follow the recipe's source boundary and actual callers; do not treat this directory as empty
or preserve its lease/purge/restore platform when implementing the replacement.

Keep useful behavior with real store/control/root consumers. Preserve original files, archives and sole copies.
