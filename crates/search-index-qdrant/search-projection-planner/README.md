# search-projection-planner

Turn prepared units and their vector products into point identities, expected
payload/readback data and a membership manifest. The result supports incremental
replacement, exact publication and later lookup of the original unit.

Current [planning](src/plan.rs) is pure and membership-scoped. It checks the complete
`VerifiedUnitSet` and required vectors, but returns a whole in-memory plan.
`PointSpec` retains vector digests, not vector values; the real upload path still
needs the corresponding encoder products.

The [manifest codec](src/manifest.rs) currently caps one membership at 4,096 entries
and 8 MiB despite larger `ProjectionBudget` defaults. Full reconstruction, digest
preimages and diffs allocate additional copies. These are current implementation
limits, not a product capacity claim or a format that must be preserved.

Use one ordered manifest, bounded pages/batches and the same prepared data through
the actual publication/bridge consumer. Keep complete membership and stable
unchanged-point identity while removing repeated maps, byte reconstruction and
whole-request digests. [FUNCTIONS.md](FUNCTIONS.md) maps the concrete changes.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [agent notes](AGENTS.md).
