# ELIOT Search — implementation spine

Read [architecture](../architecture/README.md) first. This document selects the shortest safe route to a working
application. Nearest `FUNCTIONS.md` files own local seams; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) owns evidence and status.

The product is pre-release. Internal APIs and formats need no compatibility facade. Preserve user files and sole
retained copies; remove obsolete receipt/proof/process machinery with its actual callers.

## Final connected path

```text
selected source
→ stable acquired bytes
→ immutable ObjectRef
→ current SourceRecord
→ PreparedRef or explicit gap
→ physical Qdrant generation + atomic PublishedView
→ scoped Search or Exact
→ pinned EvidenceRef + verified retained excerpt
→ CLI/local-client result
```

Every arrow has one concrete owner. A package boundary may disappear; the responsibility may not.

## Minimal cross-owner values

These are information contracts, not a universal contract crate:

```text
DataRootGuard  = retained root handle + one exclusive lock + current root metadata
ObjectRef      = format/profile/kind + content address + exact length
SourceRecord   = SourceId + non-reset row revision + current ObjectRef + selection facts
PreparedRef    = exact input ObjectRef + transform identity + prepared ObjectRef/outcome
PublishedView  = exact physical generation + retrieval profile + real redb read snapshot
EvidenceRef    = point/source/prepared identity + representation/range
RequestContext = one absolute deadline + cancellation + finite counters
```

Vendor, OS and redb types stay private to their adapters.

## Rules which prevent implementation deadlock

1. **Configuration is incremental.** Foundation ships only invocation, root and plain-storage fields. Source,
   Qdrant, query and optional-profile fields arrive with the consuming owner.
2. **Plain storage is baseline.** #598 may add encryption later through the same `ObjectRef`; it cannot block the
   plain path.
3. **Issues are ownership trackers.** Closure order is not implementation dependency order.
4. **No throwaway product path.** One-file, one-shot and fresh-generation limits use final owners and persisted
   values.
5. **No speculative framework.** Add a trait/package/registry/worker/donor only for a concrete consumer.
6. **One manager owns overlap.** Parallel work is disjoint; the manager owns interfaces, integration and removal
   of displaced routes.
7. **Correctness profile before optimization.** Complete immutable generation publication precedes stable point
   reuse and point-level mutation.
8. **A reference profile is not automatically the release profile.** Stage 3A must meet declared latency,
   throughput and disk budgets. If it does not, Stage 3B is required before release for that corpus class.
9. **No broad gate before behavior.** Minimal compiler/Clippy follows each production cut; broad runtime/scale
   evidence follows a connected path.

<a id="delivery-sequence"></a>
## Delivery sequence

<a id="1-foundation--invocation-settings-and-root"></a>
### 1. Foundation — invocation, settings and root

Issues: #333, #266, #360.

```text
help/version without owner effects
→ parse args_os once
→ merge/validate bounded BootstrapSettings
→ inspect | initialize | open one current-format plain root
→ retain one DataRootGuard
→ factual status
```

Only config locator/bounds, `data_root` and `storage.profile = plain` ship here. Source/Qdrant/query settings do
not exist yet.

Foundation creates no user source state. Initialization is final-manifest-last:

```text
validate empty/recognized target
→ acquire initialization exclusion
→ create exact incomplete-initialization marker
→ create/flush/close/verify minimal redb + object layout
→ publish final root manifest no-clobber last
→ reopen/verify current layout
→ remove marker/stage
```

Before the final manifest the root is `IncompleteInitialization`; ordinary open performs zero repair. Recovery is
explicit `discard-incomplete`, not replay/resume. It may remove only the exact recognized empty layout under the
actual exclusion. Unknown files, conflicting bytes or replacement objects refuse.

On Windows, final publication must be bound to the opened stage handle and verified root/destination directory.
`tempfile::persist_noclobber` is not the selected Windows finalizer because its current implementation performs
path-based `SetFileAttributesW` plus `MoveFileExW`. Implement one small root/object-owned native finalizer and test
it. On Unix, use an owner-private no-replace primitive or link/unlink fallback with exact final readback. Do not
create a generic filesystem framework.

