# PR #193: legacy revision envelope ownership

Date: 2026-09-18
Tracking: issue #189 / PR #193 / T02 Phase 4

## Ownership move

The legacy DIRECT protected-revision envelope had been defined entirely inside
`eliot-searchd::revision_protection`: outer/inner magic, version, finite limits,
binding records, byte encoding, byte decoding and plaintext digest validation.
That was secret-format business logic in the composition binary.

`search-os-secrets` now owns the pure compatibility contract:

- `ELSRV2\0\0` and `ELSIN2\0\0` version-1 layouts;
- 64 MiB plaintext and 65 MiB protected-object ceilings;
- namespace, platform-key, revision, content-digest and exact-length binding;
- outer and inner encode/decode;
- marker-only protected-format recognition;
- injected concrete content digest rather than a package-selected hash;
- closed content-free envelope errors.

The former monolithic package root is split into a thin public facade,
`lifecycle.rs`, and `legacy_revision_protection.rs`. Public lifecycle names are
re-exported unchanged.

## Daemon composition retained

The daemon still owns composition of the compatibility format with:

- legacy namespace/revision/content identifiers;
- the existing SHA-256 implementation;
- platform root-secret lookup/creation;
- platform-key and DPAPI-entropy derivation;
- Windows DPAPI invocation and native allocation cleanup;
- development-mode plaintext writes;
- compatibility translation to existing `DIRECT_REVISION_*` reason codes;
- explicit migration and source-policy checks outside this module.

The daemon no longer defines envelope magic, version, binding structs,
field ordering, encode/decode or plaintext-content comparison.

## Compatibility

Persisted `.dpapi` bytes are unchanged. The old outer header remains 156 bytes
and the inner header remains 148 bytes. No object, catalog, credential target,
key derivation domain, dependency, lockfile, workflow, gate or launch state is
changed.

Package tests freeze field order and round trips, reject every truncation,
validate every binding dimension, and reject malformed payload, length and
content. A daemon ownership regression prevents the codec from returning to
`eliot-searchd` or acquiring filesystem, DPAPI, Credential Manager, Qdrant,
source-catalog or policy ownership.

## Remaining Phase 4 work

The next slices move the platform root-secret and DPAPI effect boundaries out of
the daemon into the existing `search-os-secrets-windows` adapter and compose the
short-secret lifecycle in `search-os-secrets`. They must preserve the legacy
credential target, recovery semantics, current-user profile binding and exact
`DIRECT_REVISION_*` compatibility reasons. Large-revision replacement with the
accepted AES-256-GCM plus DPAPI-wrapped-DEK profile is separate behavior work and
must not be hidden in this ownership move.

## Required execution

```text
cargo test --locked -p search-os-secrets legacy_revision_protection
cargo test --locked -p eliot-searchd revision_protection
cargo test --locked -p eliot-searchd \
  --test revision_protection_contract_ownership
cargo test --locked -p eliot-searchd \
  --test revision_protection_windows_module_ownership
cargo check --locked -p search-os-secrets -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-os-secrets -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No Windows DPAPI qualification, T02 acceptance or
independent review is claimed.
