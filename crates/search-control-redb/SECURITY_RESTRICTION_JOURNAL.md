# Historical security-restriction journal

This file described a second policy/restriction state machine layered onto the generic control journal,
including complete denied/purged membership sets, dependent generations, receipts and owner succession.
It is not a baseline local-product requirement.

The trusted local application resolves an explicit non-widening source scope from one pinned
`PublishedView`. It does not persist a security command, purge transcript or membership denial set for
every local change or query. A selected restricted-client adapter owns its actual allowed source set,
limits, revocation and connection policy; publication removes or excludes affected points through the same
view/Qdrant owner.

Do not move these rows into a new manifest merely to preserve the old API. Remove the restriction codec,
paired access-policy transaction, operation identity/receipt recovery and exclusive fixtures with their
last callers. Keep only concrete scope enforcement required by an enabled restricted adapter.

See [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md) and
[`search-access`](../search-query/search-access/README.md).
