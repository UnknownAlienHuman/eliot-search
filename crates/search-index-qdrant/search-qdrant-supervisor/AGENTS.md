# Working on search-qdrant-supervisor

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source audit](FUNCTIONS.md) and
[bridge map](../search-qdrant-bridge/FUNCTIONS.md).

The baseline product connects to a configured local Qdrant endpoint through the one real bridge/runtime.
Do not make this package, executable hashes, containment receipts, secret leases or restart/quarantine state a
prerequisite for #263/#264.

Current public main has no product caller for the supervisor. `containment.rs` cannot apply Windows Job/ACL
operations and accepts caller-produced evidence. Do not harden that evidence protocol.

If an optional managed-child profile is selected, replace the package with one small concrete owner:

- spawn exactly the configured executable and retain the actual child handle;
- apply an actual platform child-tree guard or report managed mode unsupported;
- use one absolute startup/shutdown deadline;
- use the Qdrant bridge/client for authenticated readiness;
- keep one actual optional API key, not a lease/equality receipt;
- sanitize environment and own private bounded config/log resources;
- never adopt an existing process or auto-restart in the first profile;
- bounded kill/wait on failure and shutdown.

Delete artifact qualification, package-local SHA, owner fences, purpose digests, launch receipts, pure
supervisor/restart/quarantine state and the manual HTTP probe with their fixtures. If no managed profile is
shipped, delete this package entirely.

After implementation run minimal Rust 1.98 compiler/Clippy for the optional owner and actual daemon/bridge
consumers. Historical W3/P05 qualification matrices are release evidence, not source gates.
