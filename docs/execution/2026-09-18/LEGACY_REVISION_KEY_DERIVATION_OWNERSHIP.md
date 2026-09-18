# PR #193: legacy revision key-derivation ownership

Date: 2026-09-18
Tracking: issue #189 / PR #193 / T02 Phase 4

## Ownership move

The previous Phase 4 slice moved the frozen legacy protected-revision envelope
into `search-os-secrets`, but two load-bearing derivation transcripts remained
literal daemon implementation details:

- `eliot-search/revision-key-binding/v1`;
- `eliot-search/revision-dpapi-entropy/v1`.

`search-os-secrets` now owns those exact domains, the 32-byte root-secret shape,
and the ordered transcript `(namespace_id, root_secret)`. The package exposes
only pure derivation functions over an injected multipart digest owner.

The package still performs no SHA-256, Credential Manager, RNG, DPAPI,
filesystem, source-catalog or policy operation. It never persists or returns the
root secret.

## Daemon composition retained

`eliot-searchd` keeps:

- concrete SHA-256 through `DirectRevisionDigest`;
- root-secret lookup, creation and readback recovery;
- Credential Manager target and cross-process vault locking;
- protected-object inventory checks before first key creation;
- zeroizing root-secret and entropy owners;
- DPAPI invocation and DIRECT compatibility reason translation.

Both normal protector opening and read-only migration opening now call the same
package-owned derivation functions. The domain strings no longer exist in daemon
source.

## Compatibility

The digest algorithm, multipart framing, domain bytes, part order, credential
target, persisted `.dpapi` bytes and all stable `DIRECT_REVISION_*` reasons are
unchanged. No dependency, lockfile, workflow, persisted object, gate or launch
state changes.

Package tests freeze both domains, the exact root-secret length, part count and
namespace/root-secret ordering. Daemon ownership tests reject reintroducing the
derivation domains or direct derivation calls outside the package.

## Remaining Phase 4 work

The next bounded slice moves `CryptProtectData` / `CryptUnprotectData` and native
DPAPI allocation ownership out of the daemon into the existing
`search-os-secrets-windows` effect adapter. Credential Manager root-secret
lifecycle remains a separate following slice because it also owns crash recovery,
protected-object inventory checks and cross-process serialization.

## Required execution

```text
cargo test --locked -p search-os-secrets legacy_revision_protection
cargo test --locked -p eliot-searchd revision_protection
cargo test --locked -p eliot-searchd \
  --test revision_key_derivation_ownership
cargo test --locked -p eliot-searchd \
  --test revision_protection_contract_ownership
cargo check --locked -p search-os-secrets -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-os-secrets -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No Windows DPAPI qualification, T02 acceptance or
independent review is claimed.
