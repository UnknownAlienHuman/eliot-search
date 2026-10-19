# ELIOT Search — implementation

Read the [architecture manifesto](../architecture/README.md) first and keep it in the lead developer's
context. This document says what to change, where, and what to delete. Historical waves, old receipts,
pre-release formats and fixture-only APIs are evidence—not compatibility requirements.

**Inspected target:** public `main` at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, 2026-10-18/19.
Preserve #598's active private source-retention work and consume only its accepted public handoff. Do not
inspect, reset, rebase or assign a competing writer to its branch/worktrees.

## 1. Build one application

```text
ingest:
  selected source
  → stable final-handle read
  → immutable ObjectRef
  → current SourceRecord/RevisionRef
  → one MaterializationProduct
  → structure-first units / retrieval inputs
  → Qdrant mutation + exact readback
  → CurrentPublishedView

query:
  typed SearchRequest
  → pin current PublishedView
  → resolve selected scope
  → one RequestContext
  → real Qdrant BM25
  → ViewManifestEntry
  → retained object/range verification
  → EvidenceCard[]
  → optional QueryWindow
```

The first finished slice retains one real file across restart, indexes it, finds it from the CLI and reopens
the correct excerpt. Then make one edit update only affected work, rebuild a lost Qdrant index and cancel a
bounded request. DIRECT remains an explicit exact/source scan, never a silent indexed fallback.

A constructor, capability, receipt, closed issue, package split or shape test is not delivery. Connect the
real producer and consumer and remove the replaced path in the same slice.

## 2. Owners and cutovers

| Area | Replace | Keep/build |
|---|---|---|
| CLI/protocol | legacy tab/hex, paired provider host, V1 recipes/V2 endpoint, manual JSON, grants/capabilities | one child-stdio client, `u32-le + bounded Serde JSON`, direct handlers, one terminal response |
| Root ownership | durable owner epochs/records, sealed/migration ownership ceremony | one exclusive OS lock held for the daemon lifetime; it is the prerequisite for process-local GC coordination |
| Control | eternal `OPERATIONS`, full `ControlSnapshot` copies, registry v4 replay/original inputs | short typed redb transactions and point/bounded-page reads |
| Source | admission which drops bytes, refusal-only sync, duplicate stores | one inventory queue, one stable read, one immutable object writer, one current source head |
| Preparation | adjacent-stage reparsing and a second artifact writer | one materialization, structure-first units, one prepared artifact |
| Identity | revision/absolute-offset-bound Unit/S11/payload identity | stable retrieval point plus versioned manifest evidence |
| Qdrant | runtime qualification credentials, key-equality leases, in-memory mutation receipts | one daemon-owned client/runtime, native BM25, exact full/narrow mutations and readback |
| Publication | three phase/receipt machines, compensation and snapshot acknowledgements | one immutable operation artifact, one pending row, one durable current view and finite unfinished cleanup rows |
| View lifetime | route/epoch pin registry, owner IDs, renewals, sampled watermarks | one small `Mutex + Arc/Weak` current-view manager |
| Query | recipe/plan/access/executor/validator/projector operating systems | direct `Search/Exact/Continue/Expand`, one scope/context, source-backed cards |
| Paging | four handle/continuation stores and catalogue-wide fences | one process-local `QueryWindow` |
| GC/retention | generic leases, graph sweep, purge, restore and reclaimer journals | typed roots, one bounded object-GC gate, publication-owned Qdrant cleanup |

Packages are current locations, not permanent boundaries. Inline or merge a package when its remaining code
does not isolate a real dependency or effect.

## 3. Current data

### redb

```text
AppliedSettings
RootRecord
SourceRecord { source_id, root_id, locator/view, current_revision, availability }
PreparedRef { revision, preparation_profile, artifact_ref }
CurrentPublishedView
PendingPublication?
RetiredCleanup[]
```

Do not store source text, vectors, complete point pages, query history, completed operations or caller proofs.

### immutable objects

```text
ObjectRef { content identity, length, storage profile }
RevisionObject
PreparedArtifact
PublicationOperation / ManifestPage / CleanupPage
```

One writer/reader owns final object names. Unsupported old roots fail before writes and remain untouched.

### retrieval and evidence

