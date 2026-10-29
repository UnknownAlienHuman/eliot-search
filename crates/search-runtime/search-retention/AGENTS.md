# Maintenance implementation notes

[FUNCTIONS.md](FUNCTIONS.md) owns offline collection/archive/import. Implement these operations through the
actual store/control/root owners; [publication](../../search-index-qdrant/search-publication/FUNCTIONS.md)
owns live Qdrant cleanup. Do not add another lifecycle service or compatibility facade.

The old crate still exists on the inspected main; its removal belongs to the unmerged refactor.
Read the [source boundary](FUNCTIONS.md#historical-findings-and-remaining-legacy-code), inspect actual callers,
and remove displaced code with its consumers. Neither its continued presence nor its deletion implements the target.

Preserve original files, archives and sole retained copies. After production changes run minimal pinned
check/Clippy for the changed owners and immediate consumers; record only execution that actually ran.
