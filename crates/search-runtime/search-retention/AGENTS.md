# Working on retention removal

Read [FUNCTIONS.md](FUNCTIONS.md) for the only local collection recipe and the
[publication map](../../search-index-qdrant/search-publication/FUNCTIONS.md) for Qdrant cleanup.

Local object collection is explicit stopped-root maintenance under the actual exclusive root guard. Mark
all typed roots once before deleting anything. Temporary mark data is discarded on exit/restart; no online
ObjectGcGate, generic lease/graph platform, purge, retention receipt or sweep journal is implemented.

Keep exact store enumeration/deletion, current user data and truthful failure handling. Move that small
operation to the concrete owner and delete this crate and obsolete consumers. Disk pressure pauses new
writes; it does not authorize optimistic deletion. Code first, minimal changed-owner check/Clippy afterward.