```text
SourceOccurrence {
  source_id, structural_anchor, occurrence_incarnation, duplicate_position,
  current revision/representation/range
}

RetrievalInput {
  role(document|query|rerank), retrieval_profile,
  bounded contextual prefix, exact input identity
}

RetrievalPoint {
  stable point identity + collision evidence,
  occurrence incarnation, retrieval input/profile/role,
  stable filter/group coordinates, validity interval,
  actual vectors or native document input
}

ViewManifestEntry {
  view + point,
  exact retained source ObjectRef/revision,
  representation/logical occurrence/range,
  prepared reference and verification data
}
```

Equal text is not one occurrence. Disappear/reappear creates a new incarnation. Revision, absolute offsets and
citation range do not belong in stable retrieval identity. A later view can reuse a point while binding it to
a new exact revision/range; old views keep their own binding.

### publication

```text
CurrentPublishedView {
  view_id, monotonic sequence,
  route/generation/retrieval profile,
  manifest/evidence roots, selected source bindings
}

PendingPublication {
  operation_artifact_ref,
  expected_current_view, target_view,
  next apply/readback cursor
}

RetiredCleanup {
  cleanup_artifact_ref,
  kind(point IDs | whole generation),
  route/generation,
  retire_before_sequence,
  next delete/readback cursor
}
```

`RetiredCleanup` is finite unfinished external state, not history. Remove each row after exact success.

## 4. Source and control path

### Settings and invocation

Capture `args_os()` and bounded file/env/CLI config once. Preserve non-UTF-8 paths. Decode typed TOML/Serde,
validate actual fields and construct owners. Unknown input fails before side effects. Static settings require
restart; do not build a generic config journal or hash ordinary configuration.

### One root owner

Retain the actual OS exclusion primitive behind `DataRootGuard`, but simplify it to one live owner of the
current-format root. The process-local object-GC gate is valid only while this exclusive root lock is held.
Do not require durable owner epochs, sealed co-holds, migration borrowers or ownership receipts for normal
startup unless a remaining real consumer proves the need.

### Inventory

Use one `ignore::WalkBuilder` traversal and raw `notify::RecommendedWatcher` hints feeding the same bounded
dirty queue. Coalesce editor/tool bursts; overflow/uncertain continuity schedules an affected-root scan.
Incomplete scans never retire unseen files; remove/move-from alone is not retirement while replacement or
rescan is pending. Do not use a whole-tree `FileIdMap`, duplicate `PollWatcher` or supervisor per root.

For Git commit views, resolve exact OIDs once, use `ls-tree -r -z --full-tree`, raw
`diff-tree --no-renames` and one reused bounded `cat-file --batch-command -Z`. No checkout, textconv, filters,
hooks, lazy fetch or process per file. `gix` is optional only when packaging requires a no-Git installation.

### Retain the exact acquired body

```text
pre-read admission
→ stable final-handle read
→ post-read classification
→ immutable object write/readback
→ short source-head compare/set transaction
→ preparation queue
```

The object exists before its source-head reference. Same expected head/same revision replays; same expected
head/different revision conflicts. Resolve uncertain redb commit by point-reading the source row—not by
replaying a registry operation or rereading the pathname.

Delete `RegistryJournal`, v4 original-input retention/restore, full `SourceRegistry` replay, generic batch
receipts and complete `ControlSnapshot` publication. Use exact point reads and bounded iteration only for a
real inventory/publication snapshot.

### One immutable writer

Extract Eliot's useful create-new/file-sync/no-clobber/final-readback sequence from the old cutover artifact.
Use unique staging names and remove the stage after success/reuse. The current Windows directory-sync path is
a no-op returning success: qualify a real same-volume no-clobber/write-through profile or state the weaker
guarantee. Final readback is not power-loss durability, and receipts cannot make it so.

## 5. Preparation and point projection

Pass the owned `MaterializationProduct` directly into unitization. Validate persisted/external artifacts when
loading them; do not rebuild Markdown/JATS between adjacent stages.

Use structure-first boundaries: Markdown/JATS sections/items, code definitions/blocks, native document
items/pages and line-preferred plain-text fallback. Split only oversized structures; merge small adjacent
peers only under the same context. Tokenizer/model packing is a derived projection, not source identity.
FastCDC-like boundaries are only an evaluated fallback inside oversized unstructured sections.

Keep source bytes/ranges separate from bounded path/title/heading/signature retrieval context. Cache model
work only by exact input, context, role, model/task and relevant options. A cache hit may still require a new
manifest/Qdrant binding.

