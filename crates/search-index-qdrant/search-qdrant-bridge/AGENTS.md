# Working on search-qdrant-bridge

Read the [implementation spine](../../../docs/implementation/README.md),
[publication](../search-publication/FUNCTIONS.md) and local [FUNCTIONS.md](FUNCTIONS.md).

Use one daemon-owned workspace-pinned Qdrant client for publication and query. Implement only operations consumed
by the current vertical cut.

Baseline mutation rules:

- immutable points use insert-only creation;
- a retry uses the same fixed point bytes and exact readback;
- full upsert is not a retry primitive;
- the only in-place point mutation is the same fixed `valid_until` closure;
- individual points are never physically deleted;
- cleanup deletes whole never-reused retired generations;
- native inference values are observed Qdrant output, not client-known proof;
- explicit empty scope never widens.

Remove supervisor/qualification/lease ceremony, relative deadline reset and mutation ledger with their callers.
Vendor types and credentials stay private. After code run minimal Rust 1.98 compiler/Clippy for bridge plus actual
publication/query consumers, then focused native cases.
