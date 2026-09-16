# Legacy preparation-store schema ownership move

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The frozen persisted schema for legacy DIRECT preparation records now belongs
to `search-materializer` instead of `eliot-searchd`.

The package owns:

- exact `ELSPRP02` binding bytes;
- exact `ELSPRF01` lookup-reference bytes;
- binding/reference/header sizes and manifest/object ceilings;
- plaintext/protected storage tags and `.bin` / `.dpapi` extensions;
- materializer/unitizer profile revision fields;
- content, representation and manifest digest-algorithm tags;
- `eliot-search/direct-preparation-ref/v2` lookup-key preimage;
- `eliot-search/direct-preparation-object/v2` object-ID preimage;
- lower-case shard, reference and object basename derivation;
- manifest framing and strict typed validation;
- decoded payload length/SHA-256 verification;
- stable `DIRECT_PREPARATION_*` schema failure classification.

`eliot-searchd` now supplies only composition inputs:

- conversion of validated legacy digest text into exact bytes;
- canonical current materializer/unitizer profile digests;
- concrete SHA-256 and BLAKE3 implementations;
- active plaintext/DPAPI protection class and protect/unprotect calls;
- qualified filesystem reads/publication through the package-owned artifact
  lifecycle;
- data-root directory observations and historical daemon reason mapping.

No source bytes, path authority, secret acquisition, revision CAS or source
registry ownership moved into the materializer package.

## Compatibility

Persisted bytes remain unchanged:

```text
binding:
  ELSPRP02
  namespace[32]
  source_id[32]
  revision_id[32]
  content_sha256[32]
  byte_length_be[8]
  materializer_profile_digest[32]
  unitizer_profile_digest[32]

manifest:
  binding[208]
  representation_id[32]
  content_digest_algorithm[1]
  representation_digest_algorithm[1]
  materializer_revision_be[8]
  unitizer_revision_be[8]
  manifest_digest_algorithm[1]
  preparation_frame[bounded]

reference:
  ELSPRF01
  lookup_key[32]
  manifest_sha256[32]
  manifest_length_be[8]
  protection_tag[1]
```

The fixed sizes remain:

```text
binding = 208 bytes
historical minimum binding = 176 bytes
manifest header = 259 bytes
reference = 81 bytes
maximum layout = 64 MiB - 512 bytes
maximum encoded object = 65 MiB
```

The following are unchanged:

- persisted field order and endianness;
- profile names and revisions;
- SHA-256/BLAKE3 algorithm selection;
- lookup/object IDs and final locators;
- preparation gap tags and identity markers;
- stable daemon error strings;
- DPAPI/keyring behavior;
- source registry, revision object and preparation artifact bytes;
- dependencies, lockfile, workflows, gates and launch state.

## Daemon cutover

Daemon `persist`, `load` and migration inspection now call the package-owned
binding/reference/manifest codec. Hard-coded byte offsets and digest domains
were removed from the daemon. The daemon still verifies exact source-derived
preparation bytes during migration and still owns the live secret/filesystem
adapters.

`MAX_LAYOUT_BYTES` is re-exported from `search-materializer`, so profile digest
construction and persisted manifest admission share one format ceiling rather
than duplicate constants.

## Regression seams

Package tests freeze:

- byte-exact binding encode/decode;
- byte-exact reference encode/decode and protection tag;
- lookup/object digest domain separation;
- lower-case shard/reference/object names;
- manifest algorithm/profile field positions;
- every closed preparation gap;
- representation verification against binding plus exact frame marker;
- malformed/oversize/profile/digest failures;
- decoded payload length and SHA-256 binding.

Cross-package ownership tests reject restoration of schema magic, sizes,
offsets or digest domains in daemon files. Existing artifact-I/O tests continue
to cover no-clobber publication and native-identity fencing.

## Remaining Phase 3 work

Preparation artifact I/O and wire schema now have one package owner. Remaining
bounded work is:

1. move physical preparation inventory classification/paging semantics out of
   daemon while retaining data-root observation in composition;
2. move legacy revision layout/model/inventory semantics to
   `search-revision-store`;
3. then begin Phase 4 by moving DPAPI/secret-protection implementation to
   `search-os-secrets`.

## Required execution

```text
cargo +1.98.0 test --locked -p search-materializer legacy_store
cargo +1.98.0 test --locked -p search-materializer --test legacy_preparation_artifact_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test preparation_store_module_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test secure_direct_store_process
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-materializer -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-materializer -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
