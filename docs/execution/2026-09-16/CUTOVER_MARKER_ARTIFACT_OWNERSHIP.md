# Control-cutover marker artifact ownership

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 control-migration ownership

## Result

The canonical `control/control-cutover.v1` filesystem lifecycle now belongs to
`search-control-redb::migration` next to the existing marker schema and replay
classifier.

The package owns:

- bounded, locator-verified marker readback;
- strict canonical decode after exact file read;
- missing versus valid versus corrupt marker classification;
- canonical proposal validation before mutation;
- fixed `control-cutover.tmp` creation with `create_new`;
- complete write and file sync;
- exact temporary-file reread immediately before publication;
- directory-sync checkpoints;
- atomic no-clobber hard-link publication;
- exact post-publication byte readback;
- byte-identical idempotent replay;
- refusal to overwrite a different valid marker;
- corrupt, outcome-unknown and readback-mismatch classifications.

The daemon `marker_io` module now supplies only the existing qualified
filesystem platform adapter, stable `DIRECT_MIGRATION_CUTOVER_*` reason mapping
and the policy decision to arm catalog quarantine. `DirectStore` orchestration,
root-owner checks, source-history replay, staging and serve-path integration
remain in the daemon.

## Compatibility and hardening

The canonical marker codec, final filename, temporary filename, maximum size,
operator receipts, cutover replay semantics and staged database schema are
unchanged. After a successful hard link the temporary alias is removed, so the
committed on-disk marker remains the same single regular file. No dependency,
lockfile or record format changed.

Publication no longer relies on `rename` destination semantics. On Unix,
`rename` could replace a marker that appeared after the preflight check. The
same-directory hard-link operation fails when the final name already exists;
that state is then read and classified as identical replay, different valid
authority, corruption or unknown outcome. The exact temporary bytes and locator
are revalidated before the hard-link attempt.

The marker package deliberately does not create a data-root owner, perform a
cutover, authorize serving, arm quarantine or repair corrupt state. A canonical
marker is technical authority only after the caller has already established the
external root owner and all source/import invariants.

Read-only resolution also validates that the `control/` directory itself is a
real admitted directory. A redirected or unstable parent fails closed as
corrupt rather than allowing a regular-looking child through a redirected
parent.

## Regression seams

Package tests cover:

- absent → committed → byte-identical replay;
- exact marker/byte readback;
- a different valid marker never being overwritten;
- corrupt existing marker classification;
- invalid proposal rejection before mutation;
- missing control directory remaining absent for reads and invalid for writes;
- temporary marker cleanup after a successful publication.

`control_migration_owner_boundary` requires marker reads, `OpenOptions`,
`write_all`, `hard_link` and bounded `read_to_end` to remain in
`cutover_artifact.rs`, and explicitly rejects `rename`. It rejects restoration
of those operations in the daemon marker adapter. Quarantine references are
forbidden in the package owner and required in the daemon adapter.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb migration::cutover_artifact
cargo +1.98.0 test --locked -p search-control-redb migration::cutover
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_owner_boundary
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-control-redb -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, process-fixture,
T02 or independent-review PASS is claimed.
