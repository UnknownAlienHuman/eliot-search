# Implementation sequence

[Architecture](../architecture/README.md) and [local scope](../architecture/LOCAL_SCOPE.md) define the product.
This page owns delivery order and cross-owner values. Use [code map](CODE_MAP.md) to find existing files and
[working guide](WORKING_GUIDE.md) for checks/updates. The [registry](../audit/ANALYSIS_REGISTRY.md#resume) owns current progress.

## Connected path

The current binaries expose four root operations, retain-file and retained-only prepare-source. Stage 2 connects
the source/current row and complete stored preparation/current outcome. Qdrant publication, framed queries and
later stages remain future work; exact native scopes belong in the registry.

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
| PlainRootGuard | Current root-owner type retaining the root path and process-lock handle |
| ObjectRef | Format version, kind, content address and exact length; no crypto profile |
| SourceRecord | Source identity, non-reset row/registration guard, retained reference and selection facts |
| PreparedRef | Exact input ObjectRef, transform identity and artifact/outcome |
| PhysicalTextIndexProfile | Actual Qdrant schema/resource settings and retrieval-text composition |
| QueryScoringProfile | Request-time BM25 settings; not a reason to rebuild |
| PublishedView | Exact physical collection/profile and matching owned redb read snapshot |
| EvidenceRef | Point/source/prepared binding, representation and range |
| Execution context | Original absolute deadline, cancellation and finite work; root APIs take these directly |
| RequestContext | Execution state plus pinned view/selection; Search adds scoring |

These are meanings, not an instruction to create a public type or crate for every row. Root/object/preparation/
publication use execution state only. Lower I/O cannot reset budgets. Exact needs no scoring. Continue/Expand use
the window's original view/results, not the latest index. Stage 2/3A has no intra-generation sequence/validity model.

<a id="owners"></a>
## Owners

| Responsibility | Owning recipe |
|---|---|
| Invocation/settings | [Daemon bootstrap](../../bins/eliot-searchd/src/app/bootstrap.rs), [settings rules](../../crates/search-config/FUNCTIONS.md) |
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

Owners #333/#266/#360. Current source connects help/version and native arguments to typed root input, then
inspect, initialize, check-existing-owner or explicit incomplete-init discard. The CLI uses bare operation
subcommands; the daemon uses matching --operation flags. See the
[actual dispatch map](CODE_MAP.md#existing-dispatch-to-replace) and exact syntax in the
[CLI](../../bins/eliot-search/FUNCTIONS.md#capture-and-dispatch) and
[daemon](../../bins/eliot-searchd/FUNCTIONS.md) recipes.

Only data_root is consumed from bounded file/environment/CLI settings. Diagnostics use fixed output bounds;
there is no accepted diagnostic_limit setting, storage-mode selector or external initialization ID.
The root owner implements manifest-last initialization, existing-root opening and explicit incomplete-init
discard using the same control format that search-control-redb owns. Init/open hold PlainRootGuard through
the terminal output. No source body, Qdrant backend or query handler is constructed by these commands.

| Boundary | Required behavior and final check |
|---|---|
| Invocation and settings | Help/version and malformed command shape finish before root effects; lower settings layers validate before override. |
| Root lifetime and control format | All four operations use the root owner and the caller's original deadline/cancellation; existing/user data is preserved. |
| Factual output and child cleanup | Output names the requested root operation and observed state; execute final child-exit, pipe and failure cases. |

The displaced binary routes and exclusive legacy fixtures are physically removed. Do not restore them as
compatibility scaffolding. Final root/control integrity and child-pipe fixes still require locked checks and
native qualification at the final source revision. Earlier binary runs and helper-native probes do not
qualify later changes, universal root safety or an installed baseline. Record final evidence in the existing
registry; this delivery sequence is not a second status ledger.

<a id="2-walking-skeleton--one-file-to-one-cli-result"></a>
### 2. One file to one CLI result

Coordinator #264. S1 connects one explicit local file → one stable native-handle read → plain ObjectRef publication/readback → guarded CurrentSourceRecord in control/control.redb → factual CLI result.

P1 connects prepare-source --source-id UUID [--unit N] through the existing owned child. It reads verified retained input without reopening the live path, creates exact UTF-8 text and bounded ranges, stores the complete artifact through the shared writer, decodes its stored bytes, then installs a guarded current outcome and returns one complete stored unit. Empty and Gap are explicit. CONTROL preserves selected settings and compares source/registration/input/settings and expected outcome with non-reset revisions. [Owner rules](../../crates/search-control-redb/FUNCTIONS.md#current-preparation) and the [registry](../audit/ANALYSIS_REGISTRY.md#evidence-status) distinguish implementation from exact native/gate scopes.

Next Q1 consumes actual stored text/units into a fresh vectorless Qdrant collection, the scoring-text index, InsertOnly/readback/canary, matching current/evidence commit and PublishedView. U1 then connects TextQuery and a retained excerpt. Upgrade exact Qdrant client/server/schema and real consumers together at that cut. Use lexical/bridge owner settings and the operator-started local server. A fresh root without a published generation is buildable, not a corrupt root or a successful empty search.

Both source operations use the existing root-owned one-shot child, with no temporary protocol or automatic second preparation route inside retain-file. Live paging/windows arrive in Stage 4. Broad inventory, watchers, point mutation and cleanup remain later scope; encryption/permission platforms are outside local scope. Missing or corrupt retained evidence is not a card. S1/P1 do not complete Stage 2 or the product.
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

PlainRootGuard currently covers a one-shot root operation through output. The following publication/view/session
ordering applies when those future owners are connected.

Root guard precedes and outlives stores. Publication serializes selection, performs object/backend I/O outside
redb writers, commits bounded metadata, opens/verifies its snapshot, then installs under a short view lock.
Do not start the next publication before install/recovery completes. No final transaction copies the corpus.
New Search/Exact clones current under the short lock and releases it before work. Continue/Expand keep their
existing view. No writer/view/window mutex or mutable map borrow spans I/O; owned guards may remain live.
For future published views, last view-guard drop wakes cleanup but performs no deletion. One session owner
prepares then applies cursor changes.

## Slice completion

Real entrypoint, real consumer, concrete owner and removed displaced route; no silent fallback or fabricated
readiness. Run the minimal locked check/Clippy closure after code. Update one owning rule, affected links and
registry next step. A documentation change does not complete an implementation stage.