Planner emits bounded ordered pages with uploadable text/vector values, stable point identity and target
`ViewManifestEntry` values. Use ordered merge walks. Do not rebuild both complete manifests and every diff
ID into another canonical digest preimage.

## 6. Qdrant

Use the pinned client privately. One daemon-owned client/runtime serves publication and query. Construct it
from the configured endpoint and optional actual credential; key rotation rebuilds the client.

Remove runtime `QualifiedGate`, executable/client/probe receipts, API-key equality leases, process-local
mutation replay and relative timeout renewal. Qualification is release/upgrade evidence, not a runtime token.

Carry one absolute monotonic deadline/cancellation through every RPC. Before mutation dispatch, timeout is
known; after a request may have reached Qdrant, outcome is unresolved and publication inspects actual state.
Reads commit nothing.

Use native semantics exactly:

- full upsert only for a new/complete point replacement;
- `update_vectors` / `delete_vectors` for selected vectors;
- payload set/delete/overwrite with merge and replacement distinguished;
- `wait=true` and exact relevant readback;
- exact ID or whole-generation deletion;
- native prefetch/RRF/grouping for compatible branches.

Baseline is one node/shard/replica, one serialized publisher and default/weak ordering with exact readback.
Stronger topology/ordering is a measured profile, not a universal correctness floor.

Use Qdrant 1.19 `Document` for stored/query `qdrant/bm25`. Bind document/query role, `k`, `b`, `avg_len`,
tokenizer, case, stemmer, stopwords and folding to the published profile. Bounded identifier expansion is
input text, not a second sparse engine. Each sparse leaf uses the selected base population as
`params.idf.corpus`; query-only narrowing does not silently change that denominator.

Profile/vector/schema incompatibility builds and verifies a new physical generation, then publishes it.
Adding a vector name does not populate old points.

## 7. Publication and lifetime

### Apply and recover

```text
write/verify operation artifact
→ commit PendingPublication
→ for each page:
     observe if prior outcome uncertain
     dispatch missing Qdrant work
     exact readback
     advance cursor in a short redb transaction
```

No redb writer across object/Qdrant I/O. No bridge receipt ledger. After possible external effect, finish
forward under the same operation; no compensation theatre, sequence rewind or invalidation-only publication.

### Durable commit, then short pointer swap

After all apply pages verify:

```text
redb transaction:
  compare current/pending
  CurrentPublishedView = target
  RetiredCleanup += unfinished physical delete, if any
  PendingPublication = none
→ commit
→ under a short view-manager mutex:
     retired += Weak(old current)
     current = Arc(target)
→ report publication visible
```

Do **not** hold the view-manager mutex across redb fsync. A query which pins the old view between durable
commit and pointer swap remains correct: its immutable sequence/manifest still works and merely delays
cleanup. If the process crashes in that gap, restart installs the durable target and no old request survives.
If redb commit acknowledgement is uncertain, resolve typed rows before swapping. Publication completion is
reported only after the in-memory current pointer is installed.

Serving/query/status use the manager's current view. redb is the restart/recovery source, not a second live
routing authority.

### View manager

```text
current: Arc<PublishedViewState>
retired: ViewId → Weak<PublishedViewState>
```

`pin_current()` briefly locks and clones only current. A live request may clone its guard into a
`QueryWindow`. There is no route/generation/sequence/owner-ID acquisition or renewal API. The manager stores
only weak old refs and prunes dead ones. Last old drop only wakes cleanup; periodic scanning is fallback.
Never delete in `Drop`.

### Correct cleanup threshold

```text
point IDs closed at sequence N:
  safe iff no live view on the same route/generation has sequence < N

old collection:
  safe iff no live view uses that route/generation
```

The immediate predecessor is insufficient: v1 may remain live after v2 is gone while v3 closes a point seen
by both. After restart, interactive old guards are gone; durable cleanup rows remain and are reevaluated.

The cleanup table is finite. If full, pause new publication before another external mutation and continue
serving/cleanup. Do not evict a live view or delete optimistically. Unknown deletion keeps the same cursor and
retries exact readback. A later generation-cleanup row may subsume exact rows only through an explicit exact
same-generation transfer; baseline may simply retain/backpressure rather than invent another merger.

## 8. Immutable-object GC

The root's exclusive OS lock guarantees one live writer/GC process. Inside it, use one process-local
`RwLock<()>` or equivalent to close the finalization→first-reference race.

