# ELIOT Search — implementation

Read the [architecture manifesto](../architecture/README.md) first and keep it in the lead developer's
context. This document says what to write, where to write it, what to reuse and what to delete.

**Inspected target:** public `main` at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, 2026-10-18/19.
It is not a working indexed product. Historical waves, old issues, byte goldens and pre-release formats are
evidence only. Preserve #598's active private source-retention work and consume only its accepted public
handoff; do not inspect/reset/rebase/compete with its branch or worktrees.

## 1. Build one connected application

```text
ingest:
  selected source
  → stable final-handle read
  → one immutable ObjectRef
  → one current source head
  → one MaterializationProduct
  → structure-first units / retrieval inputs
  → bounded Qdrant publication/readback
  → CurrentPublishedView

query:
  SearchRequest
  → pin current PublishedView
  → resolve selected source scope
  → one RequestContext
  → real Qdrant native BM25
  → ViewManifestEntry
  → retained object/range verification
  → EvidenceCard[]
  → optional QueryWindow
```

The first finished slice retains one real local file across restart, indexes it, finds it from the CLI and
reopens the correct excerpt. A normal edit then updates only affected work. A lost Qdrant index rebuilds from
retained current-format state. DIRECT remains an explicit exact/source scan, never a silent fallback which
turns indexed failure into success.

A constructor, receipt, capability descriptor, package split or passing shape test is not delivery. Connect
the real producer and consumer and delete the replaced path in the same slice.

## 2. Owners and required cutovers

| Responsibility | Current source/problem | Required result |
|---|---|---|
| CLI | `eliot-search` has child stdio, a typed client and a legacy provider core; indexed query is translated back to tab/hex. | One parsed invocation, one owned child-stdio connection, one current typed client, bounded output. |
| Local protocol | `search-provider-protocol/client`, daemon provider composition and public runtime form parallel pairing/grant/manual-codec/recipe/line applications. | `u32-le length + bounded Serde JSON`, direct handler dispatch, optional progress, one terminal response. |
| Settings | Startup compares settings through receipt-gated activation and can reject permitted values. | Merge bounded file/env/CLI once, validate actual fields, construct owners. Static changes require restart. |
| Current control | `PersistentControlJournal` appends permanent operations, copies all records after each mutation and scans history on recovery. Registry commands restore v4 batches/original inputs. | Short typed redb transactions for current settings, roots, source heads, prepared refs, current view, one pending publication and finite cleanup rows. |
| Source inventory | Watcher/currentness/reconcile fragments do not produce one real update; root sync ends unavailable. | One bounded dirty-path/root-scan queue used by startup, watcher hints, explicit sync and recovery. |
| Source bytes | `acquire_source_v2` reads/hashes/classifies then drops the body. Revision/materializer/DIRECT stores duplicate publication logic. | Pass the same acquired bytes into one immutable object writer, then compare/set one source head. |
| Preparation | Markdown/JATS verification can reopen and rebuild complete materialization between adjacent stages. | One materialization product feeds structure-first unitization; persist/load once. |
| Point identity | Unit/S11/payload mix revision/absolute coordinates with retrieval identity, causing suffix-wide rewrites. | Stable retrieval point separated from version-specific evidence binding. |
| Publication | Three disconnected phase/receipt machines; no real Qdrant cursor or recoverable view. | One immutable operation artifact, one pending row, real Qdrant apply/readback, current-view commit and finite cleanup obligations. |
| Qdrant | Real SDK code exists but daemon does not call it; runtime gate/secret equality/process-local receipts surround it. | One daemon-owned client/runtime for publication and query; exact full/narrow operations and native BM25. |
| View lifetime | Raw route/epoch pins, owner IDs, renewals and sampled watermarks permit deletion races. | One `Mutex + Arc/Weak` current-view manager; no arbitrary old-view acquisition. |
| Query | Recipe/capability/plan/access/executor/validator/projector stacks are fixture/process-test driven. | Direct `Search/Exact/Continue/Expand` handlers with one scope/context and source-backed cards. |
| Paging | Shared and daemon handle/continuation stores duplicate state and rescan the source catalogue. | One process-local `QueryWindow` with final refs, cursor, view guard and bounded item locator. |
| GC/retention | Generic leases, graph sweep, purge, restore and reclaimer journals are disconnected. | Known typed roots, one bounded object-GC gate and publication-owned Qdrant cleanup. |

Packages are current locations, not permanent architecture. Merge/delete packages when the remaining code no
longer isolates a real dependency or effect.

## 3. Current data model

### Typed redb state

