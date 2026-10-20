# Qdrant data settings disposition

`[qdrant_data]` is not registered by the current daemon. Merge real connection and data-plane values into the
single `[qdrant]` owner described by `qdrant_process.md`.

Possible concrete fields are endpoint, connect/request timeout and finite mutation/readback batch limits—only
when the bridge consumes them. `wait=true` plus exact relevant readback is publication behavior, not a user
correctness toggle. One-node baseline uses default/weak ordering; strong ordering is not a universal floor.
Collection schema/vector/filter/retrieval-profile changes use a new generation.

No section digest, qualification token, process-local mutation receipt or generic live-reload plan.
