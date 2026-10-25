# ELIOT Search — implementation spine

Read [architecture](../architecture/README.md) first. This document selects the shortest safe path to a working
application. Nearest `FUNCTIONS.md` files own local seams; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) owns evidence and status.

The product is pre-release. Internal APIs and formats need no compatibility facade. Preserve user files and sole
retained copies, but remove obsolete receipt/proof/process machinery with its actual callers.

## Final connected path

```text
selected source
→ stable acquired bytes
→ immutable ObjectRef
→ current SourceRecord
→ PreparedRef or explicit gap
→ Qdrant generation + atomic PublishedView
→ scoped Search or Exact
→ pinned EvidenceRef + verified retained excerpt
→ CLI/local-client result
```

Every arrow has one concrete owner. A package boundary may disappear; the responsibility may not.

## Minimal cross-owner values

These are information contracts, not a mandate for a universal contract crate:

```text
DataRootGuard  = retained root handle + one exclusive lock + current root metadata
ObjectRef      = format/profile/kind + content address + exact length
SourceRecord   = SourceId + non-reset row revision + current ObjectRef + selection facts
PreparedRef    = exact input ObjectRef + transform identity + prepared ObjectRef/outcome
PublishedView  = generation + sequence + real redb read snapshot
EvidenceRef    = point/source/prepared identity + representation/range
RequestContext = one absolute deadline + cancellation + finite counters
```

Vendor, OS and redb types stay private to their adapters where practical.

## Rules which prevent implementation deadlock

1. **Configuration is incremental.** #333 is an umbrella. Foundation ships only invocation, root and plain-storage
   fields. Source, Qdrant, query and optional-profile fields arrive with the owner which consumes them.
2. **Plain storage is baseline.** #598 may add an encrypted profile later through the same `ObjectRef` boundary.
   It cannot block the plain walking skeleton.
3. **Issues are ownership trackers.** Issue closure order is not implementation dependency order. If an issue
   conflicts with this spine or its owner recipe, update the issue before coding.
4. **No throwaway product path.** One-file/one-shot limits use final owners and values.
5. **No speculative framework.** Add a trait/package/registry/worker/donor only for a concrete consumer; a trait
   normally requires multiple real implementations.
6. **One manager owns overlap.** Parallel work is disjoint. The manager owns interfaces, integration and removal
   of the displaced path.
7. **No broad gate before behavior.** Minimal compiler/Clippy follows each production cut; broad runtime and scale
   work follows a connected path.

## Delivery sequence

### 1. Foundation — invocation, settings and root

Issues: #333, #266, #360.

```text
help/version without owner effects
→ parse args_os once
→ merge/validate bounded BootstrapSettings
→ inspect | initialize | open one current-format root
→ retain one DataRootGuard
→ factual status
```

Only fields consumed here ship now: config locator/bounds, `data_root`, and `storage.profile = plain`.
Source/Qdrant/query settings do not exist yet. Initialization creates the minimal plain current-format
control/object layout. Encryption is not involved.

**Completion:** the real default entrypoint reaches the root owner; unsupported/foreign roots fail before writes;
one guard remains live while opened stores exist; old config activation and owner-lifecycle callers are removed or
explicitly bounded as the immediately following deletion.

### 2. Walking skeleton — one file to one CLI result

Coordinating issue: #264. Owners include #267, #270, #330, #331, #259, #263, #261, #272, #293, #280 and #332.

```text
explicitly select one supported local file
→ stable read once
→ plain put_object
→ commit one current source head
→ materialize/unitize once
→ persist one preparation result
→ create one fresh Qdrant generation
→ atomically install one PublishedView
→ one one-shot Search
→ resolve one pinned retained excerpt
→ render typed CLI output
```

Temporary limits: owning local account, plain storage, one explicit file, one fresh generation, one one-shot
request, no watcher, broad inventory, encrypted profile, paging or physical cleanup.

The cut persists exact root/object/source/prepared/view references and can reopen the object through the current
root. Full process-restart convergence and lost-wakeup recovery are stage 3; they do not block the first
end-to-end result.