```text
AppliedSettings
RootRecord
SourceRecord { source_id, root_id, locator/view, current_revision, availability }
PreparedRef { revision, preparation_profile, artifact_ref }
CurrentPublishedView
PendingPublication?
RetiredCleanup[]
```

Do not store source text, vectors, whole point pages, completed operation receipts, query history or
caller-supplied proofs in redb.

### Immutable objects

```text
ObjectRef { content identity, length, storage profile }
RevisionObject
PreparedArtifact
PublicationOperation / ManifestPage / CleanupPage
```

One shared writer/reader owns final object names. Current source/control/view rows reference objects after
finalization/readback. Unsupported old roots fail before writes and remain untouched.

### Source, retrieval and evidence

```text
SourceOccurrence {
  source_id
  structural_anchor
  occurrence_incarnation
  duplicate_position
  current revision/representation/range
}

RetrievalInput {
  role: document | query | rerank
  retrieval_profile
  bounded contextual prefix
  exact input bytes/identity
}

RetrievalPoint {
  stable point identity + collision evidence
  occurrence incarnation
  retrieval input/profile/role
  stable filter/group coordinates
  validity interval
  actual named vectors or native document input
}

ViewManifestEntry {
  view / stable point
  exact retained source ObjectRef / revision
  representation / logical occurrence
  exact byte/line range
  prepared reference / verification data
}
```

Equal text is not one occurrence. A disappeared/reappeared occurrence receives a new incarnation. Revision,
absolute offsets and citation range do not belong in stable retrieval identity. A later view may reuse the
point while binding it to a new exact revision/range; an old pinned view keeps its old binding.

### Publication records

```text
CurrentPublishedView {
  view_id / monotonic sequence
  route / physical generation / retrieval profile
  manifest/evidence roots
  selected source bindings
}

PendingPublication {
  operation_artifact_ref
  expected_current_view
  target_view
  next apply/readback cursor
}

RetiredCleanup {
  cleanup_artifact_ref
  kind: exact point IDs | whole physical generation
  route / generation
  retire_before_sequence
  next delete/readback cursor
}
```

`RetiredCleanup` is finite unfinished external state, not history. Remove each row after exact success.

## 4. Ingestion and current control

### Parse settings/invocation once

Use `args_os()` and bounded typed TOML/Serde. Preserve non-UTF-8 paths. Merge file/env/CLI once and pass one
closed value. Unknown fields/commands fail before starting work. Do not hash configuration, freeze it behind
a receipt or reread argv/env in chained handlers.

### Inventory and change hints

Use one `ignore::WalkBuilder` inventory. Feed raw `notify::RecommendedWatcher` events as hints into the same
bounded dirty queue. Coalesce short editor/tool bursts and reread the final admitted path. Overflow or
uncertain continuity schedules an affected-root scan.

A remove/move-from hint alone is not retirement while replacement/rescan is pending. Incomplete/cancelled
scans never retire unseen files. Do not use a whole-tree `FileIdMap`, duplicate `PollWatcher` or supervisor
per root.

For Git commit views, resolve exact commit/tree OIDs once, use `ls-tree -r -z --full-tree`, raw
`diff-tree --no-renames` and one reused bounded `cat-file --batch-command -Z`. No checkout, textconv, filters,
hooks, lazy fetch or process per file. `gix` is an optional no-Git alternative only when packaging requires
it; do not ship both by default.

### Retain exactly the acquired bytes

```text
pre-read admission
→ stable final-handle read
→ post-read classification
→ immutable ObjectRef write/readback
→ short source-head compare/set transaction
→ preparation queue
```

The object exists before the source-head reference. Same expected head + same revision replays; same expected
head + different revision conflicts. A known abort is known failure. A genuinely uncertain redb commit is
resolved by point-reading the current source row, not by replaying a registry operation or rereading the path.

Replace `RegistryJournal`, v4 original-input retention/restore, complete `SourceRegistry` replay, generic
batch receipts and full `ControlSnapshot` publication. Use exact point reads and bounded iteration only for a
real inventory/publication snapshot.

### One immutable writer

Extract the useful create-new/file-sync/no-clobber/final-readback sequence already present in Eliot's old
cutover artifact code. Use unique concurrent-safe staging names and remove the stage after success or reuse.

The current Windows directory-sync path is a no-op returning success. Qualify a native same-volume
no-clobber/write-through profile or state a weaker guarantee explicitly. Final readback proves logical state,
not power-loss durability. Do not hide the gap behind receipts.

## 5. Preparation and projection

### Prepare once

Pass the owned materialization result directly into unitization. Validate a persisted/external artifact when
loading it; do not reproduce the complete parser between adjacent functions.

