# Implementation sequence

[Architecture](../architecture/README.md) and [local scope](../architecture/LOCAL_SCOPE.md) define the product.
This page owns delivery order and cross-owner values. Use [code map](CODE_MAP.md) to find existing files and
[working guide](WORKING_GUIDE.md) for checks/updates. The [registry](../audit/ANALYSIS_REGISTRY.md#resume) owns current progress.

## Connected path

The current binaries expose four root operations, explicit file/tree index, retain-file, retained-only
prepare-source, publish-prepared, foreground watch and framed read-only Search/Raw/DecodedUtf8 Exact. The connected path reaches source/current row, complete preparation/current outcome,
Qdrant generation/current evidence/view and verified retained units/citations. Publication returns generation
state/counts; Search and Raw/DecodedUtf8 Exact are read-only query operations. Their exact native scopes belong in the registry;
Same-session Continue/Expand/expiry and stopped-root collect-objects are connected too. The
[registry](../audit/ANALYSIS_REGISTRY.md#work-register) owns their exact qualification scopes and remaining work;
connected commands do not imply an installed baseline or completion of the product.

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

Q1 consumes saved units into a fresh vectorless Qdrant 1.20.0 collection, verifies exact schema/payload/count and an independent checked Unicode/code TextQuery canary, then commits current/evidence/predecessor cleanup and validates the matching publication snapshot. Empty/Gap sources remain in inventory. CONTROL pages contain fixed ObjectRefs/ranges/IDs and request-byte metadata, not text; each upload/readback pass retains one verified Prepared object/decoded view across adjacent pages of a source. Publication returns state/counts only. Use the operator-started local server and [observed Windows storage guidance](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage). A fresh unpublished root is buildable, not a successful empty search.

U1 search pins committed metadata without a corpus scan, calls the concrete backend and resolves only returned evidence/CapturedSource in the same snapshot. It verifies actual Source/Prepared length/BLAKE3, captured decoder binding and all unit byte/line/boundary facts before emitting the full unit and citation. It never substitutes newer preparation or live paths. Query/Cancel and one terminal use the serviced owned child with an 8 MiB total frame cap and whole-card OutputLimit. Search writes neither CONTROL nor collections.

Retention, preparation and publication retain their root-owned argv child. Search uses the connected framed child with one joinable synchronous backend worker; no temporary route or session framework is required. The connected session window is described in Stage 4 and explicit/bare Index plus watch in Stage 3A; encryption/permission platforms are outside local scope. Missing returned retained objects produce explicit gaps; corrupt evidence produces no card. U1 does not complete the product.
<a id="3a-fresh-generation-convergence"></a>
### 3A. Fresh-generation convergence

R1 connects index [PATH] to existing source/preparation/publication helpers under one root guard and original
deadline. PATH admits/refreshes only that File/Tree scope; bare index reloads all stored admissions and scans
their bounded deduplicated union. Explicit File wins over Tree ignores. Unchanged key/ObjectRef is a no-op.
Admission/kind survive absence/re-add. Only confirmed in-scope absence removes current membership; ignored but
present sources remain, as do retained bytes/pinned evidence. Incomplete inventory is not removal.

R1 is accepted at its recorded functional and ordinary-repo-1 optimized RELEASE fixed-input scopes.
Durable admission/reload and foreground watch are implemented. Watch accepts no PATH,
runs one saved-selection Index child with its original 120-second budget, drains bounded retained output and
cleans up before cancellable sleep (default 30 seconds, positive --interval-ms override). Iterations do not
overlap. Owner-stdio EOF requests cooperative cancellation on parent loss. Index uses open_plain_with_recovery;
ordinary read-only opens/queries do not repair.

Persisted unknown Insert/ABA, postcommit/pre-view and held-query exclusion/cleanup have their separately pinned
native scopes; the resource run does not relabel them. Fixed-input RELEASE measurement passed the declared
profile with sampled/OS-reported counters; complete unsampled product peaks and generation-replacement peak
remain UNKNOWN/NOT_RUN. DEBUG query performance is not qualified. See the registry for exact boundaries.
X1 is DONE in its selected Windows/Release scope; M1 collector is connected/PARTIAL; archive/import are not delivered.
Archive/import and V1 relevance/scale/installed-release work remain unfinished.
Search stays read-only; query and writer share exclusive root exclusion through flush, so a concurrent view
manager is not the next prerequisite.

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

Owners #249/#275/#280/#332. FIRST RAW scans the selected committed PublishedView Source inventory, including
Empty/PreparationGap inputs, with literal UTF-8 bytes over undecoded retained objects and no Qdrant/live fallback.
Source integrity precedes raw range/hex evidence. Missing input is a Gap, corruption fails, presentation limits
do not end scanning and complete negative requires fully verified zero matches. Those one-shot proofs alone do not establish session qualification. DecodedUtf8 now verifies retained Source and SAVED Prepared UTF-8 against captured bindings,
proves byte-identity mapping and emits both refs/ranges plus text. It scans full representation across units;
no on-the-fly materialization, reprepare, normalization or live fallback occurs. PrepGap/missing representations
are explicit gaps; corruption fails. Extracted still has no real producer or implemented route.
The CLI session -> OwnedQuerySession -> daemon --session-stdio opens one Search/Raw/DecodedUtf8
window and retains at most the original 64-entry final-order prefix; Exact total matches remain separate.
Open/Continue pages are 1..64 (default 16). TTL starts at accepted Open including work, defaults/caps at
120000 ms and never renews. Expand requires emitted locators and explicit before/after bytes, with 65536 total
bytes including the original span, original representation and valid UTF-8 boundaries. Complete daemon frame
flush commits cursor progress; expiry/EOF cancels, joins and releases through the owned client reap path.

Final Sol incremental X1 gate PASS now covers the selected Windows/Release public consumer on main 59061070
with source-equivalent 473447af binaries: Raw/Decoded/ranked windows, original refs/bytes, TTL/no-renewal/EOF,
held deferred Cancel/finite partial expiry, selection/gaps, Busy release and bounded paging. The real 63+1
frame cut committed cursor 63 then 64; all 64 texts/ranges were byte-exact and every frame fit the 8 MiB cap.
Query CONTROL/objects were preserved and session wait/both EOF/OS absence observed.

Exact pre-exit release timing remains UNKNOWN, SDK-only unread NOT_PROVEN is not a separate selected gate,
and old Gridstore cause/setup observers retain their recorded UNKNOWN scope. No new extracted/parser/
nonidentity producer is a selected gate before M1; saved UTF-8 identity is the current producer.
Whole-project V1/installed final-head qualification remains NOT_RUN.
See the [window owner](../../crates/search-query/search-continuation/FUNCTIONS.md).

### 5. Maintenance and selected product depth

M1 is PARTIAL: collect-objects is connected at f01809bc in its recorded Windows/Release scope. The owned child
takes open_plain_with_recovery/root exclusion, one consistent CONTROL snapshot and typed disk-backed marks;
it verifies Prepared bindings/embedded Source closure and every Source before one final-object streaming sweep.
Only proven canonical current-format unmarked leaves may be deleted; foreign/unknown entries are preserved.
Every-32 scratch durability checkpoints bound engine bookkeeping, not object count; this is source proof,
not universal native RAM measurement. Current root.importing is unsupported/refused, not an import flow.
See the [collector rule/checks](../../crates/search-runtime/search-retention/FUNCTIONS.md#implemented-checks-and-remaining-m1).

Archive/import remain unimplemented and are next. Owners #302/#244/#281/#227/#335; optional #219/#239/#248/#334/#336 implement those consumers first,
then selected source-backed orientation/comparison/Git/document capabilities. Optional donors join existing
consumers. No encryption or restricted-user phase. Qdrant Edge, if studied later, is a replacement experiment.

<a id="lifetime-and-lock-order"></a>
<a id="publication-and-retrieval"></a>
## Lifetime and lock order

PlainRootGuard covers current operations and the published query snapshot through complete output flush.
Publication borrows short durable transactions from one ControlWriter/Database across Index/staging/upload.
No transaction spans object/backend I/O. Explicit close precedes final CAS and native read-only acceptance;
publication commits bounded metadata and completely validates
its matching snapshot before reporting visibility. No final transaction copies the corpus. Ordinary search
pins that committed snapshot and verifies only returned evidence/objects; it does not run acceptance again.

The current root-owned child serializes these operations. Later concurrent sessions may clone their view under
a short lock; Continue/Expand retain that original view/order. Do not introduce a view manager or lock before
an actual concurrent consumer needs one. No writer/view/window mutex or mutable shared borrow spans I/O.
Retired-generation cleanup must preserve every live view/window and unresolved target obligation; last guard
drop performs no deletion. Window cursor changes follow complete terminal output.

## Slice completion

Real entrypoint, real consumer, concrete owner and removed displaced route; no silent fallback or fabricated
readiness. Run the minimal locked check/Clippy closure after code. Update one owning rule, affected links and
registry next step. A documentation change does not complete an implementation stage.
