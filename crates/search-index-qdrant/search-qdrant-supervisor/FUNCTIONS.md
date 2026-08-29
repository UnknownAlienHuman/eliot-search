# Function contract — `search-qdrant-supervisor`

**Status:** W3/P05 process-lifecycle contract; no artifact is qualified yet.

The exact secret-delivery mechanism is a P05 qualification decision. Public APIs receive only a
purpose/incarnation-bound `SecretLease`; plaintext may not escape into configuration, argv, logs,
telemetry or receipts.

## Artifact and configuration operations

### `qualify_artifact(candidate, manifest, platform) -> Result<QualifiedArtifact, SupervisorError>`

Verifies local file identity, SHA-256, exact version/build identity, executable architecture, source/
license receipt and accepted qualification manifest. It never downloads or upgrades an artifact.

### `validate_process_config(config, owner, artifact, secret_lease) -> Result<QualifiedProcessConfig, SupervisorError>`

Requires loopback-only bind, owned data directory, fixed one-node topology, valid short-lived secret
lease and bounded time/restart settings.

### `materialize_private_launch_config(config, secret_lease) -> Result<PrivateLaunchConfigGuard, SupervisorError>`

Uses only the P05-qualified delivery channel and ACL. The guard is non-serializable and removes/
invalidates temporary material on every exit path. No public type exposes secret bytes.

## Lifecycle operations

### `start(owner_guard, artifact, config) -> Result<QdrantProcessGuard, SupervisorError>`

Idempotency key is `(data_root_owner_epoch, installation_incarnation, artifact_digest)`. A matching live
guard may be returned; ambiguous PID/port/config state quarantines rather than attaches.

### `wait_readiness(guard, deadline, cancel) -> Result<ProcessReadiness, SupervisorError>`

Readiness proves process identity and authenticated loopback health only. Collection/schema capability
admission remains the bridge's responsibility.

### `verify_identity(guard) -> Result<ProcessIdentityReceipt, SupervisorError>`

Rechecks PID, creation identity, executable path/digest, owner/data-root record and Job Object
membership. PID reuse or executable replacement quarantines.

### `classify_exit(observation, policy) -> RestartDecision`

Returns stop, bounded restart or quarantine. Restart counters are windowed and cannot loop forever.

### `restart(guard, decision) -> Result<QdrantProcessGuard, SupervisorError>`

Drains/stops the old child before starting the exact same accepted artifact/config generation. A config
or artifact change requires daemon reconfiguration, not an implicit restart substitution.

### `shutdown(guard, mode, deadline) -> Result<ShutdownReceipt, SupervisorError>`

Stops admission to the child, requests graceful termination, enforces bounded kill through the Job
Object and verifies child cleanup. Receipt contains no secret or raw absolute path.

## Configuration operations

Implements `config/sections/qdrant_process.md`. Enabling requires exact path/version/SHA-256,
`SecretRef`, accepted qualification and the indexed Cargo feature. Automatic download/upgrade is
always rejected.

## Crash and unknown outcome

Startup/restart uncertainty is resolved by owner/process/executable/data-root identity checks, never by
a responding port. Orphans with ambiguous identity are quarantined. The supervisor never claims Windows
containment without executed evidence.

## Required qualification fixtures

Wrong artifact never starts; loopback/auth/ACL; secret side-channel audit; Job Object child cleanup;
PID reuse/replacement; bounded restart to quarantine; second root owner denied; crash between start and
owner-record publication; no process-lifecycle logic in the bridge.