**Completion:** default binaries execute this path against real Qdrant and return the retained excerpt. Missing or
corrupt evidence cannot produce a card. Help/status advertise only constructed handlers. Legacy interceptors,
oracles, proof floors and duplicate stores are not used as intermediates.

### 3. Convergence and incremental publication

Issues: #270, #271, #331, #329, #261, #300, #303 and affected owners.

Add restart/reopen, desired-versus-current reconciliation, selected-root inventory, ABA-safe source/preparation
installation, incremental publication, lost-index rebuild and compaction.

Qdrant rules:

- explicitly use `insert_only` for a new immutable point; default upsert is forbidden;
- retries carry the identical fixed ID/input/payload and exact readback;
- closure is one non-creating fixed `valid_until = target_sequence` mutation and exact readback;
- closure may occur while old views live because their sequence remains below the exclusive boundary;
- individual point IDs are never physically deleted;
- rebuild/compaction creates a never-reused generation;
- no new publication may target a retired generation;
- whole-generation cleanup waits for all view/window guards, performs terminal collection deletion, verifies exact
  logical absence and never reuses the name;
- uncertain/failed cleanup retains its row and applies bounded backpressure; logical absence is not a claim that
  every backend file is physically erased.

These invariants make identical late point retries harmless and remove isolated-route succession, per-point
possible-writer ledgers and point cleanup journals. The pinned Qdrant 1.19 behavior still requires focused live
verification before claiming runtime safety.

### 4. Exact and live session

Issues: #249, #275, #280, #332 and direct query/evidence owners.

Add literal Exact over the pinned retained inventory, then one serviced framed live child session with
Continue/Expand. One process-local `QueryWindow` owns final ordered evidence and one `PublishedView` guard.
One-shot output exposes no reusable locator. Partial terminal output advances no cursor.

### 5. Maintenance and product depth

Issues: #302, #244, #281, #227, #335; optional #219/#239/#248/#334/#336.

Add stopped-root local-object collection and archive/import, then selected source-backed orientation,
comparison, document and Git behavior. Optional donors follow a measured consumer and cannot block baseline
Search.

## Lifetime and lock order

The implementation must preserve this partial order; do not introduce a reverse acquisition path.

1. **Process lifetime:** `DataRootGuard` is acquired before concrete stores and dropped after they close. It is not
   a mutex held/released per request.
2. **Publication serialization:** one publisher task or one narrow permit chooses work. It may own logical
   serialization across an attempt, but holds no redb writer or view-manager mutex during object/Qdrant I/O.
3. **View pin:** query clones current `PublishedView` under a short manager lock, then releases the lock before
   scope resolution, Qdrant or object reads.
4. **Backend/object I/O:** occurs without a redb write transaction and without a query-window mutation borrow.
5. **Metadata commit:** redb writer contains bounded compare/commit only. No RPC, file read/write or corpus-sized
   encoding occurs inside it.
6. **View install:** after commit, open/verify the matching read snapshot, then take the short manager lock to
   install it. Do not begin another publication until install succeeds or recovery refuses serving.
7. **Query window/output:** reserve capacity before retrieval. Prepare evidence outside the window lock. Acquire an
   exclusive window/output commit only for bounded final insertion/advance after complete output.
8. **Cleanup:** last guard may wake cleanup but never deletes inline. Whole-generation deletion runs through the
   publisher/cleanup owner with no query/view-manager lock held.

No global lock nests another global lock. If an old API requires a different order, remove or adapt that API; do
not preserve the cycle.

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

A slice is complete only when:

- its real entrypoint reaches its real consumer;
- state/effect ownership is direct and factual;
- no optional/private issue is an undeclared prerequisite;
- the obsolete route and exclusive fixtures are removed or named as the next bounded deletion;
- unsupported states fail without writes, fallback or silent widening;
- all acquired locks follow the order above;
- the smallest relevant Rust 1.98 compiler/Clippy closure ran;
- the registry records exact SHA, paths, commands/results and unrun behavior;
- conflicting issue bodies and indexes are synchronized in the same published slice.

Broad native crash, recovery, relevance and scale evidence follows the connected implementation. It does not
block writing the first product path.
