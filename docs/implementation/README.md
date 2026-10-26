# ELIOT Search — implementation spine

Read [architecture](../architecture/README.md) first. This document selects the shortest safe route to a working
application. Nearest `FUNCTIONS.md` files own local seams; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) owns evidence/status.

The product is pre-release. Internal APIs/formats need no compatibility facade. Preserve user files and sole
retained copies; remove obsolete receipt/proof/process machinery with actual callers.

## Final connected path

```text
selected source
→ stable acquired bytes
→ immutable ObjectRef
→ current SourceRecord
→ PreparedRef or explicit gap
→ physical Qdrant generation + atomic PublishedView
→ scoped Search or retained-source Exact
→ pinned EvidenceRef + verified excerpt
→ CLI/local-client result
```

Every arrow has one concrete owner. A package boundary may disappear; the responsibility may not.

## Minimal cross-owner values

```text
DataRootGuard            = retained root handle + exclusive lock + root metadata
ObjectRef                = format/profile/kind + content address + exact length
SourceRecord             = SourceId + non-reset row revision + current ObjectRef + selection facts
PreparedRef              = input ObjectRef + transform identity + prepared ObjectRef/outcome
PhysicalTextIndexProfile = field/filter schema + tokenizer/index/resource settings + text composition
QueryScoringProfile      = request-time BM25 k/b and semantic version
PublishedView            = physical collection + physical profile + owned redb read snapshot
EvidenceRef              = point/source/prepared identity + representation/range
RequestContext           = view + query profile + scope + absolute deadline/cancellation/counters
```

Stage 2/3A has no sequence. Query-time k/b is not physical schema and does not trigger a collection rebuild.

## Rules preventing implementation deadlock

1. Configuration is incremental; fields arrive with real consumers.
2. Plain storage is baseline; #598 encryption is optional.
3. Issues track ownership, not dependency order.
4. The one-file path uses final owners/persisted values.
5. Add traits/packages/registries/workers/donors only for concrete consumers.
6. One manager owns interfaces/integration; parallel writes are disjoint.
7. Complete immutable generations establish correctness before point mutation.
8. Stage 3A ships only for corpus classes meeting predeclared budgets; Stage 3B is required where it fails.
9. Physical schema/resource defaults are explicit; hidden Qdrant defaults are not a profile.
10. Minimal compiler/Clippy follows each production cut; broad evidence follows connected behavior.

<a id="delivery-sequence"></a>
## Delivery sequence

<a id="1-foundation--invocation-settings-and-root"></a>
### 1. Foundation — invocation, settings and root

Issues: #333, #266, #360.

```text
help/version without owner effects
→ parse args_os once
→ merge/validate bounded root/plain settings
→ inspect | initialize | open one current-format root
→ retain one DataRootGuard
→ factual status
```

Only config locator/bounds, `data_root` and `storage.profile=plain` ship here.

Initialization creates no user source state:

```text
validate empty/recognized target
→ acquire exclusion and retain root/parent handles
→ create exact incomplete marker
→ create/flush/close/verify minimal redb + object layout
→ publish final manifest no-clobber last
→ reopen/verify current root
→ remove marker/stage debris
```

Before final manifest, classification is `IncompleteInitialization`; ordinary open writes nothing. Explicit
`discard-incomplete` removes only an exact recognized empty layout. Unknown/conflicting/replaced entries refuse.

Windows finalization is owner-private and handle-bound; current path-based `tempfile::persist_noclobber` is not
trusted as final authority. Unix uses directory-relative no-replace or verified link/unlink fallback. File flush,
namespace publication, readback and sudden-power-loss durability are separate claims.

**Completion:** real entrypoint reaches root owner; unsupported roots fail before writes; one guard surrounds store
lifetime; status is factual; displaced config/root-lifecycle paths are removed or immediately bounded.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. Walking skeleton — one file to one CLI result

Coordinator #264; vertical owners include #267/#270/#330/#331/#263/#261/#272/#279/#291/#293/#280/#332.

Upgrade Qdrant Server/Rust client/schema/consumers together from current client 1.19 to one exact **1.20.x** patch.
Use one vectorless one-shard collection and one scoring payload field.

```text
one explicit supported file
→ one stable read
→ plain ObjectRef + current SourceRecord
→ one materialization/unitization result
→ one bounded retrieval_text per unit
→ fresh vectorless collection
→ explicit physical Text index V1
→ InsertOnly point batches + exact readback
→ deterministic TextQuery readiness canary
→ commit matching evidence/current state
→ install PublishedView
→ one TextQuery with QueryScoringProfileV1
→ verify pinned evidence/object/range
→ typed CLI output
```

Physical Text index V1:

```text
field=retrieval_text
tokenizer=Code
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords=None
min/max token length=None
nested=None
scoring=BM25
enable_hnsw=false
memory=Pinned (Stage-2 one-file profile)
```

Stage 2 has no candidate filter and creates no source/access indexes. It requests no vectors and only the routing
payload needed for evidence lookup. `retrieval_text` and excerpts do not return from Qdrant.

QueryScoringProfileV1 fixes `k=1.2`, `b=0.75`; validate finite upstream ranges. Changing only k/b does not rebuild
the collection. No baseline score threshold.

