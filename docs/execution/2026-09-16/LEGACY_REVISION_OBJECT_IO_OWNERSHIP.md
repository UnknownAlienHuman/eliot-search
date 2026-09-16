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
- removal of only the exact original temporary object;
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
`publish_legacy_revision_object` / `read_legacy_revision_object`. The existing
preparation-store `persist_immutable_object` and generic bounded reader remain
separate until the accepted `search-materializer` slice; they are deliberately
not routed through `search-revision-store`.

`search-revision-store` is now a mandatory daemon dependency. `wave2-source`
remains as a compatibility stage marker but no longer controls whether the
baseline DIRECT revision owner is linked.

## Compatibility and hardening

The legacy final paths, `.bin` / `.dpapi` extensions, maximum object ceiling,
empty-file support and stable daemon reason namespace remain unchanged.

The new package owner adds stronger locator fencing than the former helper:

- native identity is checked before and after reads;
- the temporary object is read back completely before publication;
- directory creation validates the already admitted parent;
- a temporary locator is deleted only when its native identity still matches;
- a racing final object is reused only after exact byte comparison;
- a directory-sync failure after publication is explicitly outcome-unknown.

No protected/plaintext bytes are added to logs, receipts or debug output. No
lockfile, persisted object format, registry schema, secret format, marker or gate
state changed.

## Regression seams

Package tests cover:

- publish/read/exact replay;
- valid empty revisions;
- conflicting final bytes without replacement;
- changing native identity during read;
- invalid names and oversize rejection before mutation/allocation.

`legacy_immutable_object_ownership` verifies that:

- filesystem mechanics stay in `search-revision-store`;
- secret and source-registry responsibilities do not enter the package;
- secure revision writer uses the package-backed read/publish path;
- preparation objects remain on their separate owner path;
- the daemon dependency is non-optional.

## Remaining Phase 3 work

The package still does not own the full legacy source-registry occurrence model,
canonical residency-aware path derivation, DPAPI transition orchestration or
preparation/materialization storage. Subsequent bounded slices must move:

1. remaining revision path/model/inventory semantics to `search-revision-store`;
2. preparation object/reference lifecycle to `search-materializer`;
3. then Phase 4 secret protection to `search-os-secrets`.

The legacy file journal and registry remain preserved; marker presence or a
content digest alone cannot fabricate revision authority.

## Required execution

```text
cargo +1.98.0 test --locked -p search-revision-store immutable_object
cargo +1.98.0 test --locked -p search-revision-store --test legacy_immutable_object_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test secure_direct_store_process
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-revision-store -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-revision-store -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
