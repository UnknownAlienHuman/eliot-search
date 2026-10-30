# Verification after the connected implementation

During implementation, use only the [minimal check/Clippy closure](WORKING_GUIDE.md#minimal-production-checks).
This page is the later behavior checklist, not a reason to build a test platform before the product.
No result below is claimed executed. [The registry](../audit/ANALYSIS_REGISTRY.md#evidence-status) records runs.

## Stage 1 — Foundation

Help/version have no filesystem effects. Existing/empty/unsupported/busy roots are distinguished. Initialize
manifest-last; reopen exactly; a second process gets Busy. Keep the same lock file through cleanup/restart.
No keys, profile selection, owner epochs or init replay. Unknown/user data is never removed to make init succeed.
Check private same-directory no-clobber publication, conflict and interruption on each supported OS. Report actual
durability rather than inferring it from successful readback. [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) owns details.

## Stage 2 — one file to one CLI excerpt

Run the actual binaries through acquisition → retained object/current row → preparation → real Qdrant → matching
published evidence → verified CLI excerpt. Check exact bytes/ranges, empty input, corrupt/missing evidence,
oversize input and cancellation. A mock backend or old local index is not that result.

[Lexical](../../crates/search-lexical/FUNCTIONS.md) and
[bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) own the exact schema, scoring, canary
and client/server version. Check those fields there instead of copying a second table into this checklist.
No watcher, encryption, permission framework or incremental point history is a prerequisite.

## Stage 3A — convergence and resources

Restart/reopen; edit/removal; partial inventory; stale acquisition/preparation and ABA; lost wake-ups; complete
fixed-input generation; commit/install interruption; old pinned view; whole-generation cleanup. Confirm scope
filters and IDF use the intended population and never widen an unknown/empty selection.

Before the measurement run, state corpus sizes and numeric latency, throughput, RAM/CPU/handles, retained disk,
read-snapshot and cleanup-backlog limits. Measure the actual pipeline. No corpus class is claimed supported merely
because a small example works. Resource limits are product measurements, not an agent approval bureaucracy.

## Stage 3B — only when needed

Only failed declared Stage 3A budgets justify implementing/testing source-local deltas. Then check fixed births/
closures, old-view evidence, readback, scoring convergence and compaction costs with the publisher's actual rules.
Do not add validity/sequence/occurrence machinery before measuring the simpler path.

## Later stages

Exact: representation-specific scan, cross-block matches, no cross-source matches, integrity and truthful gaps.
Session: complete terminal before display, cursor commit, expiry, cancellation, backpressure and child reap.
Maintenance: stopped-root marking, archive/import isolation, interrupted copy and original preservation.
Git/documents/models/adapters: only selected consumers; no-execute input and actual bounded behavior.
Application encryption and remote/restricted-client systems are outside [local scope](../architecture/LOCAL_SCOPE.md).

## Legacy-test disposition

Delete tests whose only purpose is freezing retired packages, waves, receipts, key rotation, grants or generated
source shapes. Move real data-integrity/range/cancellation/caller behavior with its implementation. Preserve an
old-data reader only for actual user-data access, not because old fixtures require it.

## Result discipline

Record exact commit, OS/target, toolchain, selected dependency versions/features, command and result. PASS, FAIL,
PARTIAL and NOT_RUN mean what they say. Static review and compiler output do not establish runtime correctness.
No broad test matrix, duplicated schema table or receipt generator is needed to record these facts.
