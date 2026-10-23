# Working on maintenance/restore removal

Read [FUNCTIONS.md](FUNCTIONS.md) for the only local-object, archive and import recipes. Read
[publication](../../search-index-qdrant/search-publication/FUNCTIONS.md) for live Qdrant cleanup.

Do not integrate the existing retention crate, append-log GC or daemon restore composition. They encode leases,
purge planes, historical roots, same-installation restore, receipts and key migration that the target rejects.
Keep only concrete filesystem/object checks that a current owner actually uses.

Local object collection runs after the daemon stops under the real exclusive root guard. Mark every typed
current/published/pending/cleanup reference once before deleting. Query/health output never performs a recursive
storage inventory. Backup publishes a verified archive marker last; restore imports read-only archive bytes into
a fresh destination/generation and never executes copied pending/cleanup authority.

Protect original files, archives and sole retained copies. Code first for the selected cut, then minimal pinned
check/Clippy for changed owners and real consumers. Runtime/platform/restore claims require execution.