```text
producer shared guard:
  acquire before final object publication/reuse
  → finalize/verify object
  → commit its first source/prepared/pending/cleanup/history reference
  → release

bounded GC exclusive guard:
  snapshot current source/prepared, current view, pending, cleanup and live-view roots
  → traverse known current-format manifests under finite limits
  → enumerate/delete one exact unreferenced page
  → release/yield
```

Lock order is fixed:

1. root OS lock exists for process lifetime;
2. object-GC shared/exclusive gate;
3. redb transaction;
4. view-manager mutex is never held while acquiring the GC gate or running redb/object/Qdrant I/O.

Reads need no GC guard. Strong live-view snapshots protect old manifests during a pass. Missing/corrupt/
unsupported roots, incomplete inventory or uncertain delete retain bytes. Failed deletion is rediscovered;
no sweep journal. Staging names are outside the final namespace and have separate bounded debris cleanup.

Delete `search-retention`, generic leases/graphs/sweep/purge, `wave7-lifecycle`, restore composition and their
configuration/tooling/fixtures after callers move.

## 9. Query and evidence

Current operations:

```text
Search    ranked source-backed evidence
Exact     one bounded exhaustive retained-source scan
Continue  next QueryWindow page
Expand    bounded context around one emitted item
Orient / Compare only when real handlers exist
```

Do not translate through `RecipeIdV1`, `SearchTaskPlan`, capabilities, grants, plan digests or a V3 wrapper.

Resolve requested source IDs against the pinned view: unknown fails, explicit empty never widens, compatible
sources share one route/profile retrieval/IDF population, and optional restricted session can only narrow.
Split only incompatible routes/profiles.

One request owns:

```text
PublishedViewGuard
ResolvedScope
absolute deadline + cancellation
remaining backend nominations / evidence items
remaining source-read / retained-result / output bytes
```

A small RAII semaphore may bound concurrency. No public scheduler, tickets, terminal map or durable query
ledger. Nested work never resets the deadline.

Baseline Search is one Qdrant BM25 call; preserve Qdrant order. Compatible hybrids use native Qdrant fusion.
A private integer rank merge exists only for a real non-Qdrant branch. Never compare raw scores across
populations or include leg ID in candidate identity. Ordinary top-k is non-exhaustive and does not call
unconditional exact count.

A hit becomes evidence only through:

```text
stable point
→ pinned ViewManifestEntry
→ exact retained ObjectRef/representation/range
→ actual immutable-object/range verification
→ VerifiedEvidenceItem
```

Select the output page before source I/O and group reads by object/representation. Foreign/conflicting/missing
manifest points fail the backend result; a valid entry with unavailable evidence becomes a gap. Echoed payload
or vector digests are not evidence. Projection preserves final order and only formats bounded cards; inline
it if trivial.

### QueryWindow

```text
QueryWindow {
  id, PublishedViewGuard,
  final ordered EvidenceRef[], next_index,
  optional issued item locators/cached excerpts,
  absolute expiry, finite metadata/read/output limits
}
```

Insert the first window and advance later pages only after a complete response frame flushes. Partial output
advances zero and closes safely. Page selection is direct slicing. Cards include source/revision/location/
excerpt. `WindowItemRef { window_id, item_index }` expands only a bounded neighborhood around that exact item,
not arbitrary revision offsets. Restart may expire all windows.

Delete shared and daemon handle/continuation stores, source-catalog fences, random per-item tokens, durable
replan/security histories and arbitrary-range expansion.

### Exact

Use the existing Aho-Corasick matcher and optional one bounded `regex-automata` adapter. Stream one ordered
pinned inventory, verify/read/release one object at a time and report truthful complete/incomplete outcome.
Delete compile/execute plans, permits, supplied readbacks, denominator digests, checkpoints and the second
matcher.

## 10. Local protocol and CLI

One wire:

```text
[u32 little-endian body length][bounded UTF-8 Serde JSON]
```

Check size before allocation, read exactly one body, decode the current DTO, dispatch one actual handler,
optionally emit finite progress and exactly one terminal response. Encode/size-check the whole output frame
before first write. Partial output is fail-stop; append no second error and commit no window/cursor state.

Trusted baseline is owned child stdin/stdout; an owner-only OS pipe is optional. It needs no pairing challenge,
per-frame MAC, durable binding, product grant or capability digest. A selected restricted-client adapter may
authenticate separately and terminate into the same dispatcher.

