# Function contract — `search-os-secrets`

**Status:** W1/P01 logical contract; no Windows secret backend or side-channel evidence exists.

This package owns opaque OS-user/installation/incarnation/purpose-bound local secret lifecycle. Public
APIs never return plaintext secret bytes. Process launch, provider pairing, transport sessions and source
access remain outside this package.

## Global rules

- configuration and durable records contain only typed opaque `SecretRef` values;
- plaintext is available only inside a bounded guarded-use callback/lease adapter and is non-serializable;
- one reference binds OS user, installation, incarnation, purpose and authoritative generation;
- Debug, Display, serde, errors, receipts, logs, metrics and panic text expose no secret material;
- no secret enters command line, repository file, ordinary environment snapshot or crash metadata;
- deletion claims logical unavailability from this store, not physical secure erasure.

## Configuration operations

### `section_descriptor() -> ConfigSectionDescriptor`

### `compiled_defaults() -> ConfigSectionInput`

### `validate_section(input, platform, accepted_capabilities) -> Result<ValidatedSecretsConfig, SecretError>`

Implements `config/sections/secrets.md`. Accepts only the qualified OS-bound backend and opaque refs.
Plaintext, encoded key bytes, DPAPI blobs in config and command-line secrets are rejected.

### `section_digest(validated) -> Blake3Digest32`

### `plan_section_change(old, new) -> Result<SectionReloadDecision, SecretError>`

Preserves restart/gate obligations for backend/reference changes and restrictive live behavior for shorter
leases. It never resolves a secret while planning configuration.

## Reference and binding operations

### `validate_binding(binding, current_platform_identity) -> Result<ValidatedSecretBinding, SecretError>`

Requires exact OS user identity, installation/incarnation, non-empty closed purpose and supported backend.
Cross-user/incarnation/purpose substitution fails closed.

### `validate_reference(reference, expected_binding, expected_purpose) -> Result<ValidatedSecretRef, SecretError>`

Checks reference schema/backend/store identity/generation and binding digest without opening plaintext.
Unknown versions and default/foreign refs are rejected.

### `reference_metadata(reference) -> RedactedSecretMetadata`

Returns backend class, purpose, generation, binding digest and lifecycle state only.

## Secret lifecycle

### `create(binding, purpose, entropy_port, store_port, operation, deadline, cancel) -> Result<SecretCreateReceipt, SecretError>`

Generates accepted-strength random material through an injected OS/CSPRNG port and durably stores it under
the validated binding. Success returns a new opaque reference plus content-free receipt.

Cancellation before store mutation is clean. Timeout/cancellation after possible write is
`SECRET_CREATE_OUTCOME_UNKNOWN`; recovery queries exact operation/binding/purpose identity and never
creates an untracked second secret.

### `with_secret(reference, expected_binding, purpose, consumer, deadline, cancel) -> Result<SecretUseReceipt, SecretError>`

Opens plaintext only inside a non-serializable guarded consumer invocation. The consumer receives a
borrowed bounded secret view whose type cannot be returned, cloned into a public record or formatted.

The guard verifies binding/generation before and after use, zeroizes owned transient buffers where the
qualified backend/runtime permits and returns only operation/purpose/generation/timing-class metadata.

Cancellation before consumer invocation is clean. Cancellation during use signals the consumer but does
not claim immediate physical zeroization of provider/runtime copies; the receipt reports completion or
incomplete guarded use without secret data.

### `issue_lease(reference, expected_binding, purpose, ttl, consumer_identity) -> Result<SecretLease, SecretError>`

Creates a short-lived non-serializable lease capability for a daemon-owned adapter. TTL and purpose are
bounded by config/profile. A lease cannot be converted to plaintext by public API, serialized across a
process boundary or reused after incarnation/generation change.

### `rotate(reference, expected_binding, operation, deadline, cancel) -> Result<SecretRotationReceipt, SecretError>`

Implements a crash-recoverable state machine:

```text
CURRENT(G)
→ NEXT_STORED(G+1)
→ AUTHORITY_SWITCHED(G+1)
→ OLD_RETIRED(G)
```

At every recovery point exactly one generation is authoritative; bounded grace may permit explicit
verification of the retiring generation but cannot make both default. Same operation is idempotent and
conflicting input rejects.

### `delete(reference, expected_binding, operation, deadline) -> Result<SecretDeletionReceipt, SecretError>`

Revokes future resolution, invalidates outstanding leases for the reference/generation and requests
backend deletion. Success is idempotent and explicitly does not claim physical secure erase, backup
removal or platform forensic erasure.

### `recover_operation(operation, expected_binding, store_port) -> Result<SecretRecoveryDecision, SecretError>`

Resolves unknown create/rotate/delete outcomes by exact backend operation metadata and generation state.
Ambiguous or corrupt state locks/quarantines the reference rather than exposing/choosing material.

## Health and audit

### `health(reference_or_backend, expected_binding) -> SecretStoreHealth`

Returns availability/locked/quarantined state, backend class and redacted generation counts. It contains
no existence details for foreign bindings.

### `audit_public_surfaces(fixtures) -> SecretSideChannelAudit`

Package-local test operation scans Debug/Display/serde/error/receipt/log/metric/process-launch fixture
surfaces for canary bytes. It is evidence support, not a production secret scanner.

## Cancellation, deadline and crash semantics

All store operations use finite deadlines and stable mutation identities. A timeout after possible write
is unknown until exact recovery. Crashes never permit fallback to another user/incarnation/purpose or
return secret material in recovery diagnostics.

## Typed failures

- `SECRET_BACKEND_UNAVAILABLE`
- `SECRET_BACKEND_NOT_QUALIFIED`
- `SECRET_REFERENCE_INVALID`
- `SECRET_BINDING_MISMATCH`
- `SECRET_PURPOSE_MISMATCH`
- `SECRET_GENERATION_MISMATCH`
- `SECRET_STORE_LOCKED`
- `SECRET_QUARANTINED`
- `SECRET_LEASE_EXPIRED`
- `SECRET_LEASE_REVOKED`
- `SECRET_CREATE_OUTCOME_UNKNOWN`
- `SECRET_ROTATION_INCOMPLETE`
- `SECRET_DELETE_OUTCOME_UNKNOWN`
- `SECRET_OPERATION_CONFLICT`
- `SECRET_USE_CANCELLED`
- `SECRET_PLAINTEXT_FORBIDDEN`

## Required tests / qualification evidence

- public type/serde/Debug/error/receipt surfaces cannot expose plaintext;
- canary absent from argv, config, environment snapshot, logs, metrics and crash metadata;
- cross-user, cross-installation, cross-incarnation and cross-purpose resolution denied;
- reference schema/backend/store/generation mismatch fails closed;
- create crash/timeout before/after backend write and receipt publication;
- rotation crash at every state yields one authoritative generation;
- old lease invalid after rotation/deletion/incarnation change;
- lease TTL/purpose/consumer and non-serialization bounds;
- deletion idempotency and explicit secure-erasure nonclaim;
- backend lock/corruption produces locked/quarantined state without existence leakage;
- `secrets` configuration plaintext/ref/change-policy fixtures;
- fake entropy/store/clock/consumer ports prove no process/session ownership.
