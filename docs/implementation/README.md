# ELIOT Search — implementation spine

Read [architecture](../architecture/README.md) first. This document selects the shortest path to a working
application. Nearest `FUNCTIONS.md` files own local seams and failure rules; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) records evidence, not architecture.

The product is pre-release. Internal APIs and formats need no compatibility facade. Preserve user files and sole
retained copies, but remove obsolete process/receipt/proof machinery with its callers.

## Final connected path

```text
selected source
→ stable acquired bytes
→ immutable ObjectRef
→ current SourceRecord
→ PreparedRef / explicit gap
→ Qdrant generation + atomic PublishedView
→ scoped Search or Exact
→ pinned EvidenceRef + verified retained excerpt
→ CLI/local-client result
```

Every arrow has one concrete owner. A package boundary may disappear; the responsibility may not.

## Minimal shared values

Do not design a universal contract crate around these names. They describe the information that must cross owner
boundaries:

```text
DataRootGuard      = retained root handle + one exclusive lock + current root metadata
ObjectRef          = profile/kind + content address + exact length
SourceRecord       = SourceId + non-reset row revision + current ObjectRef + current selection facts
PreparedRef        = exact input ObjectRef + transform identity + prepared ObjectRef/outcome
PublishedView      = generation + sequence + real redb read snapshot
EvidenceRef        = point/source/prepared identity + exact representation/range
RequestContext     = one absolute deadline + cancellation + finite counters
```

Vendor, OS and redb types remain private to their adapters where practical.

## Execution rules that prevent deadlock

1. **Configuration is incremental.** #333 is an umbrella, not a gate that must construct future owners. Slice 1
   ships invocation capture, merge/validation and only root/plain-storage fields. Later slices add source,
   Qdrant and query fields with their real consumer.
2. **Plain storage is baseline.** Implement one current-format plain immutable-object writer/reader now. #598 may
   later supply an encrypted profile through the same object boundary; it does not block source retention,
   preparation, publication, query or CLI work.
3. **Issues are ownership trackers, not prerequisites.** A vertical implementation cut may touch several issues.
   Do not wait for every horizontal issue to close before connecting the next real consumer.
4. **No throwaway product path.** The walking skeleton may support one explicit file, one-shot Search and one
   generation, but it uses the final root/object/source/preparation/publication/query owners.
5. **No speculative framework.** Add a trait, package, registry, worker or donor only for two real current
   implementations or one concrete consumer.
6. **One manager owns overlap.** Parallel work is limited to disjoint files. The integration manager resolves
   interfaces and removes the replaced path in the same commit or immediately following bounded commit.

## Delivery sequence

### 1. Foundation: invocation, settings and root

Issues: #333, #266, #360.

Observable result:

```text
help/version without owner effects
→ parse args_os once
→ merge/validate bounded bootstrap settings
→ inspect | initialize | open one current-format root
→ retain one DataRootGuard
→ factual status
```

Only settings consumed here ship now: config locator/bounds, `data_root`, and `storage.profile = plain`.
Qdrant/source/query settings remain absent until their owners are connected.

### 2. Walking skeleton: one file to one CLI result

Coordinating issue: #264. Owner issues include #267, #270, #330, #331, #259, #263, #261, #272, #293, #280,
#332.

Implement the narrowest end-to-end path before broad reconciliation:

```text
explicitly select one supported file
→ stable read once
→ plain put_object
→ commit one current source head
→ materialize/unitize once
→ persist one preparation result
→ build one fresh Qdrant generation
→ atomically install one PublishedView
→ one one-shot Search
→ resolve one pinned retained excerpt
→ render CLI output
```

Temporary limits are explicit: one local account, plain storage, one file, one generation, one-shot request, no
watcher, no live reconfiguration, no paging and no physical cleanup. These limits are not alternate
architectures.

This slice is the first product proof. Do not implement the complete reconciliation scheduler, optional formats,
encrypted storage or general session protocol before this path works.

### 3. Convergence and incremental publication

Issues: #271, #329, #261, #300, #303 plus affected source/preparation owners.

Add selected-root inventory, explicit/periodic reconciliation, restart/lost-wakeup recovery, ABA-safe source and
preparation installation, incremental publication and rebuild/compaction.

Incremental Qdrant rules:

- new points use never-reused stable IDs and insert-only creation;
- retries use the same fixed point bytes and verify exact readback;
- closure writes one fixed exclusive `valid_until` and may repeat only that value;
- individual points are never physically deleted in the baseline;
- closed points remain until a fresh generation rebuild/compaction;
- generation names are never reused;
- whole-generation cleanup waits for live view guards, then deletes the collection and verifies absence.

Because a point is never deleted and recreated inside one generation, a delayed duplicate insert cannot reopen a
closed point. This removes the need for isolated-route succession or a permanent possible-writer ledger.

### 4. Exact and live session

Issues: #249, #275, #280, #332 and direct query/evidence owners.

Add literal Exact over the pinned retained inventory, then one serviced framed live child session with
Continue/Expand. One process-local `QueryWindow` owns final ordered evidence and one `PublishedView` guard.
One-shot output exposes no reusable locator. Partial terminal output advances no cursor.

### 5. Maintenance and product depth

Issues: #302, #244, #281, #227, #335; optional #219/#239/#248/#334/#336.

Add stopped-root local-object collection, archive/import, then source-backed orientation, comparison, document and
Git behavior. Optional donors follow a measured consumer and cannot block the baseline.

## Owners

| Responsibility | Current recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source acquisition/current rows | [source](../../crates/search-source/README.md), [registry](../../crates/search-source/search-source-registry/FUNCTIONS.md), [reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) |
| Immutable objects | [object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Control/identity/Qdrant | [control](../../crates/search-control-redb/FUNCTIONS.md), [point identity](../../crates/search-index-qdrant/search-point-identity/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query/evidence/window | [query](../../crates/search-query/README.md), [Exact](../../crates/search-query/search-exact/FUNCTIONS.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol/client/CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Offline maintenance | [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

## Slice completion contract

A slice is complete when:

- its real entrypoint reaches its real consumer;
- state/effect ownership is direct and factual;
- the obsolete path and exclusive fixtures are removed or explicitly named as the next bounded removal;
- unsupported states fail without writes or silent widening;
- the smallest relevant Rust 1.98 compiler/Clippy closure ran;
- the registry records exact SHA, paths, command/result and unrun behavior.

Broad suites, native crash matrices, relevance and scale work follow the connected implementation. They do not
block writing the first production path.
