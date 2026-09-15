# PR #193: source-import output artifact ownership

Date: 2026-09-15. Tracking: PR #193 / issue #189 and T02 ownership restoration.

## Ownership correction

`control_migration_redb.rs` previously owned two related state machines inside
the daemon:

1. per-artifact lock-file acquisition and verification;
2. `.pending` creation/reopen, native identity pinning, hard-link publication,
   directory durability and crash-alias cleanup.

Both are storage/migration behavior. They now live in
`search-control-redb::migration`:

- `SourceImportOutputLock` owns the exclusive logical-name lock;
- `SourceImportOutputArtifact` owns pending/final locator lifecycle.

The daemon supplies `DaemonImportOutputPlatform`, which delegates only the
platform observations that cannot live in the portable control package:

- admitted output-directory validation;
- exact opened-file/locator identity verification;
- stable native file identity;
- directory-entry durability on supported platforms.

The package owner preserves the existing `DIRECT_MIGRATION_OUTPUT_*` and
`DIRECT_MIGRATION_IMPORT_*` reason namespaces. A final locator is never
overwritten. A hard-link result is not trusted by name: the daemon performs the
existing complete source replay and redb row readback through the exact file
returned by the package, then passes that verified native identity back before
alias cleanup. A distinct pending database remains untouched even when its rows
could compare equal.

The empty lock file remains after release. Removing it could allow two native
objects to represent and lock one logical output name.

## Unchanged boundaries

No output database format, redb schema, migration row, digest, pending/final
filename, source replay, content-manifest check, native identity algorithm,
dependency, workflow or authority record changed.

The daemon still owns adaptation from the legacy `DirectStore` and replay of the
validated source mapping. `search-control-redb` owns artifact mechanics and redb
import/readback semantics; it does not gain access to source bodies or daemon
filesystem internals.

## Regression coverage

Package tests cover:

- pending creation and directory durability;
- no-clobber hard-link publication;
- exact native-identity continuity;
- verified crash-alias cleanup;
- identity substitution failure before publication.

`xtask/tests/control_import_output_lock_ownership.rs` rejects restoration of
lock acquisition, pending open/create, hard-link publication or alias cleanup in
the daemon.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb \
  migration::output_lock::tests migration::output_artifact::tests
cargo +1.98.0 test --locked -p xtask --test control_import_output_lock_ownership
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd \
  --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked \
  -p search-control-redb -p eliot-searchd -p xtask \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No T02/T10/T11 completion, runtime PASS, native qualification or independent
review is claimed.
