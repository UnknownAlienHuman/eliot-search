# Agent contract — search-os-secrets

You own only `crates/search-runtime/search-os-secrets/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S27.2, S32.2, H8.4, H16.2, P01, P05, P14.

## Mission

Provide opaque, OS-bound local secrets without creating a plaintext configuration, process-command,
logging or serialization path.

## Ownership

- `SecretRef` lifecycle: create, resolve-for-use, rotate and delete
- binding to the current OS user, installation and installation incarnation
- purpose separation for Qdrant API keys, provider pairing and future local worker bindings
- zeroization/short-lived plaintext buffers behind the private adapter boundary
- receipts proving creation/rotation/deletion without revealing the secret

## Forbidden ownership

- Qdrant process supervision or network transport
- provider-session authorization or source-access grants
- returning plaintext through a public contract
- secrets in TOML/JSON config, command lines, environment dumps, logs, metrics or panic messages
- cross-user or cross-incarnation secret reuse
- claiming hardware-backed protection without executed evidence

## Allowed dependencies

`search-contracts`, `search-domain`. Platform crypto/credential dependencies require an ADR, exact
version, license review and Windows qualification. Public APIs expose opaque references and typed
receipts only.

## Required logical surface

- `SecretStore::create(scope, purpose) -> Result<SecretRef, SecretError>`
- `SecretStore::with_secret(reference, consumer) -> Result<UseReceipt, SecretError>`
- `SecretStore::rotate(reference) -> Result<RotationReceipt, SecretError>`
- `SecretStore::delete(reference) -> Result<DeletionReceipt, SecretError>`
- `SecretStore::validate_binding(reference, owner) -> Result<(), SecretError>`

`with_secret` is a behavior contract: implementations may use a guarded callback/lease so plaintext
cannot escape into caller-owned durable state.

## Failure surface

Use typed errors/reasons such as `SECRET_UNAVAILABLE`, `SECRET_BINDING_MISMATCH`,
`SECRET_STORE_LOCKED` and `SECRET_ROTATION_INCOMPLETE`. Fail closed.

## Test seams and exit evidence

- `secret plaintext absent from config argv environment logs metrics and receipts`
- `different OS user or installation incarnation cannot resolve reference`
- `rotation never exposes a mixed old/new binding`
- `crash during rotation leaves one recoverable authoritative secret`
- `delete is idempotent and does not claim physical secure erase`
- `debug formatting and serialization reveal only opaque reference metadata`

## Size and split guard

- Delivery wave: **W1 / P01 foundation; qualified with P05/P14**
- Soft `src/` target: **3,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Split only if Windows and another OS require genuinely different provider dependencies.

## Definition of done

The public surface is opaque and vendor-neutral, secret side channels are tested, binding failures are
typed, and the handoff contains raw Windows evidence rather than a security claim based on compilation.
