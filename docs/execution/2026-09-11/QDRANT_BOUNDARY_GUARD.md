# Structural Qdrant boundary guard

Issue: #187

Normative references:

- `AGENTS.md` — vendor/native boundary and invariant 17
- `docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`
- `xtask/src/qdrant_boundary.rs`
- `docs/execution/2026-09-05/tasks/T22.md`

## Current baseline

`main` exposes:

```text
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

The gate is required to prevent later agents from eroding the adapter boundary.

## Complete the task

1. Scan every Cargo manifest and reject `qdrant-client` declarations outside
   the root workspace pin and `search-qdrant-bridge` inheritance.
2. Scan Rust implementation sources outside the bridge and reject vendor
   imports/references.
3. Reject public bridge signatures, type aliases, statics and re-exports that
   mention `qdrant_client`.
4. Cross-check the exact client version across root `Cargo.toml`, `Cargo.lock`,
   `qualified.rs` and `qualification/qdrant/artifact.toml`.
5. Cross-check the qualified server version between Rust qualification code and
   the artifact manifest.
6. Keep source scanning lexical, bounded and resistant to self-matching. Do not
   turn this validator into a build system or live-server test harness.

## Acceptance

- deliberate dependency, import and public-surface leaks fail with stable paths;
- inconsistent checked-in client/server identities fail closed;
- comments and the validator's own string literals do not create false passes or
  false failures;
- no network, process spawn, repository mutation or authority issuance occurs;
- package instructions and upgrade documentation name the structural command;
- live Qdrant probes remain a separate mandatory qualification layer.

Execution tests are deferred until the refactor series is complete; this PR is
an exact implementation packet, not a passing qualification receipt.