# Test and harness cutover

Tests preserve product behavior, not pre-release package layout, receipt graphs or disconnected applications.
During refactoring, connect the real producer to the real consumer, run minimal pinned compiler/Clippy for the
changed production slice, and delete tests whose only subject disappears. Broad regression and scale suites
follow the connected application.

**Public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected tree:** `53ab0ea33f8d405f72e984881b5aea0db69e7c3b` (`R017`, 2026-10-23)

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

R016 removed the explicit snapshot daemon/CLI and six sealed Cargo test applications together with the private
snapshot control store, snapshot format, local BM25 implementation and structural isolation test. Those were
independent programs, not handlers of the selected daemon `entry.rs`.

Useful behavior from the removed snapshot prototype belongs to current owners: stable bounded acquisition,
no-clobber immutable objects, symlink/reparse refusal, exact range/line coordinates, incomplete-capture
truthfulness and bounded output. Its FNV identity, manual TCP/token protocol and local BM25 are not retained.

## R017: remove the lifecycle implementation, not just its tests

After R015 removed every exclusive restore/retention/purge test, static caller inspection found the remaining
production chain to be:

```text
wave7-lifecycle
→ entry.rs cfg module restore_composition
→ search-retention public restore/retention types
```

No current default handler called it. R017 removes daemon `restore_composition/**` and its `entry.rs` module, and
replaces the generic `search-retention` implementation with an intentionally empty no-API crate. `purge.rs` and
`sweep.rs` are deleted. This eliminates the same-installation restore, receipt phases, key migration, sole-copy
deletion, generic graph mark/sweep, leases and purge planes.

The empty package is not a compatibility facade. It remains only because removing the workspace member, daemon
optional dependency, wave feature and lockfile records must be one dedicated Cargo cut. No test should import or
recreate its old types.

The current-format causal requirements remain assigned to actual owners:

- stopped-root mark every typed current/published/pending/cleanup root before any deletion;
- unknown/corrupt reference metadata causes zero sweep;
- archive verification and complete marker occur before acceptance;
- import creates a fresh installation/Qdrant generation and executes no copied pending/cleanup authority;
- protected writable restore requires an independent real key/nonce context.

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

The explicit snapshot/sealed targets and old lifecycle implementation are gone. The current package still
contains:

- `wave1` through `wave7`/`full-baseline` feature composition and optional old-package wiring;
- an empty `search-retention` Cargo/workspace/lock shell pending coordinated removal;
- shared `sealed_*`, migration and compatibility/interceptor code used by remaining tests or old commands;
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
