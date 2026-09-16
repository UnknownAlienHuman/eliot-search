# Source-content manifest ownership move

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 control-migration ownership

## Result

The frozen `eliot.source-content.v1` line schema and its exact accounting state
machine now belong to `search-control-redb::migration`.

The package owns:

- the immutable manifest header shape;
- exact object-row rendering and contiguous ordinals;
- the fixed profile digest;
- exact declared-versus-observed object accounting;
- checked plaintext byte totals;
- the final content-manifest summary row;
- stable legacy reason codes for invalid state, count and bounds.

`eliot-searchd::control_migration_content` now composes that owner. It retains
only the responsibilities that cannot enter the control package:

- opening the existing protected/plaintext retained revision through the source
  adapter;
- rereading the exact retained bytes;
- computing BLAKE3 from those bytes;
- native staging/publication I/O that has not yet moved;
- translating already validated legacy digest strings to canonical digest
  newtypes.

No source body, path, credential, protector or BLAKE3 hasher crosses into
`search-control-redb`.

## Compatibility

The emitted UTF-8 JSON lines remain byte-for-byte the existing format:

1. `source_content_header`;
2. one `source_content_readback` row per retained object;
3. `source_content_end`.

Field order, spelling, lower-case digest encoding, booleans, newline framing and
the content profile are unchanged. The profile SHA-256 remains:

```text
8d99f177a1cf8134710dc7b586c36f2e0229060755e33089e04e492f667af29f
```

The manifest record-chain, external file name, redb binding, cutover marker and
persisted database schema are unchanged. No dependency or lockfile changed.

## Regression seams

Package tests freeze the exact header/object/end bytes and reject:

- end before header;
- early end before the declared object count;
- rows after the declared count;
- repeated header/end transitions;
- invalid target/cardinality bounds.

`control_migration_owner_boundary` verifies that the daemon no longer contains
the schema strings while the control package receives no source-byte or secret
dependencies.

## Remaining ownership move

The daemon still owns the temporary-file, immutable hard-link publication and
record-chain file readback lifecycle. A later slice should move that filesystem
state machine behind an injected platform boundary in
`search-control-redb::migration`, while leaving retained-byte acquisition in the
source adapter.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb content
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_owner_boundary
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-control-redb -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, process-fixture,
T02 or independent-review PASS is claimed.
