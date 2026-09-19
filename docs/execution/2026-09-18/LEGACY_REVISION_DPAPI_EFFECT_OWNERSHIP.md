# PR #193: legacy revision DPAPI effect ownership

Date: 2026-09-18
Tracking: issue #189 / PR #193 / T02 Phase 4

## Ownership move

The legacy revision path previously invoked `CryptProtectData`,
`CryptUnprotectData`, `LocalFree`, and owned DPAPI `DATA_BLOB` outputs inside
`eliot-searchd::revision_protection_windows`. That made the composition binary a
second native secret-effect owner even after the pure envelope and key-derivation
contracts had moved to `search-os-secrets`.

`search-os-secrets-windows` now owns:

- the native DPAPI FFI declarations;
- UI-forbidden protect/unprotect calls;
- exact finite input and output limits;
- optional-entropy copies and overwrite-on-drop;
- one RAII owner for every DPAPI `LocalAlloc` output;
- bounded copy-out and clear-before-free;
- legacy revision protect/unprotect entrypoints accepting exact 32-byte entropy;
- typed content-free native failures.

The existing short-secret API is retained and now uses the same private native
owner rather than a second inline DPAPI implementation.

## Daemon composition retained

The daemon keeps only:

- Credential Manager read/create/readback and its cross-process vault lock;
- CSPRNG creation of the legacy namespace root secret;
- protected-object inventory needed to reject missing-key replacement;
- concrete SHA-256 derivation and frozen legacy envelope composition;
- source/revision/catalog policy;
- translation from typed adapter failures to the historical
  `DIRECT_DPAPI_*` / `DIRECT_REVISION_*` reasons.

`revision_protection_windows/ffi.rs` no longer contains DPAPI `DataBlob`,
`CryptProtectData`, `CryptUnprotectData`, `LocalFree`, or `LocalAllocation`.
`revision_protection_windows/dpapi.rs` is a bounded reason-translation wrapper.

## Compatibility

The following are unchanged:

- outer and inner legacy envelope bytes;
- DPAPI optional entropy bytes;
- current-user DPAPI scope and `CRYPTPROTECT_UI_FORBIDDEN`;
- 65 MiB compatibility ceiling;
- Credential Manager target and persistence mode;
- root-secret/key-binding/entropy derivation domains;
- `DIRECT_DPAPI_INPUT_TOO_LARGE`;
- `DIRECT_DPAPI_OUTPUT_TOO_LARGE`;
- `DIRECT_DPAPI_OUTPUT_INVALID`;
- `DIRECT_DPAPI_PROTECT_FAILED:<code>`;
- `DIRECT_DPAPI_UNPROTECT_FAILED:<code>`.

The daemon adds a Windows-only path dependency on the existing workspace
adapter; `Cargo.lock` gains only that package edge. The adapter remains outside
the registered 45-package authority map and acquires no source or policy role.

## Regression coverage

- package tests freeze typed reason codes and non-Windows fail-closed behavior;
- Windows tests cover short-secret scope binding and legacy revision entropy
  binding;
- daemon wrapper tests freeze exact DIRECT compatibility reasons;
- ownership tests require all DPAPI/LocalAlloc markers in the platform package
  and reject their return to daemon FFI;
- remaining credential, RNG, mutex, inventory, and migration responsibilities
  stay mechanically bounded in daemon modules.

## Required execution

```text
cargo test --locked -p search-os-secrets-windows
cargo test --locked -p eliot-searchd revision_protection_windows::dpapi::tests
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
`rustc`, and `rustfmt` are absent). No Windows DPAPI qualification, T02
acceptance, or independent review is claimed.
