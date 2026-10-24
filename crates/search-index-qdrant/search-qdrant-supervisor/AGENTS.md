# Working on search-qdrant-supervisor

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source audit](FUNCTIONS.md),
[bridge map](../search-qdrant-bridge/FUNCTIONS.md) and optional issue
[#334](https://github.com/UnknownAlienHuman/eliot-search/issues/334).

The baseline product connects to a configured local Qdrant endpoint through one real bridge/runtime. Do not
make this package, executable hashes, containment receipts, secret leases or restart/quarantine state a
prerequisite for #263/#264.

The established caller audit found supervisor construction in package-local tests rather than normal daemon
publication/query. It also found caller-produced containment evidence without Windows Job/ACL effects. Those are
revision-specific findings. Before deleting or replacing the package, recheck current manifests, Rust/non-Rust
callers, features, targets, tests and workflows. Do not harden the evidence protocol merely because the package
remains in the workspace.

If an optional managed-child profile is selected, replace the package with one small concrete owner:

- spawn exactly the configured executable and retain the actual child handle;
- apply a real platform child-tree guard or report managed mode unsupported;
- use one absolute startup/shutdown deadline;
- use the Qdrant bridge/client for authenticated readiness;
- keep one actual optional API key, not a lease/equality receipt;
- sanitize environment and own private bounded config/log resources;
- never adopt an existing process or auto-restart in the first profile;
- perform bounded kill/wait on failure and shutdown.

Delete artifact qualification, package-local SHA, owner fences, purpose digests, launch receipts, pure
supervisor/restart/quarantine state and the manual HTTP probe with their fixtures. If no managed profile is
selected after the current caller check, delete this package entirely.

After implementation, run minimal Rust 1.98 compiler/Clippy for the optional owner and actual daemon/bridge
consumers. Native Job/tree, timeout, early-exit, auth, environment/log privacy and orphan-child behavior require
actual platform execution; historical qualification matrices are not runtime gates.