**Completion:** the real default entrypoint reaches the root owner; unsupported/foreign roots fail before writes;
one guard remains live while stores exist; status reports actual opened owners; old config activation/root
lifecycle callers are removed or named as the next bounded deletion.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. Walking skeleton — one file to one CLI result

Coordinator: #264. Owners include #267/#270/#330/#331/#263/#261/#272/#293/#280/#332.

R049 selects Qdrant Server and Rust client **1.20.x** for this implementation cut. The repository lock still
contains client 1.19.0 and changes only with code. Qdrant 1.20 provides BM25 directly on a scoring full-text
payload index and a `code` tokenizer, eliminating sparse-vector inference and fixed `avg_len` management from the
baseline.

```text
explicitly select one supported local file
→ one stable read
→ plain put_object
→ commit one current source head
→ materialize/unitize once
→ persist one preparation result
→ create one fresh never-reused physical collection
→ store retrieval text as payload
→ create scoring full-text index with explicit code/text processing
→ explicit insert_only points + exact readback
→ commit matching evidence/current state
→ explicitly install one PublishedView
→ one TextQuery Search
→ resolve one pinned retained excerpt
→ render typed CLI output
```

Temporary limits: owning local account, plain storage, one explicit file, one complete fresh physical generation,
one one-shot request, no watcher, broad inventory, encrypted profile, paging, aliases or physical cleanup.

Query addresses the physical collection named by `PublishedView`; a Qdrant alias is not visibility or recovery
authority. Exact literal/name/location remains independent because code tokenization improves lexical recall but
is not exact identifier identity.

**Completion:** default binaries execute this path against real Qdrant 1.20 and return the retained excerpt.
Missing/corrupt evidence cannot produce a card. Help/status advertise only constructed handlers. No legacy
interceptor, oracle, proof floor or duplicate store serves as an intermediate.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence — correctness reference profile

Issues: #270/#271/#331/#261/#300/#303 and affected owners.

Add restart/reopen, authoritative startup/explicit/periodic inventory, ABA-safe source/preparation installation,
lost-wakeup rediscovery, complete desired-versus-published comparison, full build into a fresh never-reused
physical collection, matching evidence/current commit, explicit view install, old-view lifetime and whole-
generation cleanup.

Visible collections are immutable. A selected source change builds the next complete generation. Cross-generation
point IDs need not be stable. This is the required correctness/reference implementation and may be the shipped
mode for corpus classes where it meets budgets.

Before declaring it the release default, measure on declared small/medium/large corpora:

```text
T_build_p50 / T_build_p95
edit-observed → view-installed latency
points and payload bytes written
peak Qdrant + retained-object + temporary disk
peak memory/CPU/handles
old-generation retention under long readers
coalesced update arrival rate versus rebuild throughput
```

Full-generation mode qualifies for a corpus class only when its declared latency and disk budgets pass and the
publisher converges under the declared update workload without an unbounded pending/backlog. Numeric budgets are
release inputs recorded by #215; they are not invented by the implementation.

<a id="3b-measured-incremental-publication"></a>
### 3B. Measured incremental publication — required only where 3A fails budgets

Issues: #329/#259/#261/#263 plus unitizer/evidence owners.

If any target corpus class fails Stage 3A latency, throughput or disk budgets, implement incremental publication
for that class before release:

- unambiguous occurrence reuse supplies stable point identity inside one physical generation;
- new immutable points explicitly use `insert_only`;
- retries carry identical fixed ID/input/payload and exact readback;
- replacement writes one fixed non-creating exclusive `valid_until` plus exact readback;
- individual points are never physically deleted;
- profile/schema change or measured closed-point growth creates a fresh generation;
- no new pending operation targets a retired generation.

A transition from fresh-generation to incremental retrieval is an explicit profile/generation rebuild, never a
reinterpretation of existing points. If all declared corpus classes pass Stage 3A budgets, close Stage 3B issues
`not_planned` with measurements.