Use structure-first boundaries:

- Markdown/JATS sections/items;
- code definitions/blocks;
- native document elements/pages;
- line-preferred plain-text fallback;
- local split only for oversized structures;
- merge adjacent small peers only under the same context.

Tokenizer/model packing is a derived projection, not source occurrence identity. FastCDC-like boundaries are
only an evaluated fallback inside oversized unstructured sections, never semantic identity.

Keep source bytes/ranges separate from bounded path/title/heading/signature retrieval context. Cache model
work only by exact input, context, role, model/task and relevant options. A cache hit can still require a new
manifest/Qdrant binding.

### Emit bounded publication pages

Planner output contains actual uploadable text/vector values, stable point identity and target evidence
entries. Use ordered merge walks; do not materialize both complete manifests and all diff ID copies into a
single canonical digest object. A harmless source-coordinate shift updates only the new view manifest.

## 6. Qdrant and publication

### One real Qdrant runtime

Use the workspace-pinned client privately. One daemon-owned client/runtime serves both publication and query.
Construct it from selected endpoint and optional actual credential. Key rotation rebuilds the client.

Remove runtime `QualifiedGate`, executable/client/probe receipts, API-key equality leases, process-local
mutation replay and relative timeout renewal. Live qualification is release/upgrade evidence, not a runtime
credential.

Carry one absolute monotonic deadline and cancellation through every RPC. Before dispatch, timeout is
known. After a mutation may have reached Qdrant, outcome is unresolved and publication inspects actual state.
Reads commit nothing.

Use exact native operations:

- full upsert only for new/complete authoritative point replacement;
- `update_vectors` / `delete_vectors` for selected vectors;
- payload set/delete/overwrite with merge/replacement distinguished;
- `wait=true` plus exact relevant readback;
- exact ID delete or whole-generation delete;
- native prefetch/RRF/grouping for compatible indexed branches.

Local baseline: one node/shard/replica, one serialized publisher and default/weak ordering with exact
readback. Stronger topology/ordering is an explicit measured profile, not a correctness floor.

### Native lexical profile

Use Qdrant 1.19 `Document` input for stored and query `qdrant/bm25`. Bind document/query role, `k`, `b`,
`avg_len`, tokenizer, case, stemmer, stopwords and folding to the published profile. Bounded identifier
expansion is document input, not a second local sparse engine. Each sparse leaf uses the selected base
population as `params.idf.corpus`; query-only narrowing does not silently change that denominator.

A profile/vector/schema incompatibility creates and fully verifies a new physical generation, then publishes
it. Adding a vector name does not populate old points.

### Publication effect loop

```text
write/verify immutable operation artifact
→ redb PendingPublication
→ for each page:
     observe if prior outcome uncertain
     dispatch missing Qdrant work
     exact readback
     advance small cursor
```

No redb writer across object/Qdrant I/O. No bridge receipt ledger. Conflicting observed state blocks the
target while the committed current view remains serviceable. After possible external effect, finish forward
under the same operation; no compensation theatre, sequence rewind or invalidation-only publication.

### Linearizable current-view commit

Use one short standard-library mutex around the view manager. After all pages verify:

```text
block pin_current
→ redb transaction:
     compare current/pending
     CurrentPublishedView = target
     RetiredCleanup += unfinished physical delete, if any
     PendingPublication = none
→ commit
→ old Weak + new current Arc
→ unblock
```

No Qdrant/object I/O under the lock. If commit acknowledgement is uncertain, inspect typed rows before
unblocking and install exactly the durable result. Clearing pending without cleanup loses work after crash;
retaining the whole completed pending operation creates an archive. Transfer only unfinished delete pages.

### View manager and cleanup eligibility

```text
current: Arc<PublishedViewState>
retired: ViewId → Weak<PublishedViewState>
```

`pin_current()` clones only current. A live request may clone its guard into a `QueryWindow`. No route,
generation, sequence, owner ID or renewal API. Last old drop wakes cleanup; periodic scanning is fallback.
Never delete inside `Drop`.

Correct cleanup conditions:

```text
point IDs closed at N:
  no live view on same route/generation has sequence < N

old collection:
  no live view uses that route/generation
```

The immediate predecessor is not enough: v1 can remain live after v2 is gone while v3 closes a point seen by
both. After restart old interactive guards are gone; durable cleanup rows remain and are reevaluated.

The cleanup table is finite. When full, backpressure publication before another external operation; never
evict a live view or delete optimistically. Unknown delete keeps the same cursor and retries exact readback.

## 7. Immutable-object GC

There is a real race between final object publication/reuse and the first redb reference. Use one
process-local `RwLock<()>` or equivalent:

