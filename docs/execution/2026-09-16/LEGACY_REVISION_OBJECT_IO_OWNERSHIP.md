# Legacy revision-object I/O ownership move

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The bounded filesystem lifecycle for legacy DIRECT revision objects now belongs
to `search-revision-store`.

The package-owned adapter covers opaque `.bin` and `.dpapi` bytes and owns:

- parent/shard directory admission and bounded creation;
- local final/temporary name validation;
- bounded exact reads through one opened file;
- pre/post length, modification-time, locator and native-identity checks;
- empty revision objects;
- `create_new` temporary writes and file sync;
- complete temporary readback before publication;
- same-directory no-clobber hard-link publication;
- exact final readback and identity comparison;
- byte-identical idempotent reuse;
- immutable conflict refusal without overwrite;
- identity-bound temporary cleanup on success and every early exit;
- publication/durability outcome-unknown classification.

The package accepts platform observations through
`LegacyRevisionObjectPlatform`. It does not acquire credentials, run DPAPI,
interpret source identity, verify plaintext content, mutate the source registry
or handle preparation/materialization artifacts.

## Daemon composition

`eliot-searchd` retains:

- legacy revision-path derivation;
- native Windows/Unix file identity observation;
- reparse/symlink directory and locator qualification;
- temporary basename entropy/time;
- DPAPI/keyring composition and plaintext digest verification;
- historical `DIRECT_*` reason mapping.

Normal DIRECT revision publication and readback now enter
`publish_legacy_revision_object` / `read_legacy_revision_object`.
Preparation object/reference I/O is separately routed to
`search-materializer`; the two capabilities share only qualified daemon
platform observations and do not import one another's package internals.

`search-revision-store` is now a mandatory daemon dependency. `wave2-source`
remains as a compatibility stage marker but no longer controls whether the
baseline DIRECT revision owner is linked.

## Compatibility and hardening

The legacy final paths, `.bin` / `.dpapi` extensions, maximum object ceiling,
empty-file support and stable daemon reason namespace remain unchanged.

The package owner adds stronger locator fencing than the former helper:

- native identity is checked before and after reads;
- the temporary object is read back completely before publication;
- directory creation validates the already admitted parent;
- a temporary locator is deleted only when its native identity still matches;
- RAII cleanup covers conflict, readback and durability failures without ever
  unlinking a replacement locator;
- a racing final object is reused only after exact byte comparison;
- a directory-sync failure after publication is explicitly outcome-unknown.

No protected/plaintext bytes are added to logs, receipts or debug output. No
lockfile, persisted object format, registry schema, secret format, marker or gate
state changed.

## Regression seams

Package tests cover:

- publish/read/exact replay;
- valid empty revisions;
- conflicting final state without replacement or temporary residue;
- changing native identity during read;
- invalid names and oversize before mutation/allocation;
- two racing, different-byte publications with one immutable winner and no temp residue.

`legacy_immutable_object_ownership` verifies that:

- filesystem mechanics stay in `search-revision-store`;
- secret, registry and preparation responsibilities do not enter the package;
- secure revision writer uses the package-backed read/publish path;
- preparation I/O uses the distinct `search-materializer` owner;
- the daemon dependency is non-optional.

## Remaining Phase 3 work

Phase 3 now has separate package owners for retained revision-object I/O and
preparation artifact I/O. The remaining slices move legacy revision layout,
model and inventory semantics into `search-revision-store`, and preparation
path/reference-layout/inventory semantics into `search-materializer`. Phase 4
then moves DPAPI/secret protection to `search-os-secrets`.

The legacy file journal and registry remain preserved; marker presence or a
content digest alone cannot fabricate revision authority.

## Required execution

```text
cargo +1.98.0 test --locked -p search-revision-store immutable_object
cargo +1.98.0 test --locked -p search-revision-store --test legacy_immutable_object_ownership
cargo +1.98.0 test --locked -p search-materializer --test legacy_preparation_artifact_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test secure_direct_store_process
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-revision-store -p search-materializer -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-revision-store -p search-materializer -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
