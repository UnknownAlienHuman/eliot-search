# Legacy preparation inventory grammar ownership

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The pure filename and locator grammar used by the legacy DIRECT preparation
physical inventory now belongs to `search-materializer`.

The package owns:

- the only admitted direct trees: `refs` and `objects`;
- final reference names: `<lower-hex-64>.ref`;
- final object names: `<lower-hex-64>.bin` and
  `<lower-hex-64>.dpapi`;
- writer temporary names:
  `.<lower-hex-64>.<decimal-pid>.<decimal-time>.dpapi.tmp`;
- the 192-byte basename ceiling;
- lower-case two-character shard derivation;
- initial classifications `unmapped reference`, `unmapped object` and
  `temporary`;
- current-overlay classifications `current reference` and `current target`;
- stable report tags for all five classes;
- canonical reference/object relative locators;
- canonical `preparation/<relative>` report locator projection.

Unknown trees, extensions, upper-case/short/nonhex IDs, malformed temporary
fields, extra components and overlong/non-ASCII names fail closed. They are not
silently treated as deletable orphans.

## Daemon composition

`eliot-searchd` retains:

- data-root and preparation-directory traversal;
- symlink/reparse and directory qualification;
- file metadata, mtime and exact byte observation;
- current source-catalog iteration;
- profile/backend selection and reference decoding;
- inventory digest, cursor and page construction;
- read-only operator JSON and stable daemon errors.

The daemon no longer owns a duplicate `Kind` enum, kind tags, final/temporary
basename parser or current reference/object relative locator formatter.

## Compatibility

The move preserves exactly:

- accepted/rejected physical names;
- lexicographic `BTreeMap` inventory order;
- directory and relative-locator strings;
- current-reference/current-target overlay;
- inventory digest inputs and kind tags;
- cursor/page digest domains;
- report field order and output bytes;
- all limits and stable `DIRECT_MIGRATION_*` failures.

No dependency, lockfile, persisted artifact, workflow, gate or launch state
changed.

## Regression seams

Package tests cover:

- closed tree parsing;
- exact final reference/object classification;
- exact temporary grammar in both trees;
- rejection of upper-case, short, unknown, extra-component and overlong names;
- frozen classification tags;
- canonical relative/rooted locators.

Cross-package ownership tests reject restoration of the local daemon enum,
filename parser, suffix grammar or classification strings.

## Remaining Phase 3 work

Preparation filesystem I/O, wire schema and physical name/classification
grammar now have one package owner. The daemon still owns inventory observation,
digest/cursor/page projection and current catalog overlay orchestration. The next
bounded move should transfer the pure inventory model/digest/page projection,
then move legacy revision layout/model/inventory semantics to
`search-revision-store`.

## Required execution

```text
cargo +1.98.0 test --locked -p search-materializer legacy_inventory
cargo +1.98.0 test --locked -p search-materializer --test legacy_preparation_artifact_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test preparation_store_module_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-materializer -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-materializer -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
