# PR #193: legacy revision test credential cleanup ownership

Date: 2026-09-19
Tracking: issue #189 / PR #193 / T02 Phase 4

## Ownership correction

After production Credential Manager lifecycle moved to
`search-os-secrets-windows`, daemon tests still retained a second native owner
for the same credential target:

- `CredDeleteW`;
- `CreateMutexW` / `WaitForSingleObject` / `ReleaseMutex` / `CloseHandle`;
- the exact revision-vault mutex name;
- target construction for `ELIOT Search/revision-key/<namespace>`;
- delete/readback retry behavior.

That duplicate existed only in `eliot-searchd` test configuration, but it still
made two packages responsible for one native credential lifecycle.

The optional package feature `test-credential-cleanup` now owns one bounded API:

```rust
pub fn delete_legacy_revision_root_secret_for_test(
    namespace_id: &[u8; 32],
) -> Result<(), LegacyRevisionRootSecretCleanupError>
```

The package derives the exact target, acquires the same cross-process mutex,
issues `CredDeleteW`, and returns success only after the normal package-owned
read path proves absence. Delete/read failures remain outcome-unknown after 16
bounded attempts. No credential enumeration, wildcard deletion, data-root read,
or source/catalog decision is possible through this API.

Production daemon builds do not enable the feature. Windows daemon test builds
enable it through a target-specific dev-dependency. `Cargo.lock` is unchanged.

## Daemon boundary

The daemon test helper retains only:

- read-only parsing of `control/namespace.id` from its disposable data root;
- exact 32-byte namespace decoding and normalized hex for diagnostics;
- invocation of the package cleanup API;
- a redacted best-effort diagnostic if cleanup remains unresolved.

The obsolete daemon file
`src/revision_protection_windows/ffi.rs` is deleted. Daemon test code no longer
contains `CredDeleteW`, vault-mutex FFI, or unsafe Windows credential code.

## Why protected-object traversal did not move

`search-revision-store/AGENTS.md` explicitly keeps concrete data-root traversal
and metadata acquisition outside the package. The existing daemon inventory
scan therefore remains composition I/O. `search-revision-store` continues to
own normalized inventory grammar/model/digest/page/report semantics, not the
filesystem walk itself.

## Preserved behavior

- exact Credential Manager target and credential type;
- the production vault mutex name and wait bound;
- test cleanup limit of 16 attempts;
- exponential backoff base of 10 ms and exponent cap of 7;
- absence verified through the package's exact credential read owner;
- best-effort drop cleanup in daemon tests;
- no product credential deletion route.

## Regression coverage

Ownership tests require:

- `CredDeleteW` and the cleanup function to exist only in
  `search-os-secrets-windows`;
- the feature to be absent from production dependencies and present only in the
  Windows dev-dependency;
- daemon test cleanup to contain no native credential or mutex APIs;
- the old daemon FFI file to remain deleted;
- historical production `DIRECT_REVISION_*` and `DIRECT_DPAPI_*` reasons to stay
  at the daemon boundary.

Package tests freeze the cleanup reason code and non-Windows fail-closed result.
The existing Windows revision-protection tests exercise cleanup through every
`TestCredentialGuard` lifecycle.

## Required execution

```text
cargo test --locked -p search-os-secrets-windows --all-features
cargo test --locked -p eliot-searchd \
  --test revision_protection_windows_module_ownership
cargo test --locked -p eliot-searchd revision_protection
cargo check --locked -p search-os-secrets-windows -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-os-secrets-windows -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No Windows qualification, T02 acceptance, or
independent review is claimed.
