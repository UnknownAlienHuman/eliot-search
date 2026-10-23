# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
During refactoring, connect the real producer to the real consumer, run minimal pinned compiler/Clippy for the
changed production slice, and delete tests whose only subject disappears. Broad regression and scale suites
follow the connected application.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `8cf331a44decded2c84c7f579dcbf8434e040dc7` (`R015`, 2026-10-23)

## Classification rule

Every legacy test belongs to one of three buckets:

1. **Delete with no replacement** when it parses Rust/Cargo/Markdown text, freezes filenames/type placement,
   requires an obsolete feature or only proves an old state machine agrees with itself.
2. **Move the causal case** when it catches a real failure that still applies to the new owner. Rewrite the case
   against the current API; do not retain the old command, receipt or fixture to keep the test alive.
3. **Retain unchanged** only when it already executes current product behavior through the selected owner.

A subprocess, native call or large fixture does not make an obsolete architecture a product requirement.

## R015: exact current-tree disposition

### Removed source-shape tests

The following files read repository source/manifests and assert exact modules, marker strings, byte ceilings,
feature topology or ownership placement. They execute no product behavior and actively prevent inlining,
merging and deletion:

- `bins/eliot-searchd/tests/product_targets.rs`;
- `bins/eliot-searchd/tests/control_migration_owner_boundary.rs`;
- `bins/eliot-searchd/tests/direct_preparation_owner_boundary.rs`;
- `bins/eliot-searchd/tests/direct_store_registry_owner_boundary.rs`;
- `bins/eliot-searchd/tests/git_source_owner_boundary.rs`;
- `bins/eliot-searchd/tests/revision_inventory_model_ownership.rs`;
- `bins/eliot-searchd/tests/revision_key_derivation_ownership.rs`;
- `bins/eliot-searchd/tests/revision_protection_contract_ownership.rs`;
- `bins/eliot-searchd/tests/source_composition_owner_boundary.rs`;
- `bins/eliot-searchd/tests/stable_native_identity_api.rs`.

This removes 39,523 source bytes. `product_targets.rs` specifically required seven daemon and one CLI legacy
harness targets to remain, contradicting this cutover guide.

### Removed lifecycle proof family

These tests exclusively exercise the rejected `wave7-lifecycle`/`search-retention` platform rather than the
selected stopped-root GC and fresh-destination archive/import:

- `bins/eliot-searchd/tests/restore_process.rs`;
- `bins/eliot-searchd/tests/retention_process.rs`;
- `bins/eliot-searchd/tests/purge_process.rs`;
- `crates/search-runtime/search-retention/tests/purge_barriers.rs`;
- `crates/search-runtime/search-retention/tests/sweep.rs`.

They instantiate same-installation restore, purge planes, receipt phases, generic graph roots, leases/pins and
synthetic CAS adapters. Qdrant is absent or represented as unresolved proof. Removing these 83,573 source bytes
does not claim that GC, backup, restore or legal erasure is implemented.

The useful causal cases are retained below as requirements for the concrete owners:

- incomplete/unknown mark input causes zero deletion;
- foreign files and sole retained copies survive;
- interruption before sweep deletes nothing and interruption during sweep can leave only garbage;
- partial archive is refused and the completion marker is published last;
- original/archive remain intact until a fresh destination is verified;
- destination receives new installation and Qdrant identities and cannot mutate the original;
- missing/wrong protected-storage material fails without downgrade or copied nonce continuity;
- no physical secure-erasure claim is inferred from file absence.

### Mixed migration tests retained only for behavior extraction

`control_migration_bootstrap_boundary.rs` and `control_migration_inventory_owner_boundary.rs` contain real
Windows/filesystem cases mixed with source-text assertions. Do not delete their useful cases blindly:

- no-clobber target creation and retained parent-directory identity;
- bounded exact reads, missing versus non-file distinction and path-escape refusal;
- directory membership drift detection between pages;
- hard-link/alias refusal;
- incomplete/unknown metadata remains a gap rather than silent absence.

Move those cases to the current root/object/source/archive owners during their cutover. Delete the remaining
`include_str!`/marker-order assertions and legacy migration harness at that time.

## Remaining harness cuts

The current manifests still contain:

- seven daemon harness-only snapshot/sealed test targets;
- one CLI snapshot test target;
- `wave1` through `wave7`/`full-baseline` feature composition;
- feature-gated `restore_composition` and optional `search-retention`;
- process fixtures for migration, quarantine, pairing, recipes, overlays, old handles and manually joined
  Qdrant/DIRECT halves.

Remove each target with its last unique causal behavior. Do not replace the feature ladder with another test
profile, package map or acceptance feature. The default binaries must become the only product application.

## Target test portfolio

### Fast owner tests

Keep small deterministic tests next to the current implementation for:

- typed redb compare/set and restart readback;
- immutable object create/reuse/no-clobber/final verification;
- preparation empty/gap states, coordinate maps and structure-first unit boundaries;
- stable retrieval identity and published evidence binding;
- Qdrant request/response conversion and full versus narrow mutation semantics;
- publication cursor transitions, late-writer handling and view-guard cleanup eligibility;
- request/window bounds and commit after complete terminal output;
- framed protocol bounds, cancellation and partial I/O.

These tests call current functions. They do not inspect repository text, issue state or module placement.

### One connected smoke

The primary acceptance path uses default binaries and the real selected Qdrant client:

```text
initialize/open current root
→ ingest one real file
→ restart
→ publish and read back native BM25 state
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

A constructor test, process oracle, manual REST half, source-shape assertion or self-test string is not this
smoke.

### Focused connected regressions

After the path exists, add only causal cases: edit/rename; explicit/empty/unknown scope; correct IDF population;
old pinned view; unknown Qdrant mutation outcome; lost-index rebuild; cancellation/deadline; failed page output;
bounded mapped expansion; unsupported/binary/missing evidence; exact no-match; stopped-root GC; archive/import.

Optional PDF/model/overlay/MCP tests exist only after that feature has a real producer, consumer and selected
runtime.

## Execution policy

For each Rust cutover:

1. identify the old owner/API and its last real callers;
2. classify its tests as delete, move causal behavior or retain;
3. connect the new producer/consumer and delete the old implementation and exclusive fixtures in the same slice;
4. run Rust 1.98 `check` and strict Clippy for changed production owners and immediate consumers;
5. defer broad workspace/all-feature/all-test, native qualification, scale and long recovery matrices until the
   connected product exists.

Test volume is not evidence of a useful product boundary. Do not preserve a removed subsystem merely because
its fixtures are large.
