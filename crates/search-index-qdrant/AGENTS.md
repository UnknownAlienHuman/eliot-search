# Working on indexing and publication

Read the [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Qdrant stores rebuildable projections; source
bytes and committed control state remain with their existing owners.

Use one publication transition owner and one pending record for real unfinished external writes.
Preparation is local until that intent is durably recorded and can be discarded beforehand. Remove
duplicate daemon/bridge journals, supplied commit-proof objects and permanent completed-operation
history. Do not replace actual write/readback/commit observations with booleans or hashes.

Publish a coherent view and admit query pins against it. Reclamation obtains exact retired IDs from
committed state and excludes active readers; snapshotting pins once does not prevent later admission
of an unsafe old pin. Reuse the existing guarded pin path and retire unguarded alternatives.

The supervisor owns the real local Qdrant process; the bridge owns SDK transport/translation. Keep
those effects concrete. A planner/point identity/pin/reclaimer name does not require a permanent crate,
new receipt protocol or independent transaction engine.
