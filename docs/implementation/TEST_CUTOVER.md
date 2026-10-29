# Verification and legacy-test cutover

This is a future verification/disposition guide, not a prerequisite to build a test platform now. Implement the
connected slice first; after Rust changes run the smallest locked Rust 1.98 check/Clippy for actual production
targets and immediate consumers. Broad tests follow the connected product. Markdown-only changes need a diff,
link/anchor check and semantic agreement, not a Rust suite.

[Registry evidence](../audit/ANALYSIS_REGISTRY.md#evidence-status) is the only current status. This page contains no
independent checkpoint or passing-build claim. The [implementation sequence](README.md#delivery-sequence) owns
stages; algorithms, field tables and resource defaults stay with their operation owners.

## Evidence when the implementation exists

| Stage | Behavior to exercise through real consumers |
|---|---|
| Foundation | Side-effect-free inspect/help; strict manifest-last initialization/discard; actual exclusion; unsupported/foreign preservation; restart and shutdown. Use the [root finalization cases](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), not a second platform recipe. |
| One-file Search | Stable source bytes → object/preparation → actual Qdrant build/readiness → committed view → retained CLI excerpt. Verify the [lexical profile](../../crates/search-lexical/FUNCTIONS.md) through the [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), including real scoring rather than schema readback alone. |
| Stage 3A | Complete inventory and fixed build input; dropped wake-ups; edits/removal/empty/gaps; stale-work rejection; old views; commit/install crash; whole-collection cleanup. |
| Conditional Stage 3B | Only after failed 3A budgets: conservative matching, fixed births/closures, exact validity if used, pending recovery, old-view evidence and scoring convergence. No physical point reclaimer. |
| Exact/session | Complete versus incomplete retained scan; request bounds; cancellation; one active request with serviced control; internal evidence versus public locators; cursor commit after complete output; healthy errors, partial writes, expiry and disconnect. |
| Selected depth | Stopped-root maintenance/import and only selected real document/syntax/model/adapter profiles. Their absence does not block the lexical baseline. |

Known empty scope performs no RPC. Whole-collection base selection removes redundant base/IDF predicates, not
request-only candidate filtering. No baseline case requires sparse inference, local alias expansion, stable
cross-generation IDs, sequence/closure, encryption, watcher or another backend.

## Release measurements

Before advertising a corpus class, declare numeric latency/throughput/memory/disk budgets in #215 and retain raw
measurements: full build and edit-to-view latency, writes, update arrival versus coalesced build throughput,
peak resources, index residency/cache, retained redb pages, current/building/reader-held collection disk, cleanup
backlog and supported request/window lifetime. Stage 3A ships only where those budgets pass; 3B is conditional.

## Disposition of existing tests

Keep a test only when it exercises useful current behavior. Move a causal case with its real owner; delete tests
that freeze removed packages, waves, receipts, grants, old wire bytes, qualification records or source shapes.
Do not regenerate their goldens to disguise a removed design as compatibility.

The [fixture ownership note](../../tests/CRATE_FIXTURE_OWNERS.md) replaces the old matrix. No typed contribution
request, merge receipt, package-count target or new evaluator is required. Existing mixed migration dispositions
are historical evidence, not a reason to re-audit all tests before implementation.

## Reporting

Record exact SHA, platform, toolchain/service profile, command and actual outcome: PASS, FAIL, PARTIAL or NOT_RUN.
A successful library build, issue closure, supplied proof or readback alone does not establish crash safety,
resource bounds, relevance or application readiness. Keep unrun behavior explicit in the registry.
