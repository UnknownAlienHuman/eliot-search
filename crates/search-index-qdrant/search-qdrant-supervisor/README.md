# search-qdrant-supervisor — optional process wrapper, not runtime authority

This package is **not connected to the public-main application path**. Repository search finds construction
and launch calls in its own lifecycle/live tests; normal daemon publication/query does not use it.

Public main contains about **90.7 KiB of production Rust** here before tests:

```text
artifact_policy.rs   5.1 KiB
containment.rs       7.8 KiB
identity.rs          9.7 KiB
launch.rs           20.2 KiB
lib.rs              20.9 KiB
owned.rs            14.2 KiB
secret.rs            4.3 KiB
sha256.rs            8.7 KiB
```

Most of that code models artifact qualification, owner fences, secret leases, process identity receipts,
restart/quarantine and caller-supplied containment evidence. Those records do not make Qdrant available to
the product.

Read the [implementation guide](../../../docs/implementation/README.md),
[Qdrant bridge map](../search-qdrant-bridge/FUNCTIONS.md), local [source map](FUNCTIONS.md) and
[optional process issue #334](https://github.com/UnknownAlienHuman/eliot-search/issues/334).

## Product order

The first working product uses a configured local Qdrant endpoint through the one real bridge/runtime:

```text
configured endpoint + optional actual API key
→ connect
→ verify live server and selected collection generation
→ publication/query
```

No process supervisor is required for this path. An externally started local service, development process or
installed service is acceptable for #263/#264.

A managed child is a later convenience profile. It must not block retained-source → Qdrant → retained-excerpt
integration and must not introduce another qualification, secret or recovery authority.

## If managed-child mode is selected

Keep only a small concrete owner:

```text
ManagedQdrant {
  Child / platform child-tree guard
  endpoint
  data directory
  optional actual API key
  original startup/shutdown deadlines
}
```

Behavior:

1. Spawn exactly the configured executable/config; never adopt a PID, process name or occupied endpoint.
2. Hold the actual child handle for the lifetime of the managed dependency.
3. On Windows, either apply a real Job Object/kill-on-close platform adapter or report managed mode
   unsupported. A caller-provided Boolean/digest is not containment.
4. Use the same Qdrant bridge/client for authenticated readiness and server capability checks; do not keep a
   second hand-written HTTP parser.
5. Use one absolute startup deadline. Failure owns bounded kill/wait of the child actually spawned.
6. On shutdown, stop new application work, close bridge users, terminate/reap the owned child and then release
   its data/config resources.
7. Do not auto-download, auto-upgrade, adopt foreign listeners or run an automatic restart/quarantine state
   machine in the first profile. A child exit makes the dependency unavailable; daemon/user restart is enough.
8. If an API key is enabled, one actual in-memory value is supplied to both child and bridge. There is no
   lease/equality receipt protocol. Sanitize the child environment and do not emit the key to argv/config/logs.
9. Generated config/log paths are private, bounded and cleanup-owned. Do not overwrite a shared fixed
   `config.yaml` or create unbounded permanent stdout/stderr logs in the index data directory.

## Removal boundary

After the direct endpoint path is connected, delete the current package unless managed-child mode is
explicitly selected. If selected, replace it rather than wrapping it.

Remove:

- `QualifiedArtifact`, artifact manifest/digest admission and package-local SHA-256;
- `QdrantOwnerFence`, `SecretLeaseEvidence`, purpose/equality checks and launch receipts;
- caller-supplied `ContainmentEvidence` and claimed Job/ACL reports;
- pure `QdrantSupervisor` transition/restart/quarantine state machine;
- PID/creation/executable/path/data-root receipt comparisons after an owned handle exists;
- process-local operation identities and qualification fixtures;
- duplicate manual HTTP readiness client;
- source-shape and exact-byte tests which only preserve the retired API.

Reuse only concrete child ownership, bounded wait/kill and minimal loopback/config helpers if they remain
smaller than direct daemon composition.

## Completion

The baseline is complete when the daemon connects to a configured local Qdrant endpoint through #263 and the
real product path works. Optional managed mode is complete only when a real spawned child is bounded and
reaped by its actual handle/platform containment, authenticated readiness is checked through the bridge and
no artifact/lease/receipt/qualification state machine remains.
