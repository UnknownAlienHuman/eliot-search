# Implementation sequence

[Architecture](../architecture/README.md) and [local scope](../architecture/LOCAL_SCOPE.md) define the product.
This page owns delivery order and cross-owner values. Use [code map](CODE_MAP.md) to find existing files and
[working guide](WORKING_GUIDE.md) for checks/updates. The [registry](../audit/ANALYSIS_REGISTRY.md#resume) owns current progress.

## Connected path

```text
selected source → stable acquired bytes → immutable ObjectRef/current SourceRecord
→ prepared representation or explicit gap → Qdrant generation + matching published evidence
→ scoped Search / retained-source Exact → verified excerpt → CLI
```

One OS account, one machine, plain storage. No encrypted profile, key handoff, pairing/grants or multi-user system.
Preserve user data and unpublished #598 work without depending on it. A boundary is a responsibility, not a required crate.

<a id="minimal-cross-owner-values"></a>
## Cross-owner values

| Value | Meaning |
|---|---|
| DataRootGuard | Open root metadata/path and one retained process-lock handle |
| ObjectRef | Format version, kind, content address and exact length; no crypto profile |
| SourceRecord | Source identity, non-reset row/registration guard, retained reference and selection facts |
| PreparedRef | Exact input ObjectRef, transform identity and artifact/outcome |
| PhysicalTextIndexProfile | Actual Qdrant schema/resource settings and retrieval-text composition |
| QueryScoringProfile | Request-time BM25 settings; not a reason to rebuild |
| PublishedView | Exact physical collection/profile and matching owned redb read snapshot |
| EvidenceRef | Point/source/prepared binding, representation and range |
| ExecutionContext | Original absolute deadline, cancellation and operation-specific finite counters |
| RequestContext | Execution state plus pinned view/selection; Search adds scoring |

These are meanings, not an instruction to create a public type or crate for every row. Root/object/preparation/
publication use execution state only. Lower I/O cannot reset budgets. Exact needs no scoring. Continue/Expand use
the window's original view/results, not the latest index. Stage 2/3A has no intra-generation sequence/validity model.

<a id="owners"></a>
## Owners

| Responsibility | Owning recipe |
|---|---|
| Invocation/settings | [Config](../../crates/search-config/FUNCTIONS.md) |
| Root/lock/shared file publication | [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Immutable bytes | [Objects](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Acquisition/current source/inventory | [Source family](../../crates/search-source/README.md) |
| Transformation/unit ranges | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Text meaning/index profile | [Lexical](../../crates/search-lexical/FUNCTIONS.md) |
| Qdrant SDK/readback | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication/recovery/generation cleanup | [Publisher](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Typed metadata/read snapshots | [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Selection/evidence/query | [Query family](../../crates/search-query/README.md) |
| Framing/client/presentation | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Composition | [Daemon](../../bins/eliot-searchd/FUNCTIONS.md) |
| Offline maintenance/import | [Maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

<a id="delivery-sequence"></a>
## Delivery sequence

<a id="1-foundation--invocation-settings-and-root"></a>
### 1. Foundation — invocation, settings and root

Owners #333/#266/#360. Implement actual help/version → args_os/settings once → inspect/init/open → root guard →
factual status. Only data root, bounded config input and diagnostics are needed. No keys or storage-mode selector.
Use the root owner's standard lock and same-parent no-clobber helper, not a custom Windows security platform.
Initialize manifest-last; known empty incomplete state may be explicitly discarded/retried. Unknown/user data stays.
Use the [actual dispatch map](CODE_MAP.md#existing-dispatch-to-replace). Work in this order:

| Connected change | Stop condition before the next change |
|---|---|
| Parse invocation once | Help/version and invalid input finish before spawning or opening stores; selected settings reach the actual handler. |
| Route inspect/init/open to the root owner | One guard owns the real root lifetime; plain initialization and refusal paths use the existing root recipe, not key selection. |
| Connect factual status and remove bypasses | Real binary dispatch cannot enter a displaced route for converted commands; status reports observed root/store state. |

These can land as small connected cuts with minimal actual-consumer check/Clippy. No Qdrant server, complete
settings catalogue, transport rewrite, crypto removal programme or future test framework blocks Foundation.
When a shared legacy route cannot yet leave, name its remaining caller and next removal; do not create a facade.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. One file to one CLI result

Coordinator #264. One explicit local file → stable read → plain ObjectRef/current row → materialize/unitize →
fresh vectorless Qdrant collection → explicit scoring text index → InsertOnly/readback/canary → matching current
and evidence commit → PublishedView → TextQuery → retained excerpt → CLI.

Upgrade exact Qdrant client/server/schema and their real consumers together; the main lock still has client 1.19.0,
while the selected scoring-text path needs the documented 1.20.x APIs. Pin the exact adopted patch at the code cut.
Use lexical/bridge owner settings, not copied tables. Keep final owners' persistent references, not a throwaway demo.
Connect to the operator-started local server; no supervisor construction or key lease. A fresh root without a
published generation is buildable, not a corrupt root or a successful empty search; the bridge owns those cases.
The one-shot CLI uses the actual owned-child framed path for one request. Live paging/windows arrive in Stage 4;
do not create a temporary line protocol or require full interactive behavior to deliver the first result.
No broad inventory, watcher, paging, encryption, permission system, point mutation or cleanup platform first.
Complete when default binaries return real retained evidence through Qdrant; missing/corrupt evidence is not a card.

<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence

Owners #270/#271/#331/#261/#300/#303. Add restart, inventory, stale-work/ABA guards, lost-wakeup repair and complete
desired-versus-published comparison. Fixed input → fresh complete collection → matching metadata commit → view install.
Visible collections are immutable. Keep old views until safe whole-generation cleanup. Add only real scope indexes.
Candidate narrowing and IDF population remain distinct. Declare numeric corpus/resource/latency budgets before
measurement under #215; include retained collections, read-snapshot pages and backlog. Ship only measured classes.

<a id="3b-measured-incremental-publication"></a>
### 3B. Source-local incrementality — only when 3A budgets fail

Owners #259/#329/#261/#263. Then add bounded complete-source deltas, conservative occurrence reuse, fixed births,
InsertOnly additions and non-creating closures. Preserve old views and verify scoring convergence. No physical
point deletion: compact by replacing the generation. Oversize work gets a fresh generation or explicit gap.
If every declared class passes 3A, leave 3B absent; do not weaken corpus requirements merely to avoid optimization.

### 4. Exact and live session

Owners #249/#275/#280/#332. Literal Exact scans pinned retained inventory. Extend the same serviced framed
connection with bounded QueryWindows; do not replace the one-shot transport. Continue pages stored order;
Expand uses an emitted item's neighborhood. Complete terminal output precedes the prepared local cursor update;
partial transport output commits nothing and closes the session.

### 5. Maintenance and selected product depth

Owners #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336. Add stopped-root collection/archive/import,
then selected source-backed orientation/comparison/Git/document capabilities. Optional donors join existing
consumers. No encryption or restricted-user phase. Qdrant Edge, if studied later, is a replacement experiment.

<a id="lifetime-and-lock-order"></a>
<a id="publication-and-retrieval"></a>
## Lifetime and lock order

Root guard precedes and outlives stores. Publication serializes selection, performs object/backend I/O outside
redb writers, commits bounded metadata, opens/verifies its snapshot, then installs under a short view lock.
Do not start the next publication before install/recovery completes. No final transaction copies the corpus.
New Search/Exact clones current under the short lock and releases it before work. Continue/Expand keep their
existing view. No writer/view/window mutex or mutable map borrow spans I/O; owned guards may remain live.
Last guard drop wakes cleanup but performs no deletion. One session owner prepares then applies cursor changes.

## Slice completion

Real entrypoint, real consumer, concrete owner and removed displaced route; no silent fallback or fabricated
readiness. Run the minimal locked check/Clippy closure after code. Update one owning rule, affected links and
registry next step. A documentation change does not complete an implementation stage.
