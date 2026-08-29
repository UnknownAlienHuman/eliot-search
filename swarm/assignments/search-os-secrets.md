# `search-os-secrets` implementation packet

**Path:** `crates/search-runtime/search-os-secrets`  
**Capability:** C01/C15/C30 security support  
**Delivery:** W1 / P01 foundation; qualified with P05/P14  
**Gate:** BLOCKED until W0 contracts/domain handoffs are accepted  
**Trace:** S27.2, S32.0-S32.2, H8.4, H16.2, P01, P05, P14  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Provide opaque OS-bound local secrets without creating a plaintext configuration, process-command,
logging, crash-report or serialization path.

## Owns

- secret creation, guarded use, rotation and deletion
- binding to OS user, installation, incarnation and declared purpose
- purpose separation for Qdrant API keys, provider pairing and local worker credentials
- short-lived plaintext lease semantics and non-secret receipts

## Must not own

- process supervision, transport sessions or source-access grants
- returning plaintext from a public method
- credentials in config files, argv, environment dumps, logs, metrics or panic messages
- cross-user/incarnation reuse
- physical secure-erasure or hardware-backed claims without executed evidence

## Logical primitives

- `SecretPurpose`, `SecretRef`, `SecretBinding`, `SecretLease`, `SecretGeneration`, `SecretStoreReceipt`, `SecretStoreHealth`

## Logical operations

1. `create(binding, purpose) -> Result<SecretRef, SecretError>`
2. `with_secret(reference, consumer) -> Result<SecretUseReceipt, SecretError>`
3. `rotate(reference) -> Result<SecretRotationReceipt, SecretError>`
4. `delete(reference) -> Result<SecretDeletionReceipt, SecretError>`
5. `validate_binding(reference, expected) -> Result<(), SecretError>`

`with_secret` is a guarded-use contract: plaintext cannot be returned or stored by the public API.

## Required invariants

- public serialization and Debug expose only opaque reference metadata
- a different OS user, installation incarnation or purpose cannot resolve a reference
- rotation has one authoritative generation before and after recovery
- secret material never appears in argv, config, logs, metrics, telemetry or receipts
- deletion is idempotent and does not overclaim physical erasure

## Typed failure surface

- `SECRET_UNAVAILABLE`
- `SECRET_BINDING_MISMATCH`
- `SECRET_STORE_LOCKED`
- `SECRET_ROTATION_INCOMPLETE`
- `SECRET_PURPOSE_MISMATCH`

## Exit tests / evidence

- `plaintext_absent_from_public_side_channels`
- `cross_user_and_cross_incarnation_resolution_denied`
- `purpose_separation_enforced`
- `rotation_crash_recovers_one_authoritative_generation`
- `debug_and_serialization_are_opaque`
- `deletion_is_idempotent_without_secure_erase_claim`

## Suggested internal modules

```text
search-os-secrets/src/
  reference.rs
  binding.rs
  store.rs
  lease.rs
  rotation.rs
  receipt.rs
  platform/
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 3,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Split platform providers only when they require genuinely incompatible dependencies or safety boundaries.
