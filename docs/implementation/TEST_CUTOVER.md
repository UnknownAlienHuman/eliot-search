# Test and harness cutover

Tests preserve product behavior, not pre-release file layout, package count, receipts or disconnected
applications. During refactoring, connect the real producer to the real consumer, run minimal compiler/Clippy
for changed owners, and delete tests whose only subject disappears. Broad regression and scale suites follow
the connected application.

## Verified public-main inventory

At `c8e38b6`:

- `bins/eliot-searchd/tests` contains **76 Rust files**;
- **33** are named `*_module_ownership.rs`;
- **28** are named `*_process.rs`;
- `bins/eliot-searchd/Cargo.toml` additionally declares **seven harness-only test targets** for snapshot/sealed
  binaries;
- historical wave features compile different disconnected package sets rather than one selected application.

Representative source inspection shows three distinct categories.

### 1. Delete source-shape tests

Examples such as `owner_composition_module_ownership.rs`,
`config_composition_module_ownership.rs` and `public_runtime_service_module_ownership.rs` read Rust source as
text and assert:

- exact module filenames and `mod`/`pub use` spelling;
- exact function/type marker location;
- source byte ceilings;
- forbidden token strings;
- old lifecycle states, receipts, helper names and test-function names;
- absence/presence of obsolete files.

These tests actively prevent merging packages, inlining trivial facades and deleting owner/config/provider
state machines. Remove them with the old owner. Compiler visibility, dependency review and product behavior
replace them; do not create new source-token, AST, rustdoc or module-map enforcement.

### 2. Delete disconnected process proofs

`indexed_query_process.rs` explicitly states that it does **not** drive the served daemon. It constructs the
process-test in-memory Qdrant oracle, canonical plan, grants/access checkpoints, leg tickets, epoch pins,
caller readback and projector. Passing it proves the old proof stack agrees with itself.

`product_spine_process.rs` explicitly joins two separate halves:

```text
legacy daemon DIRECT commands
+ manual std::net HTTP calls to an exact Qdrant executable
```

It does not call the bridge from the daemon and does not provide one CLI search operation. Preserve useful
live-fixture mechanics only if the connected product test reuses them; delete the two-halves acceptance claim,
manual duplicate Qdrant client and old identity/payload assertions.

The same rule applies to process fixtures for migration, quarantine, pairing, purge, overlay, legacy owner
lifecycle, handle/continuation and other removed systems. A subprocess does not make obsolete behavior a
product requirement.

### 3. Move causal behavior cases

`direct_preparation_process.rs` drives a real binary and contains useful scenarios even though it targets the
old DIRECT command surface. Move the behavior, not the old command/store/credential harness:

- match crossing an internal unit boundary;
- exact line/byte coordinates without silent normalization;
- binary/unsupported preparation produces a gap, not an exact negative;
- retained historical revision survives source deletion and restart;
- same-content ABA reuses immutable content while the current head changes;
- overlapping literal matches are returned once each;
- finite result/output limit changes completion truthfully.

Other old tests may contain equally useful causal cases. Before deleting a family, list the defect each case
would catch. Move only cases that still apply to the new owner and express them through the current API.

## Target test portfolio

### Fast owner tests

Keep small deterministic tests next to the implementation for:

- typed redb compare/set and restart readback;
- immutable object create/reuse/no-clobber/final verification;
- materialization coordinate maps and structure-first unit boundaries;
- stable point identity and view-manifest binding;
- Qdrant request/response conversion and full versus narrow mutation semantics;
- publication cursor transitions and view-guard cleanup eligibility;
- request/window bounds and commit-after-complete-output;
- framed protocol bounds, partial I/O and exactly one terminal response.

These tests call current functions. They do not parse source files, inspect issue state or assert module
layout.

### One connected product smoke

The primary acceptance test uses default binaries and the real selected Qdrant path:

```text
initialize/open current root
→ ingest one real file
→ restart
→ publish/read back native BM25 point
→ CLI Search
→ pinned manifest lookup
→ exact retained excerpt
```

It fails if any half is mocked or manually joined. It does not call a process-test oracle, duplicate REST
client or test-only mutation API.

### Focused connected regressions

After the smoke path works, add only scenarios with causal value:

- edit and rename touch only affected work;
- explicit/empty/unknown source scope and correct base IDF population;
- old pinned view remains readable across publication;
- unknown Qdrant mutation outcome resolves from actual state;
- lost Qdrant state rebuilds from current retained/control state;
- cancellation/deadline stops work without renewing time;
- page failure advances no `QueryWindow` cursor;
- bounded expansion cannot leave the emitted item;
- unsupported/binary/missing evidence yields honest gaps;
- exact scan owns complete-negative evidence.

Optional PDF/model/overlay/MCP tests exist only after that feature has a real producer, consumer and selected
runtime.

## Refactor execution policy

For each Rust cutover:

1. identify the old owner/API and its last real callers;
2. list existing tests in three buckets: delete, move behavior, retain unchanged;
3. connect the new producer/consumer and delete the old implementation and exclusive fixtures in the same
   slice;
4. run minimal Rust 1.98 `check` and strict Clippy for changed production owners and immediate consumers;
5. run only focused causal cases needed for that slice;
6. defer full workspace/all-features/all-tests, native qualification, scale and long recovery matrices until
   the application path is integrated.

Do not keep a removed subsystem merely because its test suite is large. Test volume is not evidence of a
useful product boundary.
