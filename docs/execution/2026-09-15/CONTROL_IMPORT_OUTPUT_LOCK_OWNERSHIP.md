# PR #193: source-import output-lock ownership move

Date: 2026-09-15. Tracking: PR #193 / issue #189 and T02 ownership restoration.

## Ownership correction

`control_migration_redb.rs` previously owned the per-artifact lock-file state
machine inside the daemon: lock-file creation/opening, zero-length validation,
`try_lock`, contention classification, lock durability and repeated locator
verification. That is storage/migration behavior, not daemon composition.

The state machine now lives in
`search-control-redb::migration::SourceImportOutputLock`. The daemon supplies
only `DaemonImportOutputPlatform`, which delegates the three platform-specific
observations already owned by integration code:

- admitted output-directory validation;
- exact opened-file/locator identity verification;
- directory-entry durability on supported platforms.

The package owner preserves existing reason strings by carrying platform errors
without rewriting them. Internal lock failures retain the existing
`DIRECT_MIGRATION_OUTPUT_*` namespace. The empty lock file remains after release;
removing it would permit two inodes to represent one logical output name.

No output database format, redb schema, migration row, digest, pending/final
locator, publication order, native identity implementation or dependency changed.
The remaining pending-file creation, exact redb readback, hard-link publication
and alias cleanup are still daemon-owned and are the next T02 storage slice.

## Regression coverage

Package unit tests cover exclusive ownership, release/reacquisition, retained
lock-file presence, invalid names/nonempty lock state and elapsed deadlines.
`xtask/tests/control_import_output_lock_ownership.rs` rejects restoration of
`ImportOutputGuard`, direct `try_lock` or `TryLockError` handling in the daemon
and requires composition through the package-owned platform port.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb migration::output_lock::tests
cargo +1.98.0 test --locked -p xtask --test control_import_output_lock_ownership
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-control-redb -p eliot-searchd -p xtask \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No T02/T10/T11 completion, runtime PASS, native qualification or independent
review is claimed.