```text
producer shared guard:
  acquire before final object publication/reuse
  → finalize/verify object
  → commit first source/prepared/pending/cleanup/history reference
  → release

bounded GC exclusive guard:
  snapshot current source/prepared, current view, pending, cleanup and live-view roots
  → traverse only known current-format manifests under finite limits
  → enumerate/delete one exact unreferenced page
  → release/yield
```

Reads do not need the gate. Strong live-view snapshots protect old manifests for the pass. Missing/corrupt/
unsupported roots, incomplete inventory or uncertain delete retain bytes. Failed deletion remains
rediscoverable; no sweep journal is required. Staging files are outside the final namespace and have separate
bounded debris cleanup.

Delete `search-retention`, generic leases/graphs/sweep/purge, `wave7-lifecycle`, restore composition and their
config/tooling/fixtures after callers move.

## 8. Query, evidence and paging

### One current request

Use current typed operations only:

```text
Search    ranked source-backed evidence
Exact     one bounded exhaustive retained-source scan
Continue  next QueryWindow page
Expand    bounded context around one emitted item
Orient / Compare only when real source-backed handlers exist
```

Do not translate current requests through `RecipeIdV1`, `SearchTaskPlan`, capabilities, grants, plan digests
or a V3 compatibility wrapper.

### Scope and request context

Resolve explicit/default source IDs against the pinned view. Unknown IDs fail; explicit empty never widens.
Compatible sources share one route/profile retrieval/IDF population. Split only incompatible routes/profiles.
Optional restricted session can only narrow.

One request owns:

```text
PublishedViewGuard
ResolvedScope
absolute deadline
cancellation token
remaining backend nominations
remaining evidence items
remaining source-read bytes
remaining retained/output bytes
```

A small RAII semaphore may bound concurrency. No public scheduler snapshot, tickets, terminal map or durable
query ledger. Nested operations never reset the deadline.

### Retrieval and evidence

Baseline Search is one real Qdrant BM25 call; preserve Qdrant order. Compatible hybrids use native Qdrant
prefetch/RRF/grouping. A small local integer rank merge exists only for a real non-Qdrant branch. Never compare
raw scores across populations or include leg ID in candidate identity. Ordinary top-k is non-exhaustive and
does not call unconditional exact count.

A hit becomes evidence only through:

```text
stable point
→ pinned ViewManifestEntry
→ exact retained ObjectRef / representation / range
→ real immutable-object/range verification
→ VerifiedEvidenceItem
```

Select one output page before retained-source I/O. Group selected reads by object/representation. A foreign/
conflicting/missing manifest point contaminates the backend result; a valid entry whose object/range is
unavailable becomes a gap. Echoed payload/vector/readback digests are not evidence.

Projection preserves final order, enforces remaining output limits and formats cards. If trivial, inline it
and delete the package.

### One QueryWindow

```text
QueryWindow {
  id
  PublishedViewGuard
  final ordered EvidenceRef[]
  next_index
  optional issued item locators / cached excerpts
  absolute expiry
  finite metadata/read/output limits
}
```

First-page insertion and later cursor advance commit only after a complete frame flushes. Failed/partial
output advances zero and closes the session/window safely. Page selection is direct slicing, not prefix or
corpus rescanning.

Cards contain source/revision/representation/location/excerpt directly. Optional
`WindowItemRef { window_id, item_index }` expands only a finite neighborhood around that exact manifest item;
it cannot request arbitrary bytes from the revision. Restart may expire all windows.

Delete shared and daemon handle/continuation stores, source-catalog fences, random per-item tokens, durable
replan/security histories and arbitrary-range expansion.

### Exact scan

Use the existing Aho-Corasick literal matcher and optional one bounded `regex-automata` adapter. Stream one
ordered pinned retained inventory, verify/read/release one object at a time and report truthful complete or
incomplete outcome. Delete compile/execute plans, permits, supplied readbacks, denominator digests,
checkpoints and the second matcher.

## 9. Local protocol and CLI

Use one wire:

```text
[u32 little-endian body length][bounded UTF-8 Serde JSON]
```

Check declared size before allocation, read exactly one body, decode current DTO, dispatch one actual handler,
optionally emit finite progress and exactly one terminal response. Encode/size-check the complete output frame
before the first write. Partial output is fail-stop; append no second error and commit no window/cursor state.

Trusted baseline is owned child stdin/stdout; an owner-only OS pipe is optional. It needs no pairing challenge,
per-frame MAC, durable binding, product grant or capability digest. A selected restricted-client adapter may
authenticate externally and terminate into the same dispatcher.

