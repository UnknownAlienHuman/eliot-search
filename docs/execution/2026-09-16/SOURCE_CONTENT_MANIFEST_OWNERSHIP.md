# Source-content manifest, record-chain and artifact ownership move

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 control-migration ownership

## Result

The frozen `eliot.source-content.v1` schema, exact accounting state machine,
shared source-import SHA-256 record chain and immutable record-artifact
lifecycle now belong to `search-control-redb::migration`.

The package owns:

- immutable manifest header/object/end rows and contiguous ordinals;
- the fixed content profile digest and exact object/source-byte accounting;
- the 512 MiB artifact and 8 KiB row ceilings;
- the frozen empty/row/end SHA-256 record-chain domains;
- no-clobber temporary artifact creation;
- exact second-pass row comparison against regenerated source replay;
- native-identity and locator revalidation through an injected platform;
- hard-link publication without replacement;
- full final record-chain readback, matching-final reuse and conflict refusal;
- deletion of only the exact original temporary object;
- typed state, I/O, identity, outcome-unknown and immutable-conflict failures.

`eliot-searchd` now composes those owners. It retains only responsibilities that
cannot enter the control package:

- replaying the legacy source registry;
- opening the existing protected/plaintext retained revision;
- rereading exact retained bytes and computing BLAKE3;
- providing qualified native identity, locator and directory-sync observations;
- generating a non-authoritative temporary basename;
- rendering the historical operator report and mapping typed failures to the
  existing stable reason namespace.

No source body, path, credential, protector or BLAKE3 hasher crosses into
`search-control-redb`.

## Compatibility

The emitted UTF-8 JSON lines remain byte-for-byte the existing format:

1. `source_content_header`;
2. one `source_content_readback` row per retained object;
3. `source_content_end`.

Field order, spelling, lower-case digest encoding, booleans, newline framing,
artifact names and the content profile are unchanged. The profile SHA-256 is:

```text
8d99f177a1cf8134710dc7b586c36f2e0229060755e33089e04e492f667af29f
```

The shared record chain remains:

```text
eliot-search/sha256-parts/v1\0
eliot-search/source-map-chain/v1
eliot-search/source-map-row/v1
eliot-search/source-map-end/v1
```

The frozen two-row fixture remains:

```text
a2a8dc044d640c9c5d91dea46b338425c6d1f457d3eee7693ce0b8da4ee51966
```

The redb binding, cutover marker and persisted database schema are unchanged.
No dependency, lockfile or workflow changed.

The new owner additionally closes the previous staging race: after the exact
second replay and immediately before publication, it reopens the temporary
locator, requires the original native identity and recomputes the complete
record chain. A replaced temporary object is not published under the
content-addressed final name.

## Regression seams

Package tests freeze the exact schema/profile/chain values and cover:

- exact second-pass comparison and final inspection;
- changed regenerated rows before publication;
- existing conflicting final state without overwrite;
- changed temporary native identity;
- nonlocal temporary names;
- invalid row framing and aggregate bounds.

`control_migration_owner_boundary` now requires the package owner to contain
`hard_link`, temporary cleanup and record readback. It rejects restoration of
`StagingFile`, `OpenOptions`, `BufReader`, `BufWriter`, `hard_link`,
`remove_file`, `read_until` or daemon-local record-chain state in the plan and
content adapters.

## Remaining ownership work

This completes the mapping-plan/source-content immutable record-artifact slice.
The inactive redb `.pending`/final lifecycle is already package-owned. Remaining
PR #193 work is in adjacent control-migration orchestration and cutover/native
marker I/O, followed by the accepted T02 direct-store and source-root moves.
Those slices must preserve the single source-registry owner and must not move
retained bytes or credential handling into the control package.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb migration::record_artifact
cargo +1.98.0 test --locked -p search-control-redb migration::record_chain
cargo +1.98.0 test --locked -p search-control-redb migration::content
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_owner_boundary
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-control-redb -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, process-fixture,
T02 or independent-review PASS is claimed.
