# ELIOT Search — implementation spine

Read [architecture](../architecture/README.md) first. This document selects the shortest safe route to a working
application. Nearest `FUNCTIONS.md` files own local seams; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) owns evidence/status.

The product is pre-release. Internal APIs/formats need no compatibility facade. Preserve user files and sole
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

These are information contracts, not a universal-contract-crate mandate:

```text
DataRootGuard  = retained root handle + one exclusive lock + current root metadata
ObjectRef      = format/profile/kind + content address + exact length
SourceRecord   = SourceId + non-reset row revision + current ObjectRef + selection facts
PreparedRef    = exact input ObjectRef + transform identity + prepared ObjectRef/outcome
PublishedView  = physical generation + retrieval profile/sequence + real redb read snapshot
EvidenceRef    = point/source/prepared identity + representation/range
RequestContext = one absolute deadline + cancellation + finite counters
```

Vendor, OS and redb types stay private to their adapters where practical.

## Rules which prevent implementation deadlock

1. **Configuration is incremental.** #333 is an umbrella. Foundation ships only invocation, root and plain-storage
   fields. Source, Qdrant, query and optional-profile fields arrive with the consuming owner.
2. **Plain storage is baseline.** #598 may add encryption later through the same `ObjectRef`; it cannot block the
   plain path.
3. **Issues are ownership trackers.** Closure order is not implementation dependency order.
4. **No throwaway product path.** One-file/one-shot/full-generation limits use final owners and persisted values.
5. **No speculative framework.** Add a trait/package/registry/worker/donor only for a concrete consumer; a trait
   normally requires multiple real implementations.
6. **One manager owns overlap.** Parallel work is disjoint; the manager owns interfaces, integration and deletion
   of displaced routes.
7. **Full generation before incremental mutation.** Restart-convergent complete rebuild precedes stable point
   reuse, fixed closures and compaction heuristics.
8. **No broad gate before behavior.** Minimal compiler/Clippy follows each production cut; broad runtime/scale
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

#### Selected initialization shape

Foundation creates no user source state. Use a manifest-file-last empty-layout operation:

```text
validate empty/recognized target
→ acquire initialization exclusion
→ create exact incomplete-initialization marker
→ create/flush/close/verify minimal redb + object layout
→ create/sync final root-manifest temp in the root
→ publish the final manifest no-clobber last
→ reopen/verify current layout
→ remove marker/stage
```

Before the final manifest, classification is `IncompleteInitialization`; ordinary open performs zero repair.
Baseline recovery is explicit `discard-incomplete`, not replay/resume. It may remove only the exact recognized
empty initialization layout under exclusion. Unknown files, conflicting bytes or replacement objects refuse and
require manual relocation. There is no initialization journal, operation transcript or phase machine.

Prefer `tempfile::NamedTempFile` plus `persist_noclobber` for the final manifest if the selected Windows path
semantics remain correct. Otherwise use one small root-owner native finalizer; do not create a generic filesystem
layer.

**Completion:** real default entrypoint reaches root owner; unsupported/foreign roots fail before writes; one
actual guard remains live while stores exist; status reports opened owners; old config activation/root lifecycle
callers are removed or named as the immediate bounded deletion.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. Walking skeleton — one file to one CLI result

Coordinator: #264. Owners include #267/#270/#330/#331/#259/#263/#261/#272/#293/#280/#332.

```text
explicitly select one supported local file
→ one stable read
→ plain put_object
→ commit one current source head
→ materialize/unitize once
→ persist one preparation result
→ build one fresh never-reused Qdrant generation
→ commit matching evidence/current state
→ explicitly install one PublishedView
→ one one-shot Search
→ resolve one pinned retained excerpt
→ render typed CLI output
```

Temporary limits: owning local account, plain storage, one explicit file, one complete fresh generation, one
one-shot request, no watcher, broad inventory, encrypted profile, paging, aliases or physical cleanup.

The cut persists exact root/object/source/prepared/view references and can reopen the object through the current
root. It addresses the physical collection named by `PublishedView` directly; a Qdrant alias is not visibility
or recovery authority.

**Completion:** default binaries execute the path against real Qdrant and return the retained excerpt. Missing or
corrupt evidence cannot produce a card. Help/status advertise only constructed handlers. No legacy interceptor,
oracle, proof floor or duplicate store serves as an intermediate.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence — restart and correctness before point optimization

Issues: #270/#271/#331/#261/#300/#303 and affected owners.

Add:

- process restart/reopen of root, objects, current source/prepared state and current `PublishedView`;
- startup, explicit and periodic authoritative inventory without a watcher;
- ABA-safe source/preparation installation and lost-wakeup rediscovery from current state;
- desired selected inventory versus published inventory comparison;
- complete rebuild into a fresh never-reused physical Qdrant generation whenever desired state differs;
- complete matching evidence snapshot/current commit, explicit view install and old-generation retirement;
- whole-generation cleanup after every guard is gone.

