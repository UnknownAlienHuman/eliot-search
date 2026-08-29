# Agent contract — search-qdrant-supervisor

You own only `crates/search-index-qdrant/search-qdrant-supervisor/`. Do not edit another package, the
root workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S27, H8.1-H8.5, P05.

## Mission

Start and supervise exactly one qualified local Qdrant process under the daemon owner without exposing
Qdrant data-plane operations.

## Ownership

- exact executable path, version, digest and process identity qualification
- loopback-only bind configuration and process/filesystem ACL preparation
- Windows Job Object assignment and child-process cleanup
- opaque API-secret reference consumption through `search-os-secrets`
- startup readiness, bounded restart, quarantine and shutdown receipts
- PID/executable/hash mismatch detection

## Forbidden ownership

- collection creation, payload schemas, point mutations, queries or readback
- recipe meaning, access authority, scoring or publication commit
- automatic download/upgrade or unpinned `latest`
- plaintext secrets in files, environment dumps, argv or logs
- running outside the data-root owner fence
- claiming Windows containment without executed tests

## Allowed dependencies

`search-contracts`, `search-domain`, `search-os-secrets`. Platform process/ACL dependencies require
exact qualification. It may expose only an opaque process guard, endpoint descriptor and readiness
receipt.

## Required logical surface

- `QdrantSupervisor::qualify(artifact) -> Result<QualifiedArtifact, SupervisorError>`
- `QdrantSupervisor::start(owner, artifact, secret_ref) -> Result<QdrantProcessGuard, SupervisorError>`
- `QdrantProcessGuard::readiness() -> ProcessReadiness`
- `QdrantProcessGuard::verify_identity() -> Result<ProcessIdentityReceipt, SupervisorError>`
- `QdrantProcessGuard::shutdown(mode) -> Result<ShutdownReceipt, SupervisorError>`
- `QdrantSupervisor::recover(observed) -> RecoveryDecision`

## Failure surface

Relevant reasons include `QDRANT_ARTIFACT_MISMATCH`, `QDRANT_PROCESS_IDENTITY_MISMATCH`,
`QDRANT_START_FAILED`, `QDRANT_QUARANTINED` and `SECRET_UNAVAILABLE`.

## Test seams and exit evidence

- `wrong executable hash or version never starts`
- `PID reuse or executable replacement quarantines`
- `process binds loopback only and requires API authentication`
- `child exits with daemon/Job Object`
- `bounded restart stops at quarantine threshold`
- `secret plaintext absent from argv logs config and crash reports`
- `second data-root owner cannot supervise the same process root`

## Size and split guard

- Delivery wave: **W3 / P05**
- Soft `src/` target: **5,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Data-plane calls belong to `search-qdrant-bridge`; do not recreate them here.

## Definition of done

The process owner is exact, bounded and testable on Windows; all data-plane semantics remain outside
this crate and the handoff contains raw containment evidence.
