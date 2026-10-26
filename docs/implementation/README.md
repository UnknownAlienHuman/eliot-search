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
DataRootGuard  = retained root handle + exclusive lock + current root metadata
ObjectRef      = format/profile/kind + content address + exact length
SourceRecord   = SourceId + non-reset row revision + current ObjectRef + selection facts
PreparedRef    = input ObjectRef + transform identity + prepared ObjectRef/outcome
PublishedView  = physical collection + retrieval profile + owned redb read snapshot
EvidenceRef    = point/source/prepared identity + representation/range
RequestContext = absolute deadline + cancellation + finite counters
```

Stage 2/3A `PublishedView` has no sequence. Sequence/validity exists only if Stage 3B is selected.

## Rules which prevent implementation deadlock

1. Configuration is incremental: fields arrive with their real consumer.
2. Plain storage is baseline; #598 encryption is optional.
3. Issues track ownership, not prerequisite order.
4. The one-file path uses final owners and persisted values; it is not throwaway.
5. Add a trait/package/registry/worker/donor only for a concrete consumer.
6. One manager owns interfaces/integration; parallel writes are disjoint.
7. Complete immutable generations establish correctness before point-level optimization.
8. Stage 3A may ship only for corpus classes meeting predeclared budgets; Stage 3B is required where it fails.
9. Minimal compiler/Clippy follows each production cut; broad evidence follows connected behavior.

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

Only config locator/bounds, `data_root` and `storage.profile = plain` ship here. Source/Qdrant/query settings do
not exist yet.

Initialization creates no user source state:

```text
validate empty/recognized target
→ acquire exclusion and retain root/destination-parent handles
→ create exact incomplete marker
→ create/flush/close/verify minimal redb + object layout
→ publish final manifest no-clobber last
→ reopen/verify current root
→ remove marker/stage debris
```

Before final manifest, classification is `IncompleteInitialization`; ordinary open performs zero repair. Explicit
`discard-incomplete` removes only the exact recognized empty layout under exclusion. Unknown/conflicting/replaced
entries refuse.

Windows finalization is owner-private and handle-bound to the opened stage plus retained destination parent. The
current path-based `tempfile::persist_noclobber` implementation is not trusted as final authority. Run the native
canary before supporting the profile. Unix uses a directory-relative no-replace primitive or verified link/unlink
fallback. File flush, namespace publication, readback and sudden-power-loss durability are separate claims.

**Completion:** real entrypoint reaches root owner; unsupported roots fail before writes; one guard surrounds store
lifetime; status is factual; old config/root-lifecycle routes are removed or the next bounded deletion.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. Walking skeleton — one file to one CLI result

Coordinator #264; owners include #267/#270/#330/#331/#263/#261/#272/#279/#291/#293/#280/#332.

Upgrade Qdrant Server/Rust client/schema/real consumers together from the current 1.19 lock to one exact **1.20.x**
patch. Use a vectorless physical collection with direct BM25 over a scoring full-text payload index.

```text
one explicit supported file
→ one stable read
→ plain ObjectRef + current SourceRecord
→ one materialization/unitization result
→ bounded retrieval_text per unit
→ one fresh never-reused vectorless collection
→ create scoring Text index (Code tokenizer, explicit processing)
→ create source/access filter indexes
→ insert_only point batches + exact readback
→ deterministic TextQuery readiness canary
→ commit matching evidence/current state
→ install one PublishedView
→ one TextQuery Search
→ pinned evidence/object/range verification
→ typed CLI output
```

Temporary limits: owning local account, plain storage, one file, one ordinary shard, one complete collection, one
one-shot request. No watcher, broad inventory, encryption, aliases, custom sharding, Edge, paging, point mutation or
physical cleanup.

Stage 2's one-file collection is the complete selected BM25 population, so query omits redundant filter and IDF.
`TextQuery` matches any term. Required/excluded terms exist only in an explicit typed request; punctuation-sensitive
spelling/location remains Exact.

Preparation supplies retrieval text once. Do not append snake/camel/acronym aliases: Qdrant's code tokenizer
already splits naming conventions and indexes the joined identifier; duplication changes term frequency/length.

Point IDs are deterministic and generation-bound. No old/new occurrence matching, stable cross-generation identity,
sequence or closure is needed.

**Completion:** default binaries execute against real Qdrant 1.20 and return the correct retained excerpt. Missing
or corrupt evidence cannot produce a card. Help/status expose only constructed handlers. No legacy oracle/proof/
duplicate store serves as intermediate.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence — correctness/reference profile

Issues: #270/#271/#331/#261/#300/#303 and affected owners.

Add restart/reopen, authoritative startup/explicit/periodic inventory, ABA-safe source/preparation installation,
lost-wakeup rediscovery, complete desired-versus-published comparison, full fresh physical collection build,
matching evidence/current commit, explicit view install, old-view lifetime and whole-collection cleanup.

Visible collections are immutable. Any selected-state difference builds another never-reused collection. No
intra-generation mutation or stable cross-generation identity. Capture complete desired input into durable target
metadata before Qdrant effects; all pages and recovery use those fixed refs.

Measure predeclared corpus classes/workloads:

```text
full build p50/p95
edit observed → view installed p50/p95
points/payload bytes/backend writes
peak memory/CPU/handles
current + building + reader-retained collection disk
redb retained-page growth
update arrival rate versus rebuild throughput
cleanup backlog and maximum reader/window lifetime
```

Stage 3A may ship for a class only if correctness plus every declared latency/throughput/resource/disk budget passes
and coalescing converges without unbounded backlog.

<a id="3b-measured-incremental-publication"></a>
### 3B. Source-local incrementality — only where 3A budgets fail

Issues: #259/#329/#261/#263 plus unitizer/evidence owners.

Then, and only then, add:

- source-local old/new descriptor merge;
- unambiguous one-old/one-new occurrence reuse;
- operation-fixed births for ambiguous/unmatched units;
- explicit insert-only additions;
- one verified fixed non-creating `valid_until` closure;
- exact readback and one complete-source metadata/current commit;
- fresh-generation compaction/profile change.

Individual points are never physically deleted. Oversize source delta falls back to fresh generation or explicit
gap. If all target classes pass Stage 3A, close Stage 3B issues `not_planned` with measurements.

Qdrant BM25 statistics are shard-local. Do not use per-root/source custom shards as a generic portfolio
optimization; cross-shard scores would require another fusion owner. Custom sharding is only reconsidered for a
scope always queried as one partition.

### 4. Exact and live session

Issues: #249/#275/#280/#332.

Add literal Exact over the pinned retained inventory, then one serviced framed child session with Continue/Expand.
One bounded process-local `QueryWindow` holds final ordered references and the same view guard. One-shot output has
no reusable locator; partial terminal output advances no cursor.

### 5. Maintenance and product depth

Issues: #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336.

Add stopped-root object collection and archive/import, then selected source-backed orientation/comparison/document/
Git behavior. Optional donors follow a measured consumer. Qdrant Edge remains an isolated replacement experiment,
never fallback/dual backend.

<a id="lifetime-and-lock-order"></a>
## Lifetime and lock order

1. `DataRootGuard` is acquired before stores and dropped after stores close.
2. One publisher selects work but holds no redb writer/view mutex during object/Qdrant I/O.
3. Query clones current view under a short lock and releases it before scope/backend/evidence work.
4. Object/Qdrant/process/model I/O occurs without redb writer or window mutation borrow.
5. Redb writer performs bounded compare/commit only.
6. After publication commit, open/verify matching snapshot, then install view under a short lock; no next publication
   begins before installation or restart refusal.
7. Reserve capacity before retrieval; prepare evidence outside window lock; mutate cursor only after complete output.
8. Last guard drop only wakes cleanup; collection deletion runs without query/view-manager lock.

No global lock nests another global lock. Remove/adapt any old API requiring reverse order.

## Qdrant/query profile

The selected vectorless lexical profile is:

```text
payload field retrieval_text
Text index: tokenizer=Code, lowercase=true, ascii_folding=false,
stemmer=none, stopwords=none, scoring=BM25, nested=false
query: TextQuery with explicit profile k/b
```

For scope:

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Whole-collection base omits `idf`. Explicit empty returns empty without RPC. Unknown/incomplete never widens.
Every filter field is payload-indexed. Different physical collections/profiles are separate score populations.

Qdrant is the only selected indexed backend. No backend trait, Tantivy/Edge fallback, alias authority or dual writer.
A replacement prototype may occur only after connected measurements fail explicit criteria and must replace the
bridge behind the same retained/prepared/evidence boundary.

## Owners

| Responsibility | Current recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source/current rows | [source](../../crates/search-source/README.md), [registry](../../crates/search-source/search-source-registry/FUNCTIONS.md), [reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) |
| Immutable objects | [object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Lexical profile | [lexical](../../crates/search-lexical/FUNCTIONS.md) |
| Control/Qdrant | [control](../../crates/search-control-redb/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query/scope/evidence/window | [query](../../crates/search-query/README.md), [scope](../../crates/search-query/search-access/README.md), [Exact](../../crates/search-query/search-exact/FUNCTIONS.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol/client/CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Offline maintenance | [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

## Slice completion contract

A slice completes only when real entrypoint reaches real consumer; ownership is direct; no optional/private hidden
prerequisite exists; displaced routes/fixtures are removed or bounded next; unsupported/incomplete states fail
without writes/fallback/widening; lock order holds; minimal Rust 1.98 check/Clippy ran; registry records exact
SHA/commands/results/unrun work; and conflicting docs/issues are synchronized.
