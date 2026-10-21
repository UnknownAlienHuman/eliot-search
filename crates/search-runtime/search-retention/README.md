# search-retention — remove the lifecycle platform

The package's generic leases, graph sweep, purge and restore protocols are not the local product.
The selected replacement is one explicit stopped-root maintenance pass described in [FUNCTIONS.md](FUNCTIONS.md).
It marks current/published/pending/cleanup objects once and only then deletes exact unreferenced objects.

This supersedes the proposed per-page ObjectGcGate/root-rescan algorithm. No online local object GC is claimed.
Qdrant cleanup is still owned by [publication](../../search-index-qdrant/search-publication/FUNCTIONS.md), which
waits for old live views and retains unfinished deletion state. Do not merge these two lifetimes into a
retention service. Source bodies and sole retained copies remain user data.
