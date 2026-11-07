# Verification after the connected implementation

During implementation, use only the [minimal check/Clippy closure](WORKING_GUIDE.md#minimal-production-checks).
This page is the later behavior checklist, not a reason to build a test platform before the product.
Checklist rows are not execution claims. [The registry](../audit/ANALYSIS_REGISTRY.md#evidence-status) records actual runs and their scopes.

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

#### Predeclared Stage 3A corpus qualification profile

This profile was predeclared before benchmarking; corpus and acceptance limits below are unchanged.
The 9e574180 optimized RELEASE fixed-input run is FULL_RELEASE_PASS at its recorded sampling/evidence scope;
[the registry](../audit/ANALYSIS_REGISTRY.md#release-fixed-input-resource-evidence) records pins, measurements,
UNKNOWN unsampled peaks and unexercised replacement. Earlier DEBUG failures/incomplete runs remain distinct.
This is not universal peak, replacement-workload, platform or whole-product qualification.

**Corpus class `ordinary-repo-1`:** exactly 1000 unique UTF-8 code/text/JSON files totalling exactly 128 MiB;
exactly one 8 MiB boundary file and every other file at most 256 KiB. Content derives from representative
project source/docs/JSON with deterministic unique markers, never identical duplicate bodies. Qdrant 1.20
text index residency is explicit **Pinned** for this class.

**Acceptance limits (pre-measurement proposals, parent-approved):**
Full Index <=120 s (the existing single-operation budget; no new allowance), noop <=60 s; sequential search
p95 <=500 ms / max 2000 ms and at least 2 queries/s; full throughput at least 1.1 MiB/s and 8.4 files/s.
Product combined CLI+daemon average CPU <=8.0 core-equivalents, Qdrant <=16.0; whole-machine utilization
<=80% average on 32 logical cores is an environment-validity condition, not product-regression proof.
Working set / handles: product combined <=4 GiB / 512, Qdrant <=8 GiB / 1024. Disk for Source/Prepared/CONTROL
<=2x corpus + 1 GiB. Separately, Qdrant storage <=8x corpus + 1 GiB after convergence and 10 min of cleanup;
transient replacement storage high-water <=16x corpus + 2 GiB. These backend bounds cover all current and
retained generations, WAL/index/payload and any snapshots. Measure on dedicated, initially empty benchmark
storage; report total Qdrant storage and all generations, not only live point count. The fixed-input run
measured total storage/observed high-water; generation-replacement high-water remains unexercised.
Stable known cleanup backlog is 0 beyond the current generation after 10 min; no partial generation is visible.
Retained caps remain the existing source caps; no application/model quota changes.

Resource counters use actual available peaks where possible and are reported as sampled maxima; an unknown peak
is UNKNOWN, never verified. Only failed declared Stage 3A budgets justify implementing/testing
[source-local deltas](#stage-3b--only-when-needed).

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
