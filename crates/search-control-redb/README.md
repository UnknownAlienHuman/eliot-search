# search-control-redb

Own compact typed current/pending/published metadata and coherent redb read snapshots. Source/prepared bodies
belong to immutable objects; Qdrant holds derived retrieval state; sessions own transient query windows.

[FUNCTIONS.md](FUNCTIONS.md) is the single schema/capture/commit/lookup recipe. Publication stages complete
metadata in bounded transactions before its small final current-pointer commit. Queries use the snapshot
already held by their published view, not a fresh latest transaction for each page of an old result.

No redb writer spans object/backend/process I/O. [Publication](../search-index-qdrant/search-publication/FUNCTIONS.md)
owns external effects and view installation; this package is not a second coordinator. Remove generic journals,
full copied snapshots, receipts and migration compatibility with actual callers. Preserve unsupported old roots
without writes. See [AGENTS.md](AGENTS.md) for the code-cut boundary.