The fresh-generation retrieval profile has no intra-generation source update. Build with explicit `insert_only`,
settle/read back the complete build, then make it visible. Once visible, the collection is immutable. A source
change builds the next generation. Cross-generation point IDs need not be stable.

This is production architecture, not a disposable implementation. It exercises retained evidence, recovery,
scope, old-view lifetime and cleanup without requiring stable occurrence matching, fixed point closure or
compaction thresholds.

**Completion:** after restart and a real source edit/removal, current state converges to a new complete generation;
an old pinned view remains exact; a dropped wake-up is repaired; incomplete inventory never removes unseen data;
retired generation cleanup is bounded and truthful.

<a id="3b-measured-incremental-publication"></a>
### 3B. Measured incremental publication — optimize only after 3A evidence

Issues: #329/#259/#261/#263 plus preparation/unitizer/evidence owners.

First measure complete-rebuild time, bytes, Qdrant writes and retained generations on declared corpora. Add
incrementality only when those measurements justify it.

Incremental profile rules:

- unambiguous occurrence reuse supplies stable point identity inside one physical generation;
- new immutable points explicitly use `insert_only`; default upsert is forbidden;
- retries carry identical fixed ID/input/payload and exact readback;
- replacement writes one fixed non-creating exclusive `valid_until = target_sequence` plus exact readback;
- older sequences remain valid below the boundary, so closure does not wait for old readers;
- individual point IDs are never physically deleted;
- profile/schema change or measured closed-point growth builds a fresh never-reused generation;
- no new pending operation targets a retired generation.

A transition from fresh-generation to incremental retrieval is an explicit profile/generation rebuild, never a
reinterpretation of old points.

**Completion:** representative edits show materially lower work than 3A without weakening old-view evidence,
recovery, scope or cleanup truth. If the gain is small, keep fresh-generation publication and do not ship the
incremental machinery.

### 4. Exact and live session

Issues: #249/#275/#280/#332 and direct query/evidence owners.

Add literal Exact over the pinned retained inventory, then one serviced framed child session with
Continue/Expand. One process-local `QueryWindow` owns final ordered evidence and a `PublishedView` guard.
One-shot output exposes no reusable locator. Partial terminal output advances no cursor.

### 5. Maintenance and product depth

Issues: #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336.

Add stopped-root object collection and archive/import, then selected source-backed orientation, comparison,
document and Git behavior. Optional donors follow a measured consumer and cannot block baseline Search.

<a id="lifetime-and-lock-order"></a>
## Lifetime and lock order

Preserve this partial order; no reverse acquisition path:

1. `DataRootGuard` is acquired before stores and dropped after stores close. It is not per-request locking.
2. One publisher task/permit selects work, but holds no redb writer or view-manager mutex during object/Qdrant I/O.
3. Query clones current `PublishedView` under a short manager lock and releases it before scope/backend/object work.
4. Object/Qdrant/process/model I/O occurs without a redb write transaction and without a QueryWindow mutation borrow.
5. Redb writer performs bounded compare/commit only; no RPC, filesystem read/write or corpus-sized encode.
6. After commit, open/verify matching read snapshot, then take the short manager lock to install the view. Do not
   begin another publication until installation succeeds or recovery refuses serving.
7. Reserve window/output capacity before retrieval. Prepare evidence outside the window lock; mutate cursor/window
   only for bounded final insertion/advance after complete output.
8. Last guard drop may wake cleanup but never deletes inline. Generation deletion runs with no query/view-manager
   lock held.

No global lock nests another global lock. If an old API requires another order, remove/adapt it rather than
preserving the cycle. qntx/qmd's recent per-statement lock correction is corroborating donor evidence, not a new
dependency.

## Backend and alias boundary

Qdrant remains the only selected indexed backend. Do not add a backend trait, Tantivy fallback or dual writer.
Tantivy/Zoekt are implementation donors for commit/visibility/immutable generations only.

After #264/3A, reconsider Qdrant only if measured setup, recovery, scoped-IDF, relevance or scale criteria fail.
A prototype must consume existing retained/prepared/evidence contracts and replace the derived backend cleanly.

Qdrant aliases are not serving authority because alias mutation cannot atomically install ELIOT's redb snapshot
and old pinned views require physical generations. `PublishedView` always names the exact collection.

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
- the obsolete route and exclusive fixtures are removed or named as the immediate next deletion;
- unsupported/incomplete states fail without writes, fallback or silent widening;
- locks follow the order above;
- the smallest relevant Rust 1.98 compiler/Clippy closure ran;
- the registry records exact SHA, paths, commands/results and unrun behavior;
- conflicting issues/indexes are synchronized in the same published slice.

Broad native crash, recovery, relevance and scale evidence follows the connected implementation; it does not
block writing the first product path.