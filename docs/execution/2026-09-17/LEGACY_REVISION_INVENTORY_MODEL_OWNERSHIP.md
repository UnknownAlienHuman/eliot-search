# PR #193: legacy revision inventory model ownership

Date: 2026-09-17
Tracking: issue #189 / PR #193 / T02 Phase 3

## Result

The pure revision-residue inventory model, digest preimages, cursor grammar,
bounded page selection and canonical operator report now belong to
`search-revision-store`.

The package owns:

- deterministic shard and object ordering;
- normalized relative locator, encoded length, Unix mtime and inventory kind;
- the 256-shard and 65,536-object ceilings;
- referenced/orphan/temporary counts and checked unreferenced-byte accounting;
- the frozen `revision-residue-inventory`, `revision-residue-shard` and
  `revision-residue-file` SHA-256 multipart preimages;
- the catalog/backend/inventory checkpoint preimage;
- the exact `o1.<lower-hex-64>.<canonical-decimal>` cursor grammar;
- stale-cursor and no-progress classification;
- the 32-object / 512 MiB page selection bounds;
- exact row JSON, page digest, continuation projection and
  `legacy-revision-orphans-v1` response bytes;
- the 60 KiB report ceiling;
- stable `DIRECT_MIGRATION_*` model/paging reason codes.

Concrete SHA-256 remains injected through `LegacyRevisionInventoryDigest`.
Filesystem traversal, qualified metadata, current catalog observation, exact
object reads and encoded-object SHA-256 remain daemon composition.

## Daemon composition

`control_migration_orphans.rs` now performs only:

1. malformed-cursor rejection through the package parser before I/O;
2. qualified data-root/shard traversal and metadata observation;
3. current catalog overlay when constructing normalized entries;
4. package inventory/checkpoint/page composition;
5. exact encoded-object fingerprinting for selected rows;
6. second inventory/catalog sweep and second selected-object fingerprint check;
7. package report rendering.

The daemon no longer owns local `Entry`, `Inventory` or `Cursor` models, digest
domain strings, counts, aggregate-byte arithmetic, cursor encoding, page
selection, row JSON, page digest or full response schema.

## Compatibility

The move preserves:

- accepted physical names and classification tags;
- BTree-ordered shard/object inventory;
- names/sizes/mtimes inventory digest preimages;
- catalog/backend/inventory checkpoint preimage;
- cursor spelling, lower-case digest and canonical decimal requirements;
- page object/byte bounds and no-progress behavior;
- row and full report field order and values;
- stable `DIRECT_MIGRATION_*` failures;
- read-only/no-deletion semantics;
- second-sweep and exact selected-object readback fencing.

No persisted revision object, catalog schema, dependency, lockfile, workflow,
authority record, gate or launch state changed.

## Regression seams

Package tests cover deterministic sorting/counts, duplicate and kind mismatch
rejection, canonical/stale cursors, byte-bounded pagination and byte-exact report
projection with frozen digest-domain outputs.

Cross-package ownership tests reject restoration in daemon of local inventory
models, cursor parser, digest domains, page constants or report schema.

## Required execution

```text
cargo +1.98.0 test --locked -p search-revision-store legacy_inventory
cargo +1.98.0 test --locked -p search-revision-store \
  --test legacy_inventory_model_ownership
cargo +1.98.0 test --locked -p eliot-searchd \
  --test revision_inventory_model_ownership
cargo +1.98.0 test --locked -p eliot-searchd \
  --test control_migration_process
cargo +1.98.0 check --locked \
  -p search-revision-store -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked \
  -p search-revision-store -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, Windows native-I/O,
T02 or independent-review PASS is claimed.

## Next bounded slice

Move DPAPI/secret-protection operation ownership from daemon composition into
`search-os-secrets` while leaving source identity, catalog policy, revision
admission and plaintext verification outside that package.
