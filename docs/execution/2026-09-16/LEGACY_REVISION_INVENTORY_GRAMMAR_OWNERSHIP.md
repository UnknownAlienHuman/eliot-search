# Legacy revision inventory grammar ownership

Date: 2026-09-16
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The pure layout and filename grammar used by the legacy DIRECT revision-object
inventory now belongs to `search-revision-store`.

The package owns:

- the canonical legacy root name `revisions`;
- the 65 MiB encoded-object ceiling;
- exact lower-case two-character shard names;
- final plaintext names `<lower-hex-64>.bin`;
- final protected names `<lower-hex-64>.dpapi`;
- historical temporary names `.<lower-hex-64>.<decimal-pid>.tmp`;
- current protected temporary names
  `.<lower-hex-64>.<decimal-pid>.<decimal-time>.dpapi.tmp`;
- the 192-byte basename ceiling;
- stable `catalog_referenced`, `unreferenced_revision_object` and
  `uncommitted_temporary_object` report tags;
- canonical shard-relative and `revisions/`-rooted locators.

Unknown extensions, upper-case/short/nonhex IDs, malformed decimal fields,
extra components, mismatched shard/identity pairs and overlong/non-ASCII names
fail closed. They are never silently reclassified as deletable residue.

## Daemon composition

`eliot-searchd` retains:

- data-root and revision-tree traversal;
- directory, symlink/reparse and metadata qualification;
- exact byte observation and SHA-256 fingerprinting;
- current source-catalog membership overlay;
- inventory digest, cursor and operator page construction;
- stable `DIRECT_MIGRATION_*` reason mapping.

`control_migration_orphans.rs` now calls the package parser, shard validator,
classification overlay and locator projector. Its duplicate `Kind` enum,
classification strings, final/temporary parser and lower-hex helper are removed.
The secure DIRECT kernel imports the root and object-size compatibility constants
from `search-revision-store` instead of defining duplicate literals.

## Compatibility

The move preserves exactly:

- admitted and rejected physical names;
- lexicographic inventory order;
- current catalog referenced/orphan overlay;
- relative and rooted locator strings;
- inventory digest inputs and kind tags;
- cursor/page digest domains;
- report field order and output bytes;
- existing limits and stable daemon failures.

No dependency, lockfile, persisted object, registry schema, workflow, gate or
launch state changes.

## Regression seams

Package tests cover:

- frozen root/object/name bounds;
- lower-case shard grammar;
- exact final plaintext/protected names;
- both historical and current temporary-name forms;
- upper-case, short, unknown, extra-component and overlong rejection;
- stable classification tags;
- canonical final, relative and rooted locators.

Cross-package ownership tests reject restoration of the daemon-local enum,
parser, suffix matching, classification strings or constant literals.

## Remaining Phase 3 work

The package now owns revision-object filesystem mechanics plus pure physical
layout/name grammar. The daemon still owns inventory observation, digest/cursor
page projection, catalog overlay orchestration and plaintext/protected decoding.
The next bounded move should transfer the pure revision inventory model/digest
projection, followed by Phase 4 secret protection in `search-os-secrets`.

## Required execution

```text
cargo +1.98.0 test --locked -p search-revision-store legacy_inventory
cargo +1.98.0 test --locked -p search-revision-store --test legacy_immutable_object_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test secure_direct_store_module_ownership
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-revision-store -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-revision-store -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.