Code tokenizer already emits subwords and joined camel/snake identifier tokens for ranking. Do not append local
aliases. Free text is any-token nomination. Full-text filters, if later exposed, require all subwords in any order
and are neither exact identifier nor phrase predicates. Exact punctuation/location remains retained-source Exact.

Temporary limits: owning local account, plain storage, one file, one collection, one one-shot request. No watcher,
broad inventory, encryption, paging, aliases, custom sharding, Edge, filter indexes, point mutation or cleanup.

**Completion:** default binaries execute against real Qdrant 1.20 and return the correct retained excerpt. Schema,
resource settings and TextQuery canary read back exactly; missing/corrupt evidence cannot produce a card; no legacy
oracle/proof/local-BM25/duplicate-store path serves as intermediate.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence — correctness/reference profile

Issues: #270/#271/#331/#261/#300/#303 and affected owners.

Add restart/reopen, authoritative inventory, ABA-safe source/preparation installation, lost-wakeup rediscovery,
complete desired-versus-published comparison, full fresh collection build, matching evidence/current commit,
explicit view install, old-view lifetime and whole-collection cleanup.

Create only filter indexes used by real multi-source/access/path-kind clauses. Exact keyword/filter indexes set
`enable_hnsw=false` and explicit `Pinned|Cached|Cold` memory. Prefix/tenant optimization appears only for an actual
query pattern. Text-index residency is also explicit per corpus class; physical-profile change builds a fresh
generation.

Visible collections are immutable. Any selected-state difference builds another never-reused collection. No
intra-generation mutation or stable cross-generation identity. Capture complete desired input durably before
Qdrant effects.

Scope:

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Whole-collection base omits `idf`; explicit empty avoids RPC; unknown/incomplete never widens.

Measure predeclared corpus classes/workloads: build/edit latency, points/payload/writes, CPU/RAM/handles, text/filter
index residency/cache behavior, current+building+reader-retained collection disk, redb retained pages, update rate
versus rebuild throughput, cleanup backlog and max reader/window lifetime.

Stage 3A ships for a class only if correctness plus every declared budget passes and coalescing converges without
unbounded backlog.

<a id="3b-measured-incremental-publication"></a>
### 3B. Source-local incrementality — only where 3A budgets fail

Issues: #259/#329/#261/#263 plus unitizer/evidence owners.

Only then add source-local old/new merge, unambiguous occurrence reuse, operation-fixed births, explicit InsertOnly
additions, one verified non-creating closure, exact readback and one complete-source metadata commit. Individual
points are never deleted; profile/schema change or closed-point growth builds a fresh generation.

Because Qdrant text statistics may remain stale after point updates/deletes until optimization, qualify scoring and
optimizer convergence, not just mutation readback. Oversize source delta falls back to a fresh generation or gap.
If all target classes pass Stage 3A, close Stage 3B issues `not_planned` with measurements.

### 4. Exact and live session

Issues: #249/#275/#280/#332.

Add literal Exact over pinned retained inventory, then one serviced framed session with Continue/Expand. One bounded
`QueryWindow` holds final ordered references, original query profile and same view guard. Continue never requeries;
partial output advances no cursor.

### 5. Maintenance and product depth

Issues: #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336.

Add stopped-root collection/archive/import, then selected source-backed orientation/comparison/document/Git behavior.
Optional donors follow measured consumers. Qdrant Edge remains an isolated replacement experiment, never fallback.

<a id="lifetime-and-lock-order"></a>
## Lifetime and lock order

1. `DataRootGuard` before stores; drop after stores close.
2. Publisher holds no redb writer/view mutex during object/Qdrant I/O.
3. Query clones view under a short lock, releases it before scope/backend/evidence work.
4. Object/Qdrant/process/model I/O occurs without redb writer/window mutation borrow.
5. Redb writer performs bounded compare/commit only.
6. After publication commit, open/verify snapshot, then install view; no next publication before install/recovery.
7. Reserve capacity before retrieval; prepare evidence outside window lock; mutate cursor only after complete output.
8. Last guard drop wakes cleanup; collection deletion holds no query/view lock.

No global lock nests another global lock. Remove/adapt any old API requiring reverse order.

## Owners

| Responsibility | Recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source/current rows | [source](../../crates/search-source/README.md), [registry](../../crates/search-source/search-source-registry/FUNCTIONS.md), [reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) |
| Objects | [object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Lexical profiles | [lexical](../../crates/search-lexical/FUNCTIONS.md) |
| Control/Qdrant | [control](../../crates/search-control-redb/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query/scope/evidence/window | [query](../../crates/search-query/README.md), [scope](../../crates/search-query/search-access/README.md), [Exact](../../crates/search-query/search-exact/FUNCTIONS.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol/client/CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Offline maintenance | [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

## Slice completion contract

A slice completes only when real entrypoint reaches real consumer; ownership is direct; hidden defaults/private
prerequisites are absent; displaced routes/fixtures are removed or bounded next; unsupported/incomplete states fail
without writes/fallback/widening; lock order holds; minimal Rust 1.98 check/Clippy ran; registry records exact
SHA/commands/results/unrun work; and conflicting docs/issues are synchronized.
