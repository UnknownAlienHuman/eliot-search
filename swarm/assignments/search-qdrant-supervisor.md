# `search-qdrant-supervisor` implementation packet

**Path:** `crates/search-index-qdrant/search-qdrant-supervisor`  
**Capability:** C01/C15 process support  
**Delivery:** W3 / P05  
**Gate:** BLOCKED until exact Qdrant artifact ADR and `search-os-secrets` handoff are accepted  
**Trace:** S9.1-S9.3, S27, H8, P05  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-os-secrets`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Start and supervise exactly one qualified local Qdrant process under the daemon owner without exposing Qdrant data-plane operations.

## Owns

- exact executable path, version, digest and process identity qualification
- loopback-only configuration, filesystem/process ACL and Windows Job Object containment
- opaque API-secret reference consumption
- readiness, bounded restart, quarantine, drain and shutdown receipts
- PID/executable/hash mismatch detection

## Must not own

- collection creation, payload schema, point mutation/query/readback
- recipe meaning, access authority, scoring or publication commit
- automatic download/upgrade or floating `latest`
- plaintext secrets in files, environment dumps, argv or logs
- startup outside the data-root owner fence

## Logical primitives

- `QualifiedQdrantArtifact`, `QdrantProcessConfig`, `QdrantEndpoint`, `ProcessIdentity`, `QdrantProcessGuard`, `RestartPolicy`, `QuarantineState`, `SupervisorReceipt`

## Logical operations

1. `qualify_artifact(candidate) -> Result<QualifiedQdrantArtifact, SupervisorError>`
2. `start(owner, artifact, secret_ref) -> Result<QdrantProcessGuard, SupervisorError>`
3. `verify_identity(guard) -> Result<ProcessIdentityReceipt, SupervisorError>`
4. `readiness(guard) -> ProcessReadiness`
5. `recover(observed, policy) -> RecoveryDecision`
6. `shutdown(guard, mode) -> Result<ShutdownReceipt, SupervisorError>`

## Required invariants

- wrong artifact digest/version/path never starts
- process binds loopback only and requires authenticated access
- PID reuse or executable replacement quarantines the process
- child terminates with daemon/Job Object boundary
- restart attempts are bounded and end in explicit quarantine
- secret plaintext is absent from argv, config, logs and receipts

## Typed failure surface

- `QDRANT_ARTIFACT_MISMATCH`
- `QDRANT_PROCESS_IDENTITY_MISMATCH`
- `QDRANT_START_FAILED`
- `QDRANT_QUARANTINED`
- `QDRANT_CONTAINMENT_FAILED`
- `SECRET_UNAVAILABLE`

## Exit tests / evidence

- `exact_artifact_digest_and_version_required`
- `pid_reuse_and_executable_replacement_quarantine`
- `loopback_auth_and_acl_fixture`
- `job_object_child_cleanup`
- `bounded_restart_to_quarantine`
- `secret_absent_from_process_side_channels`
- `second_data_root_owner_denied`

## Suggested internal modules

```text
search-qdrant-supervisor/src/
  artifact.rs
  config.rs
  process.rs
  identity.rs
  containment.rs
  restart.rs
  readiness.rs
  shutdown.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 5,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Qdrant data-plane calls belong to `search-qdrant-bridge`; do not recreate them here.
