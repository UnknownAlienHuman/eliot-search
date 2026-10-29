# Working on search-eliot-adapter

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), local [source audit](FUNCTIONS.md), deletion issue
[#535](https://github.com/UnknownAlienHuman/eliot-search/issues/535) and optional MCP issue
[#219](https://github.com/UnknownAlienHuman/eliot-search/issues/219).

## Currentness boundary

The R039 inspection found no production construction/import of `EliotAdapter` outside this package and found a
disconnected binding/grant/replay/terminal lifecycle. That is evidence for the inspected revision, not a
permanent claim about every later branch head.

Immediately before deletion, repeat an exact current-branch search across:

- workspace and package manifests;
- Rust imports, constructors, path/module inclusion and feature wiring;
- daemon/CLI targets, tests, workflows and non-Rust tooling.

If no real consumer exists, delete the package, workspace/dependency/lock wiring and exclusive fixtures. If a
consumer is found, name it and retain only the smallest truthful unavailable boundary required while that caller
moves. Do not connect or harden `EliotAdapter`, payload digests, terminal receipts, drain/quarantine or the
exact-eleven-recipe model.

## Optional MCP leaf

MCP follows a working standalone Search application. `rmcp` is absent from the ELIOT lock. The latest stable
SDK inspected in R039 was `rmcp 3.7.0` at commit
`5d6a11f2ae022f513aec0f6e0f10f89a57891a7f` with Rust 1.88 MSRV. This is candidate evidence, not an installed or
reserved pin. Recheck the exact current SDK, advisories, features and transitive closure with the real #219
consumer.

The first leaf is stdio-only and uses the same current `search-provider-client::LocalClient` as the CLI. Expose
only implemented `Search`, `Exact`, `Continue` and `Expand` handlers. The leaf owns MCP process lifetime, finite
tool schemas and request/result mapping only. It never owns source scope, grants, capabilities, Qdrant/redb/
object state, planning, ranking, windows or another session ledger.

Do not enable HTTP/OAuth/auth/resources/prompts/sampling/elicitation/Tasks/child-process/client transports or
raw filesystem/Qdrant/internal tools in the first profile merely because the SDK supports them.

After future code, run minimal Rust 1.98 compiler/Clippy for the leaf, current client/contracts and direct
consumers, plus focused schema, bounds, cancellation/EOF and source-backed result cases. Optional MCP work never
blocks the baseline product.