CLI captures one invocation, owns child/stream handles, converts timeout once to an absolute deadline, sends
best-effort cancellation on interrupt and performs bounded child cleanup. `Continue` carries `WindowId`;
`Expand` carries `WindowItemRef` plus bounded relative context. Use Serde for machine output; do not substring-
parse events or concatenate JSON.

Delete provider pairing/binding/grant/manual JSON/recipe stacks, the old typed session, legacy core/TCP/token/
endpoint/tab/hex paths and their wire/crypto fixtures with callers.

## 11. Optional depth

- Orientation: existing path/title/heading/syntax/Markdown link facts first; bounded relations only.
- Code outline: selected `ast-grep outline` output; import actual SCIP/LSP locations when available.
- PDF: one native extractor returning page/text coordinates; OCR only when useful text is absent.
- Dense retrieval: one selected model/profile and actual vector input; no model registry before caller.
- Reranking: finite source-backed subset only; it may reorder/subset, never create evidence.
- Editor overlay: one explicit process-local branch shadowing matching occurrences.
- No GraphRAG/notebook database/query-rewriting LLM/generated-answer pipeline in baseline Search.

## 12. Donor seams

| Seam | Reuse | Reject |
|---|---|---|
| CLI | ripgrep parse-once dispatch | repeated argv capture/framework |
| Inventory | `ignore::WalkBuilder` + raw `notify` hints | whole-tree ID cache/duplicate poller |
| Git | `ls-tree`, raw `diff-tree`, reused `cat-file --batch-command` | checkout/filter/process-per-file |
| Control | redb typed tx + MVCC reads | operation ledger/full snapshots |
| Objects | Eliot cutover finalization shape + Git temp/final publication | migration owner/permanent receipt |
| Framing/config | bounded Serde JSON/TOML | hand-written grammar/canonical hash |
| Deadlines | one absolute remaining-time pattern | renewed duration per phase/RPC |
| Qdrant | pinned client, native BM25/IDF/fusion/grouping, exact mutations | second backend/local sparse engine |
| Structure | existing parsers + selected Docling/Unstructured boundary ideas | imported document framework |
| View lifetime | Lucene in-memory snapshot rule | durable query leases/public reclaim permits |
| Object GC | Tantivy living-file callback idea | second index/generic managed-file authority |

Lucene explicitly states its snapshot state is not persisted; Eliot persists only real unfinished cleanup.
Tantivy supplies the “derive living files from committed/in-flight metadata” idea; Eliot roots come from its
own typed current/pending/cleanup/live records.

## 13. Implementation order

1. **#330/#270:** one immutable writer/reader and one real source head using #598's accepted handoff.
2. **#331:** one materialization, structure-first units and prepared ref.
3. **#259/#329:** stable point identity and versioned manifest evidence.
4. **#263:** one real Qdrant runtime and native BM25 path.
5. **#261/#300/#303:** pending publication, commit-then-swap view manager and finite cleanup rows.
6. **#302:** object-GC gate and known-root bounded GC.
7. **#272/#291/#293/#279:** selected scope, request context, evidence lookup and connected Search.
8. **#275/#280/#332:** QueryWindow, framed protocol and actual CLI.
9. **#249:** direct exact scan.
10. Delete superseded packages/features/tests as each caller cutover lands; do not defer removal to a final
    mega-task.

## 14. Verification

Write code first. Run minimal pinned compiler/Clippy for changed production owners and immediate consumers.
Use focused checks only for catastrophic semantics in the current slice; broad matrices/native/scale work
follow after the connected product exists.

Minimum readiness evidence:

- retain a real file and restart;
- real CLI → daemon → Qdrant → retained excerpt;
- edit/rename without suffix-wide rewrite;
- explicit scope and IDF population;
- old query survives view switch;
- v1/v2/v3 cleanup threshold and generation cleanup;
- crash at pending/current/cleanup/pointer-swap boundaries;
- GC finalization→first-reference race under one root owner;
- paging/expansion/partial output;
- cancellation/deadline without renewal;
- delete Qdrant and rebuild;
- unsupported modes fail truthfully.

Do not claim Windows durability, live-Qdrant behavior, recovery, performance or readiness from source presence,
fixtures, capability descriptors, review records or compilation alone.
