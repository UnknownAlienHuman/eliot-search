# Implementation sequence

[Architecture](../architecture/README.md) defines the product. This page owns delivery order and cross-owner
boundaries. Read only the current stage and affected owner recipes. The
[registry](../audit/ANALYSIS_REGISTRY.md#resume) owns branch/status/evidence.

The product is pre-release: internal APIs/formats need no compatibility facade. Preserve original user files and
sole retained copies; remove obsolete machinery with its actual callers.

## Connected path

```text
selected source → stable acquired bytes → immutable ObjectRef → current SourceRecord
→ PreparedRef or explicit gap → Qdrant generation + matching published evidence
→ scoped Search or retained-source Exact → verified excerpt → CLI/local client
```

Every step has a concrete owner, not necessarily a separate crate. One manager integrates one connected change
in one worktree. Configuration arrives with consumers; optional #598 encryption does not block plain storage.

<a id="minimal-cross-owner-values"></a>
## Cross-owner values

```text
DataRootGuard       = retained handles + exclusive process lock + root metadata
ObjectRef           = format/profile/kind + content address + exact length
SourceRecord        = SourceId + non-reset row revision + ObjectRef + selection facts
PreparedRef         = input ObjectRef + transform identity + prepared artifact/outcome
PhysicalTextIndexProfile = physical schema/resource settings + retrieval-text composition
QueryScoringProfile = request-time BM25 settings; separate from physical schema
PublishedView       = exact physical collection + physical profile + owned redb read snapshot
EvidenceRef         = point/source/prepared binding + representation/range
ExecutionContext    = absolute deadline + cancellation + counters needed by this operation
RequestContext      = query execution context + pinned view + resolved scope; Search adds scoring profile
```

`ExecutionContext` is a small private value or borrowed parameters, not a generic context service. Root creation,
object writes, preparation and publication need no query view, scope or scoring profile. A query passes its same
execution budget to lower-level I/O; nested calls cannot renew the deadline or reset counters. Never fabricate a
view or add optional query fields to every background operation. These names describe responsibilities, not a
requirement to create one public type per row. Exact needs no BM25 settings; Continue retains its original
ordered results instead of requerying under a new scoring profile.

Stage 2/3A has no intra-generation sequence. Changing query-time k/b does not rebuild a collection; changing
physical schema/retrieval meaning does. The [lexical owner](../../crates/search-lexical/FUNCTIONS.md) defines both
profiles; the [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) maps them to Qdrant.

<a id="owners"></a>
## Owners

| Responsibility | Read when affected |
|---|---|
| Invocation/config | [Typed settings](../../crates/search-config/FUNCTIONS.md) |
| Root, initialization, shared platform finalizer | [Root owner](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Immutable source/prepared objects | [Object store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Stable acquisition/current source/inventory | [Source family](../../crates/search-source/README.md) |
| Transformation/unit ranges | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Lexical profile | [Lexical owner](../../crates/search-lexical/FUNCTIONS.md) |
| Qdrant SDK and readback | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/recovery/whole-generation cleanup | [Publisher](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Durable metadata/snapshots | [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Query/evidence/scope | [Query family](../../crates/search-query/README.md) |
| Framing / serviced connection / CLI | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Composition | [Daemon](../../bins/eliot-searchd/FUNCTIONS.md) |
| Stopped-root maintenance | [Maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

<a id="delivery-sequence"></a>
## Delivery sequence

<a id="1-foundation--invocation-settings-and-root"></a>
### 1. Foundation — invocation, settings and root

Owners: #333/#266/#360.

```text
help/version without effects → capture args_os/settings once
→ bounded root/plain settings → inspect | initialize | open → DataRootGuard → factual status
```

Implement only config locator/bounds, data root and plain profile. Initialization publishes the final root manifest
last and no-clobber; before that, only a recognized empty incomplete layout may be explicitly discarded and retried.
No resume/replay journal, automatic repair or migration. Unknown entries and unsupported roots remain untouched.

The root recipe owns the [Windows finalization candidate](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#windows-finalization)
and Unix operation. Implement the smallest platform canary there; source inspection is not platform support.

Complete when the real entrypoint reaches the root owner, one guard surrounds store lifetime, status is factual,
unsupported roots fail before writes, and the displaced config/root routes are removed or assigned the next bounded
removal. Do not build a configuration or filesystem framework first.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. Walking skeleton — one file to one CLI result

Coordinator #264; use the affected owners above, not a prerequisite graph of every issue.

```text
one explicit supported file → stable read → plain ObjectRef/current SourceRecord
→ materialize/unitize → bounded retrieval_text → fresh vectorless Qdrant collection
→ explicit scoring text index → InsertOnly build + exact readback + TextQuery canary
→ matching evidence/current commit → PublishedView → TextQuery → retained excerpt → CLI
```

Upgrade the locked Qdrant client 1.19, server, schema and real consumers together to one exact 1.20.x patch.
Use the lexical owner's physical/query profiles; do not copy their defaults here. The first smoke is one local
account, plain root, file, physical collection and one-shot request. Persist the final owners' exact references
so restart can extend this path rather than replace a throwaway demo.

No watcher, broad inventory, encryption, paging, filter indexes, stable cross-generation occurrence identity,
point mutation, cleanup platform or second backend is a prerequisite.

Complete when default binaries return the correct retained excerpt through real Qdrant; missing/corrupt evidence
cannot produce a card. A legacy oracle, proof harness or local BM25 implementation is not an intermediate product.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence

Owners: #270/#271/#331/#261/#300/#303 and affected consumers.

Add restart, authoritative inventory, ABA-safe source/preparation installation, lost-wakeup repair and complete
desired-versus-published comparison. Capture fixed generation input, build a fresh complete physical collection,
commit matching evidence/current, install its view, and retain old views until whole-generation cleanup is safe.
Visible collections are immutable; no validity interval or cross-generation point continuity is required.

Add only filter indexes used by real scope clauses and choose explicit measured index residency. Scope resolution
keeps candidate filtering distinct from IDF population; even whole-population queries retain request-only filters.

Before measuring, declare corpus classes and numeric build/edit latency, throughput, memory/CPU/handles,
current+building+reader-retained disk, redb retained-page and cleanup-backlog budgets under #215. Coalesce updates
without unbounded backlog. This correctness/reference profile ships only for classes meeting those budgets.

<a id="3b-measured-incremental-publication"></a>
### 3B. Source-local incrementality — only when 3A budgets fail

Owners: #259/#329/#261/#263 and actual unitizer/evidence consumers.

Only then add a bounded source-local delta, conservative occurrence reuse, operation-fixed births, InsertOnly
additions and fixed non-creating closures. The publisher owns atomic source metadata and old-view semantics.
Individual points are not physically deleted; compaction replaces the whole generation. Qualify scoring/optimizer
convergence as well as mutation readback. An oversized delta uses a fresh generation or explicit gap.

If every target class meets Stage 3A budgets, leave Stage 3B absent and close its issues with the measurements.
Do not weaken a declared corpus requirement merely to avoid necessary optimization.

### 4. Exact and live session

Owners: #249/#275/#280/#332. Add literal Exact over the pinned inventory, then one serviced framed session.
A bounded QueryWindow retains final ordered references and the same view. Continue does not requery; Expand uses
an emitted item's bounded neighborhood. Partial output advances no cursor.

### 5. Maintenance and selected product depth

Owners: #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336. Add stopped-root collection/archive/import,
then selected source-backed orientation/comparison/document/Git behavior. Optional donors join an existing consumer.
Qdrant Edge, if evaluated, is an isolated replacement experiment, never a permanent fallback.

<a id="lifetime-and-lock-order"></a>
## Lifetime and lock order

DataRootGuard precedes stores and outlives them; it is not a per-request mutex. A publisher serializes selection,
performs object/Qdrant I/O outside a redb writer, commits bounded metadata, opens/verifies the matching snapshot,
then installs the view under a short lock. Do not begin another publication before installation/recovery completes.

New Search/Exact clones current under the short view lock, then releases it before scope/backend/evidence work.
Continue/Expand keep the existing window's view instead of acquiring current. Prepare output outside a window
mutation lock; commit cursor progress only after complete bounded output. No redb
writer, view-manager mutex or window mutation borrow spans filesystem, Qdrant, process or model I/O. Last guard
drop wakes cleanup and performs no deletion. Remove old APIs that require the reverse order.

## Slice completion

The entrypoint reaches the real consumer; state/effect ownership is concrete; displaced routes and exclusive
fixtures are removed or named for immediate bounded removal; failures do not widen scope or silently fall back.
Run the smallest locked compiler/Clippy closure after code. Record focused execution when behavior exists and
leave unrun behavior explicit. Update the owning rule and conflicting references, not another specification.
