# Legacy preparation-artifact I/O ownership move

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The bounded filesystem lifecycle for legacy DIRECT preparation objects and
reference records belongs to `search-materializer`.

The package-owned compatibility adapter treats artifact bytes as opaque and
owns:

- admitted parent/child directory handling;
- local final and temporary basename validation;
- bounded exact reads through one opened file;
- pre/post length, modification-time, locator and native-identity checks;
- no-clobber `create_new` temporary writes and file sync;
- complete temporary readback before publication;
- same-directory hard-link publication without replacement;
- exact final readback and native identity comparison;
- byte-identical idempotent replay;
- immutable conflict refusal;
- identity-bound temporary cleanup on success and early failure;
- publication/durability outcome-unknown classification.

The package does not open a source store, acquire credentials, perform DPAPI,
validate source plaintext, mutate a registry or publish a revision CAS object.
The related persisted binding/reference/manifest schema is now also package
owned and is documented separately in
`LEGACY_PREPARATION_STORE_SCHEMA_OWNERSHIP.md`.

## Daemon composition

`eliot-searchd::secure_direct_store_storage_io` provides one qualified native
platform adapter to two distinct package owners:

- `search-revision-store` for retained revision objects;
- `search-materializer` for preparation objects and references.

The daemon retains:

- native Windows/Unix identity observation and reparse qualification;
- temporary-name entropy/time;
- concrete SHA-256/BLAKE3 implementations;
- current profile construction and exact source-derived body recomputation;
- DPAPI composition and plaintext verification;
- stable historical `DIRECT_*` reason translation.

The private compatibility names `read_regular_file` and
`persist_immutable_object` delegate the entire read/publication lifecycle to
`search-materializer`. The helper source contains no `OpenOptions`,
`write_all`, hard-link publication or direct temporary cleanup.

Binding/reference/manifest field order, offsets, fixed sizes, digest preimages
and canonical locator names no longer belong to daemon modules.

## Compatibility and hardening

Preparation object/reference locations, extensions, byte layouts, profile
binding, encryption behavior, size ceilings and daemon reason strings are
unchanged. No dependency, lockfile or persisted format changed because
`search-materializer` was already a mandatory baseline DIRECT dependency.

The package owner fences locator/native identity before and after every read,
verifies the temporary artifact before publication, refuses replacement under
all races and retains outcome-unknown classification after a possible
externally visible effect.

No source or preparation bytes are added to logs, receipts or debug output.

## Regression seams

Package tests cover:

- preparation object publication, exact readback and replay;
- reference conflict without overwrite or temporary residue;
- changed native identity;
- oversize and invalid-name rejection before mutation.

`legacy_preparation_artifact_ownership` requires package ownership of
`OpenOptions`, hard-link publication, bounded readback, cleanup and the frozen
store schema. It rejects those mechanics/schema constants in daemon adapters
and confirms that secure revision writes remain routed to
`search-revision-store`, not `search-materializer`.

## Remaining Phase 3 work

Preparation artifact I/O and persisted schema now have one package owner.
Remaining work is physical preparation inventory classification/paging and the
legacy revision layout/model/inventory move to `search-revision-store`. Phase 4
then moves DPAPI/secret protection to `search-os-secrets`.

## Required execution

```text
cargo +1.98.0 test --locked -p search-materializer legacy_artifact
cargo +1.98.0 test --locked -p search-materializer legacy_store
cargo +1.98.0 test --locked -p search-materializer --test legacy_preparation_artifact_ownership
cargo +1.98.0 test --locked -p search-revision-store --test legacy_immutable_object_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test preparation_store_module_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test secure_direct_store_process
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-materializer -p search-revision-store -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-materializer -p search-revision-store -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
