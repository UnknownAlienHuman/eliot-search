# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
During refactoring, connect the real producer to the real consumer, run minimal pinned compiler/Clippy for the
changed production slice, and delete tests whose only subject disappears. Broad regression and scale suites
follow the connected application.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `bf2bde299b3f009527397571bd9baeea51fde840` (`R016`, 2026-10-23)

## Classification rule

Every legacy test belongs to one of three buckets:

1. **Delete with no replacement** when it parses Rust/Cargo/Markdown text, freezes filenames/type placement,
   requires an obsolete feature or only proves an old state machine agrees with itself.
2. **Move the causal case** when it catches a real failure that still applies to the new owner. Rewrite the case
   against the current API; do not retain the old command, receipt or fixture to keep the test alive.
3. **Retain unchanged** only when it already executes current product behavior through the selected owner.

A subprocess, native call or large fixture does not make an obsolete architecture a product requirement.

## R015: structural and lifecycle proof removal

R015 removed ten files that read repository source/manifests and asserted exact modules, marker strings, byte
ceilings, feature topology or ownership placement. It also removed daemon `restore_process`, `retention_process`,
`purge_process` and package `search-retention/tests/{purge_barriers,sweep}.rs`, which exclusively exercised the
rejected `wave7-lifecycle`/generic retention platform.

The useful causal requirements survive without those state machines:

- incomplete/unknown mark input causes zero deletion;
- foreign files and sole retained copies survive;
- interruption before sweep deletes nothing and interruption during sweep can leave only garbage;
- partial archive is refused and the completion marker is published last;
- original/archive remain intact until a fresh destination is verified;
- destination receives new installation and Qdrant identities and cannot mutate the original;
- missing/wrong protected-storage material fails without downgrade or copied nonce continuity;
- no physical secure-erasure claim is inferred from file absence.

## R016: remove eight isolated applications

The exact current manifests and source at `bf2bde29` showed eight explicit Cargo test applications:

- daemon `eliot-search-snapshotd` from `src/main.rs`;
- daemon `eliot-search-sealed-{authority,catalog,direct,recover,store,transaction}` from `src/bin/**`;
- CLI `eliot-search-snapshot` from its own `src/main.rs`.

The snapshot daemon was a separate loopback TCP/token service with its own alternating control store, immutable
snapshot format and complete in-memory BM25 index. The snapshot CLI spoke only that old protocol. The six sealed
programs were separate Windows owner/catalog/store/transaction/access applications. None was a handler called by
the selected daemon `entry.rs`; their only Cargo callers were the explicit test-target declarations. The remaining
`legacy_snapshot_isolation.rs` merely required this topology and source split to stay in place.

R016 removes the eight target declarations and their exclusive source:

- daemon snapshot `src/main.rs`, `control_store.rs`, `snapshot.rs`, `snapshot/**`, `lexical/**`;
- six daemon sealed entrypoint files under `src/bin/`;
- CLI snapshot `src/main.rs`;
- `tests/legacy_snapshot_isolation.rs`.

This does **not** claim that the shared `sealed_*`, migration, key/object or current daemon modules are now gone;
several remain imported by `entry.rs` or other tests. It removes only the independently compiled applications.
No workspace member or dependency changed, so `Cargo.lock` is intentionally unchanged.

Useful behavior from the removed snapshot prototype is already assigned to current owners and must be tested
there: stable bounded acquisition, no-clobber immutable objects, symlink/reparse refusal, exact range/line
coordinates, incomplete-capture truthfulness and bounded output. Do not preserve its FNV snapshot identity,
manual TCP/token protocol or local BM25 implementation to keep those cases.

Protected-object behavior belongs to the accepted object/key owner and current root/object tests. The old sealed
programs' owner epochs, purge/access receipts and transaction CLIs are not donors for that behavior.

## Mixed migration tests retained only for behavior extraction

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

The explicit snapshot/sealed Cargo targets are gone. The current package still contains:

- `wave1` through `wave7`/`full-baseline` feature composition and optional old-package wiring;
- feature-gated `restore_composition`, shared `sealed_*` modules and compatibility/interceptor code;
- process fixtures for migration, quarantine, pairing, recipes, overlays, old handles and manually joined
  Qdrant/DIRECT halves;
- lexical fixture-backed activation/profile authority.

Remove each owner with its last real caller and move only its causal behavior. Do not replace the wave ladder
with another test profile, package map or acceptance feature. The default binaries must become the only product
application.

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