CLI captures one invocation, owns child/stream handles, converts timeout once to an absolute deadline, sends
best-effort cancellation on interrupt and performs bounded child cleanup. `Continue` carries `WindowId`;
`Expand` carries `WindowItemRef` plus relative bounded context. Use Serde for human/JSON/NDJSON presentation;
do not substring-parse events or concatenate JSON.

Delete provider pairing/binding/grant/manual JSON/recipe stacks, typed old session, legacy core/TCP/token/
endpoint/tab/hex paths and old wire/crypto fixtures with their callers.

## 10. Optional depth and documents

- Orientation: existing path/title/heading/syntax/Markdown link records first; bounded relations only.
- Code outline: selected `ast-grep outline` output with real ranges; import actual SCIP/LSP locations when
  available. Do not infer missing semantic relationships.
- PDF: one selected native extractor (for example `pdfium-render`) returning page/text coordinates; OCR only
  for pages without useful text.
- Dense retrieval: one real selected model/profile and actual vector input. No model registry before caller.
- Reranking: finite source-backed subset only; it may reorder/subset, not generate evidence.
- Editor overlay: one explicit process-local shadow branch over matching source occurrences.
- No GraphRAG/notebook database/query-rewriting LLM/generated answer pipeline in Search baseline.

## 11. Donor mechanisms

| Seam | Reuse | Reject |
|---|---|---|
| CLI | ripgrep parse-once direct dispatch | repeated argv capture or command framework |
| Inventory | `ignore::WalkBuilder` + raw `notify` hints | whole-tree ID cache / duplicate poller |
| Git | `ls-tree`, `diff-tree --no-renames`, reused `cat-file --batch-command` | checkout/filter/process-per-file |
| Control | redb short typed tx + MVCC reads | eternal operation ledger / full snapshot copies |
| Objects | Eliot cutover finalization shape + Git temp/final publication | migration owner / permanent receipt |
| Framing/config | Serde JSON/TOML on bounded input | hand-written grammars / canonical hashes |
| Deadlines | one absolute `timeout_at`/remaining-time pattern | renewed duration per phase/RPC |
| Qdrant | pinned client, native BM25/IDF/fusion/grouping and exact mutations | second backend / local sparse platform |
| Structure | existing parsers + selected Docling/Unstructured boundary ideas | imported document framework |
| View lifetime | Lucene in-memory snapshot rule | durable query leases / public reclaim permits |
| Object GC | Tantivy living-file callback pattern | second index / generic managed-file authority |

Lucene explicitly says its snapshot state is not persisted; Eliot persists only real unfinished cleanup.
Tantivy supplies the “derive living files from committed/in-flight metadata” idea; Eliot roots come from its
own typed current/pending/cleanup/live records.

## 12. Implementation order

1. **#330/#270:** one immutable writer/reader and one real source head using #598's accepted handoff.
2. **#331:** one materialization/structure-first unitization and prepared ref.
3. **#259/#329:** stable point identity + versioned manifest evidence.
4. **#263:** one real Qdrant runtime and native BM25 path.
5. **#261/#300/#303:** pending publication, current-view manager and finite cleanup rows.
6. **#302:** object-GC gate and known-root bounded GC.
7. **#272/#291/#293/#279:** selected scope, request context, evidence lookup and connected Search.
8. **#275/#280/#332:** QueryWindow, framed protocol and actual CLI.
9. **#249:** direct exact scan.
10. Remove superseded packages/features/tests as each caller cutover lands; do not defer all deletion to a
    final cleanup mega-task.

Later: one edit/rename, restart, lost-index rebuild, documents/code depth, optional models and performance.

## 13. Verification and delivery

During implementation, write code first and run minimal pinned compiler/Clippy for changed production owners
and immediate consumers. Use only focused checks needed to expose catastrophic semantic mistakes in the
current slice. Broad test matrices, native/platform qualification and scale work follow after the connected
product path exists.

Minimum product evidence before calling Search ready:

- retain a real file and restart;
- prepare/index/query it through real CLI/daemon/Qdrant;
- reopen exact retained excerpt;
- update/rename without suffix-wide rewrite;
- explicit scope and IDF population;
- old query survives view switch;
- v1/v2/v3 cleanup threshold and generation cleanup;
- crash at pending/current/cleanup boundaries;
- GC finalization→reference race;
- paging/expansion/partial output;
- cancellation/deadline/no renewal;
- delete Qdrant and rebuild;
- unsupported modes fail truthfully.

Do not claim Windows durability, live Qdrant behavior, recovery, performance or readiness from source presence,
fixtures, capability descriptors, review records or compilation alone.
