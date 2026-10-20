# Working on search-eliot-adapter

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), local [source audit](FUNCTIONS.md) and optional
issue [#219](https://github.com/UnknownAlienHuman/eliot-search/issues/219).

This package has no public-main caller and duplicates binding/grant/replay/terminal lifecycle already being
removed from the provider stack. Do not connect or harden `EliotAdapter`, payload digests, terminal receipts,
drain/quarantine or exact-eleven-recipe mapping. Delete the package if no real shared pure mapping remains.

MCP is optional and follows a working standalone Search application. Use the official `rmcp` SDK in one
stdio-only tools leaf and the same current `search-provider-client::LocalClient` as the CLI. Expose only real
`Search`, `Exact`, `Continue` and `Expand` handlers; add optional operations only after their source-backed
handlers exist.

The leaf owns only MCP lifetime, finite tool schemas and request/result mapping. It never owns source scope,
grants, capabilities, Qdrant/redb/object state, planning, ranking, windows or another session ledger. MCP
cancellation maps to one live LocalClient request; daemon windows remain daemon-owned.

Do not enable HTTP/OAuth/auth/resources/prompts/sampling/elicitation/Tasks/child-process/client transports or
raw filesystem/Qdrant/internal tools in the initial profile. Final dependency/features/advisory review occurs
when the optional binary is actually implemented.

After code, run minimal Rust 1.98 compiler/Clippy for the leaf, current client/contracts and direct consumers,
plus focused tool schema, bounds, cancellation/EOF and source-backed result cases. Optional MCP work never
blocks the baseline product.