### Why custom sharding is not the baseline optimization

Qdrant custom shards are useful when each shard key is always queried independently. BM25/full-text statistics
are shard-local. A portfolio query across multiple shard keys can therefore compare scores derived from different
statistics populations; `idf.corpus` cannot reach into another shard. Local RRF/fusion would add another ranking
owner. Do not partition by root/source as a generic substitute for Stage 3B. Reconsider only for an explicitly
single-partition product scope with measured evidence.

### Why Qdrant Edge is not the baseline yet

Qdrant Edge is technologically attractive: embedded, single-process, sparse/BM25 capable and directly aligned
with immutable generation directories. However, the current official 0.8.0 line is beta, has a large embedded
storage closure, manual optimization/flush obligations, and recently reported WAL acknowledgement/replay defects
fixed in newer server source but not yet established by a stable released Edge line. Keep an isolated post-
walking-skeleton spike; do not replace the server baseline before a released version passes persistence,
Windows, BM25, filter, restart and size/latency checks.

### 4. Exact and live session

Issues: #249/#275/#280/#332 and query/evidence owners.

Add literal Exact over the pinned retained inventory, then one serviced framed child session with
Continue/Expand. One process-local `QueryWindow` owns ordered evidence and a `PublishedView` guard. One-shot output
exposes no reusable locator. Partial terminal output advances no cursor.

### 5. Maintenance and product depth

Issues: #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336.

Add stopped-root object collection and archive/import, then selected source-backed orientation, comparison,
document and Git behavior. Optional donors follow a measured consumer and cannot block baseline Search.

<a id="lifetime-and-lock-order"></a>
## Lifetime and lock order

1. `DataRootGuard` is acquired before stores and dropped after stores close. It is not per-request locking.
2. One publisher task/permit selects work, but holds no redb writer or view-manager mutex during object/Qdrant I/O.
3. Query clones current `PublishedView` under a short manager lock and releases it before backend/object work.
4. Object/Qdrant/process/model I/O occurs without a redb write transaction or QueryWindow mutation borrow.
5. Redb writer performs bounded compare/commit only; no RPC, filesystem I/O or corpus-sized encode.
6. After commit, open/verify matching snapshot, then take the short manager lock to install the view. Do not begin
   another publication until installation succeeds or recovery refuses serving.
7. Reserve output/window capacity before retrieval. Prepare evidence outside the window lock; mutate cursor/window
   only for bounded final commit after complete output.
8. Last guard drop may wake cleanup but never deletes inline. Generation deletion runs with no query/view lock.

No global lock nests another global lock. If an old API requires another order, remove or adapt it.

## Backend boundary

Qdrant Server 1.20 is the selected indexed backend. Do not add a backend trait, Tantivy fallback, Edge fallback,
custom-shard ranking layer or dual writer. Reconsider only after connected measurements show failure against
explicit setup, persistence, scoped-IDF, relevance, latency, memory or scale budgets. Any replacement prototype
must consume the same retained/prepared/evidence contracts and replace the derived backend cleanly.

## Owners

| Responsibility | Current recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source/current rows | [source](../../crates/search-source/README.md), [registry](../../crates/search-source/search-source-registry/FUNCTIONS.md), [reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) |
| Immutable objects | [object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Control/Qdrant | [control](../../crates/search-control-redb/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query/evidence/window | [query](../../crates/search-query/README.md), [Exact](../../crates/search-query/search-exact/FUNCTIONS.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol/client/CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Offline maintenance | [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

## Slice completion contract

A slice is complete only when its real entrypoint reaches its real consumer; ownership is direct; no optional
issue is a hidden prerequisite; displaced routes/fixtures are removed or bounded as the next deletion; unsupported
states fail without writes/fallback/widening; lock order is preserved; minimal Rust 1.98 check/Clippy ran; the
registry records exact SHA/commands/results/unrun work; and conflicting issues/indexes are synchronized.
