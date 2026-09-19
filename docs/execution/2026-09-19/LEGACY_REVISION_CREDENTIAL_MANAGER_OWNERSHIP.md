# PR #193: legacy revision Credential Manager ownership

Date: 2026-09-19
Tracking: issue #189 / PR #193 / T02 Phase 4

## Ownership move

The legacy revision path previously owned `CredReadW`, `CredWriteW`,
`CredFree`, `BCryptGenRandom`, the named revision-vault mutex, and the complete
root-secret create/readback protocol inside `eliot-searchd`. After DPAPI moved,
that still left the daemon as the native Windows secret owner.

`search-os-secrets-windows` now owns:

- the exact `ELIOT Search/revision-key/<lower-hex-namespace>` target;
- `CRED_TYPE_GENERIC` and local-machine persistence validation;
- Credential Manager read/write/free and credential-allocation cleanup;
- the fixed 32-byte secret owner and redacted/zeroizing lifetime;
- `BCryptGenRandom` root-secret generation;
- the `ELIOT-Search-RevisionVault-v1` cross-process mutex;
- bounded retry and exact write-readback verification;
- typed content-free platform failures;
- read-only existing-key lookup for migration.

The adapter exposes the same frozen 32-byte platform shape, while daemon
ownership tests compare it with the pure `search-os-secrets` contract. The
adapter does not own the derivation domains or hash.

## Concurrency correction

The old daemon implementation generated a candidate before acquiring the vault
mutex and wrote it immediately after lock acquisition. Two first-open processes
could both observe absence, generate different keys, then serialize two writes;
the second process could overwrite the first credential.

The package-owned protocol re-reads Credential Manager while holding the mutex.
If another process published a key after the initial read, that existing key is
returned and the stale generated candidate is zeroized without a write. This is
covered by a deterministic fake-platform regression.

## Daemon composition retained

The daemon still owns:

- bounded protected-object inventory under the admitted revision root;
- the decision between `CreateIfMissing` and `RequireExisting`;
- concrete SHA-256 key-binding/entropy derivation;
- frozen legacy envelope composition and revision/source policy;
- historical `DIRECT_REVISION_*` reason translation;
- test-only credential deletion until that support path receives a separate
  bounded owner.

The daemon wrapper first performs a read-only package lookup. Existing keys do
not require a filesystem scan. Only confirmed absence triggers protected-object
inventory and the explicit missing-key requirement.

## Compatibility

Unchanged:

- Credential Manager target text;
- generic credential type and local-machine persistence;
- 32-byte root secret;
- CSPRNG source and named mutex;
- retry counts/backoff ceilings;
- legacy `.dpapi` bytes and derivation transcripts;
- all historical `DIRECT_REVISION_KEY_*` and `DIRECT_REVISION_RNG_FAILED`
  reasons.

No dependency, `Cargo.lock`, persisted object, control schema, workflow, gate,
or launch-state change is required.

## Regression coverage

Package tests cover:

- exact lower-case target construction;
- existing-key fast path;
- post-lock concurrent winner adoption without overwrite;
- successful create plus exact readback;
- readback mismatch;
- protected-object missing-key refusal;
- redacted secret debug and stable typed reasons;
- non-Windows fail-closed behavior.

Daemon ownership tests require production Credential Manager, RNG, and mutex
markers in the Windows adapter and reject their return to daemon production
modules. The daemon wrapper tests freeze exact historical reason translation.

## Required execution

```text
cargo test --locked -p search-os-secrets-windows credential
cargo test --locked -p search-os-secrets-windows
cargo test --locked -p eliot-searchd \
  revision_protection_windows::credential::tests
cargo test --locked -p eliot-searchd \
  --test revision_protection_windows_module_ownership
cargo test --locked -p eliot-searchd \
  --test revision_key_derivation_ownership
cargo test --locked -p eliot-searchd revision_protection
cargo check --locked -p search-os-secrets-windows -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-os-secrets-windows -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No Windows Credential Manager qualification,
T02 acceptance, or independent review is claimed.
