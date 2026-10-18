# ELIOT Search — implementation

Read the [architecture manifesto](../architecture/README.md) first and keep it in the lead developer's
context. This document owns what to change, where to change it and which donor mechanisms to use.

**Inspected source:** public `main` at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, 2026-10-18 UTC.
This is the refactoring target, not a claim that the indexed application already works. Preserve #598's
active manager/private work and consume its useful held-source result when integrating. Historical waves,
closed issues and pre-release formats are evidence, not prerequisites or compatibility obligations.

## 1. Build one application

Use the existing CLI, daemon, source owners, redb owner and Qdrant bridge. Do not create another product,
backend, lifecycle platform or orchestration framework.

```text
ingest: selected source → stable read → retained revision → prepared units
        → bounded Qdrant mutation/readback → published view

query:  typed request → current-view guard → scoped Qdrant query
        → view-manifest binding → retained bytes → excerpt/location
```

The first completed slice is one real local file retained across restart, indexed, found from the CLI and
reopened at the correct excerpt. Then make one edit update only affected work, restart with the same view,
rebuild a lost index and cancel a bounded request. DIRECT remains an explicit exact/source scan, not a
silent fallback that turns an indexed failure into success.

### Historical requirements kept with small mechanisms

| Useful workload | Implementation |
|---|---|
| Local standalone search | CLI/daemon, redb and local Qdrant. Lexical search needs no model service. |
| Large repositories and portfolios | Selected roots and commit/index/worktree views. Portfolio is a scope filter, not duplicate points or collections. |
| Exact identifiers and negative proof | Ranked BM25 for navigation; literal/regex exact scan over a stated retained inventory for completeness. |
| Incremental evidence | One inventory queue, changed-byte retention, structure-first units and one published view. |
| Orientation and comparison | Paths/headings/syntax facts first; import actual semantic locations when available. |
| Documents | Existing Markdown/JATS plus one selected native PDF extractor with page/text coordinates. |
| Unsaved edits | One explicit in-memory editor overlay that shadows matching source occurrences. |
| Recovery | Keep current source/control state and unfinished external publication; rebuild derived index data. |
| Backup | Stop the daemon, close writers, copy one coherent current-format root and rebuild Qdrant after restore. |
| Responsiveness | Bounded batches, one deadline, cancellation and interactive priority. |

Not baseline: backward readers for pre-release roots, universal migration, legal-hold/purge workflows,
cross-user key migration, online research, generated answers, a second search backend or durable records
for every pure transition.

## 2. Existing owners and required result

| Responsibility | Existing source | Required result |
|---|---|---|
| Invocation and config | [`eliot-search`](../../bins/eliot-search/src/entry.rs), [`eliot-searchd`](../../bins/eliot-searchd/src/entry.rs), `search-config` | Parse invocation/config once and construct real handlers from typed settings. Remove receipt-gated startup and repeated argv/env capture. |
| Source inventory | `search-source-reconcile`, `search-safe-reader`, daemon source roots | One bounded dirty-path/scan queue across startup, watcher hints, periodic recovery and explicit sync. |
| Source retention | daemon native-source composition, `search-revision-store`, `search-control-redb` | Pass the bytes already read into one immutable writer; atomically record source/head references after the object exists. |
| Preparation | `search-materializer`, `search-unitizer`, `search-code-enricher`, `search-lexical` | Produce one prepared artifact and structure-first source units without replaying the full materializer. |
| Projection | `search-projection-planner`, `search-point-identity` | Separate stable retrieval-point identity from revision-specific evidence binding; emit ordered bounded manifest pages and uploadable input. |
| Publication | `search-publication`, daemon publication composition, redb publication records | One recoverable pending artifact, one external mutation owner and one visible-view commit. |
| Qdrant | `search-qdrant-bridge` and its existing blocking runtime | Private SDK translation for schema, full/narrow mutations, readback and scoped query. No process-local replay ledger. |
| Query/evidence | public dispatch, query composition, planner/executor/validator/projector | One typed request reaches a real indexed handler and returns ranked, source-backed excerpts. |
| View lifetime | `search-epoch-pins`, `search-index-reclaimer` | Replace raw route/epoch pins and generic retention with current-view guards and cleanup owned by publication. |

A package is a current code location, not a permanent boundary. Change an owner and its direct consumers in
one slice. Do not ship a constructor, receipt type or facade without the real producer and consumer.

## 3. Data model to implement

Keep the records needed to answer and update search:

```text
SourceRecord
  source_id, locator/view, current_revision, availability

Revision
  revision_id, content_digest, length, immutable_object_ref

PreparedArtifact
  revision_id, preparation_profile, text/maps/structure, gap if unsupported

SourceOccurrence
  source_id, structural_anchor, occurrence_incarnation, duplicate_position,
  current revision/representation/range

RetrievalInput
  role(document|query|rerank), retrieval profile, bounded context, exact input bytes/digest

RetrievalPoint
  point_id, stable occurrence incarnation, retrieval-input/profile identity,
  stable filter/group coordinates, named vectors, validity interval

PublishedView
  route/generation/epoch, ordered manifest pages, scope/profile, current source bindings

ViewManifestEntry
  point_id → source revision, representation, logical occurrence, exact range,
  prepared-artifact/reference data required to reopen evidence

PendingPublication
  operation artifact ref, target view, effect/readback cursor, finite limits
```

### Separate retrieval identity from evidence binding

Current code cannot retain a point across a harmless source-coordinate shift:

- raw `UnitId` includes revision provenance, absolute coordinates and ordinal;
- S11 includes `representation_id` and `unit_id` in the point key;
- `PointPayload` includes source revision, representation and Unit ID;
- `diff_manifests` rejects a reused point ID whose full entry changed.

Change these owners together. A retrieval point is identified by one stable occurrence incarnation plus the
exact retrieval input/profile/role and stable filter coordinates. Version-specific revision, representation,
range and citation data live in the published view manifest. Consecutive views may retain the point while
updating only their own evidence binding. Changed retrieval input, changed structural meaning or a new
occurrence incarnation creates a new point. If an occurrence disappears and later returns, allocate a new
incarnation rather than reopening a closed interval.

Do not reduce this to a content hash: equal passages can occur more than once. Do not keep old-view citation
meaning in a mutable Qdrant payload. Pinned views must continue to resolve their exact old revision/range.

### Source units are structure-first

Use parser/syntax structure as the primary boundary: Markdown/JATS sections/items, code definitions/blocks
and document-native elements. Split only oversized structures. Merge adjacent small peers only under the
same heading/context and selected size bound. Tokenizer/model-specific packing is a derived projection,
not source occurrence identity.

The current line-preferred layout remains a fallback for plain text. Evaluate content-defined splitting
only inside oversized unstructured sections and only after measuring retained units/points after insertions.
FastCDC-style boundaries are a deduplication mechanism, not semantic identity.

## 4. Finish ingestion without another admission chain

### Settings and startup

Decode bounded TOML/Serde settings, merge file/env/CLI once, validate actual fields and construct owners.
Current startup compares user settings against defaults and calls activation with no receipts, so permitted
values can end in `DAEMON_CONFIG_PARTIAL_REFUSED`. Remove that gate and connect settings to the handlers
that use them. A concrete live setting is either supported or requires restart; no general config journal.

### Filesystem and Git

Use one `ignore::WalkBuilder` inventory and one `notify::RecommendedWatcher` as a hint source. Coalesce
short editor/tool bursts, then reread the final admitted path. A remove/move-from event alone is not source
retirement while a replacement or rescan is pending. An incomplete scan never retires unseen files.

Do not adopt `notify-debouncer-full`'s default whole-tree `FileIdMap`: it recursively walks watched paths,
follows links and retains an ID for every object, duplicating the ignore-filtered inventory. Raw events,
`NoCache` or a small cache populated from admitted inventory are sufficient.

For a Git commit view, resolve the ref once, list the initial tree, use raw `diff-tree --no-renames` for
changes and reuse one bounded `cat-file --batch-command` child for blobs. No checkout, textconv, filters,
lazy fetch or process-per-file. `gix` is an alternative only for an installation that must run without Git;
do not maintain both backends by default.

### Retain the bytes already read

`FreshRootContext::acquire_source_v2` obtains the body, hashes/classifies it and then drops it from the
returned value. Pass that same verified buffer/stream into the immutable writer. Do not reread a pathname
and claim the second contents belong to the first observation. Replace the refusal-only
`SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE` path with actual source/head/update effects.

Use one persistent writer/reader based on `search-revision-store`. Retire materializer's second
staging/write/sync/link/readback engine and old DIRECT storage after callers switch. Write the immutable
object first; then commit its revision/source-head reference in a short redb transaction. A crash before
the reference leaves an unreferenced object for GC; a crash after it leaves preparation to retry.

### Prepare once

Pass the owned materializer product into unitization. Current Markdown/JATS builders reopen the revision and
rebuild the materialization through `verify_markdown` / `verify_jats`; remove that in-process replay.
Validate persisted/external artifacts when they are loaded, not by reproducing the whole pipeline between
adjacent functions.

Keep original source bytes/ranges separate from a bounded path/title/heading/signature retrieval prefix.
Cache expensive model outputs by the exact prepared input, context, role, model/task and relevant options.
Rebind current source evidence separately. A cache hit does not imply that no manifest or Qdrant work is
needed, and a new occurrence must not inherit an old citation.

## 5. Publish one recoverable view

### Native lexical and optional vectors

Use Qdrant 1.19 local `qdrant/bm25` `Document` input for document and query roles. Explicitly bind `k`, `b`,
`avg_len`, tokenizer, case, stemmer, stopwords and ASCII folding to the published sparse-vector profile.
Document and query embeddings are different roles. Keep only bounded useful code-identifier expansion;
it changes term frequency and effective document length.

A profile/tokenization change normally creates a new physical collection generation, fully populates and
verifies it, then switches the published route. Adding a named-vector schema does not populate old points.
In-place vector backfill is an optional scale optimization requiring dual writes, bounded progress,
delete/retirement accounting and explicit completeness before profile switch.

### Use the correct Qdrant mutation

- Full upsert only for a new point or complete authoritative replacement.
- `update_vectors` / `delete_vectors` for selected named vectors.
- `set_payload` merges fields; `delete_payload` removes selected keys.
- `overwrite_payload` replaces the complete authoritative payload while preserving vectors.

Read back the affected names/fields. A timeout after dispatch is an unresolved external effect, not proof
of rollback. Use one absolute deadline across validation, RPCs, pages and readback.

### Pending artifact before pending record

The current `PublicationIntent` stores an opaque `prepared_manifest_ref` while old/new manifests and diff
remain in process memory. Replace it with a real bounded operation artifact written before redb references
it. The artifact contains route/generation/profile, old/new manifest roots, ordered change pages, target
epoch, exact affected IDs/input pages and the next effect/readback cursor.

redb stores only small current/pending metadata and the artifact reference. The daemon then performs
Qdrant work, reads actual state and commits the visible view. It never holds the redb writer across Qdrant
I/O. After visibility, manifest pages become roots of the new/old view as needed; operation-only cursor and
checkpoint pages are removed after recovery and old-view requirements end. No eternal operation archive.

### One current-view guard

Replace arbitrary route/epoch acquisition with one owner holding the current immutable `PublishedView`.
A query clones a non-public guard from the current pointer. A continuation may receive another guard only
from its still-live query guard under the same owner operation. After publication swaps the current pointer,
new queries cannot acquire the old view; its guard count can only decrease.

This removes the race in sampled `PinRegistrySnapshot`/`ReclamationWatermark`: no new old-view pin can
appear after cutover. Remove raw `acquire_route_pin`, old-route acquisition, renewal of arbitrary
continuations and release-by-known-owner APIs. Query windows are process-local and finite; restart may
expire them and replan.

### Reclaim as publication cleanup

Keep the useful exact delete/readback code, but fold its ownership into publication instead of another
reclaimer journal.

- Retired points in the same generation: after every view that can see them drains, delete exact IDs in
  bounded pages and read back absence on the same route/generation.
- Replaced generation: after its last view guard drains, delete the whole old collection/generation.
- Pending operation artifact: retain until its external outcome is resolved.
- View manifests/source revisions/prepared artifacts: retain while current, pending or a live view refers
  to them; then delete exact unreferenced objects.

Store one cleanup cursor with the existing publication artifact. On uncertain deletion, read back and retry;
leaking an object is safer than deleting a referenced object. Do not build generic leases, legal holds,
purge tombstones or a complete object-graph operating system for normal local GC.

### Backup and restore

Baseline backup is offline and current-format only: stop the daemon, resolve/finish or report pending
publication, close redb/object writers and copy one coherent data root. Qdrant is derived and rebuilds.
Restore starts the same current application against a copied root for the same supported storage/secret
profile. Unsupported old formats, live root cutover, re-encryption and cross-user migration are not
baseline features. Original roots remain untouched until the user explicitly removes them.

## 6. Connect the query to source evidence

### One request and scope

Replace V1/V2 parallel request trees with one typed request containing scope, query and real limits. Parse
one bounded frame with Serde; remove tab/hex protocols, substring event parsing, plan fingerprints and
mandatory exact-scan budgets for ordinary ranked search.

Resolve one compatible portfolio into one Qdrant filter population and the same `params.idf.corpus`.
Remove per-membership fan-out and the executor's grouped-plan refusal together. Split only across different
physical routes or incompatible retrieval settings. Use native prefetch/RRF/grouping for Qdrant legs;
keep a small local merge only for real DIRECT/editor-overlay results.

Ordinary top-k is explicitly non-exhaustive. Remove unconditional `count_exact` and
`CompleteCandidateScope`. Count/scroll exist only for operations that request them.

### Candidate to excerpt

Implement the actual lookup:

```text
Qdrant point
→ current view manifest entry
→ source revision / representation / range
→ retained revision reader
→ verified excerpt and location
```

Qdrant payload is not source text. Candidate validation must hash/check actual retained bytes once at a
clear owner; comparing caller-returned digest fields is not validation. Propagate the request deadline and
cancellation through readback.

Select candidates and useful neighboring structural records before body I/O. Group ranges by retained
revision/representation and merge same-basis overlaps. The current range API reads the whole revision and
then copies a slice; share one cancellable verification/read pass across selected ranges where whole-object
integrity requires it. Charge all scanned bytes, not only returned bytes.

Return excerpts in the first result. One short-lived query window keeps ordering, cursor, view guard and
optional opaque expansion locators. Remove the second handle store, catalogue-wide continuation hashes and
records that remain after expiry. The cursor advances; it does not rescan the window prefix each page.

### Exact and optional model depth

Exact literal/regex checking streams the stated retained inventory through the existing matcher. Remove the
second per-offset matcher and corpus-sized preloaded readback vector. Preserve complete/incomplete outcomes.

Optional dense retrieval encodes each distinct query input once and uses a real named-vector input. Optional
reranking reads a finite source-backed subset and only reorders/subsets it. The model worker is used only if
the selected runtime needs process isolation; it does not read the corpus, build a graph, rewrite the query
or generate an answer.

## 7. Donors at the named seams

Use the [donor index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) for pinned source and limits.
The selected mechanisms are:

| Seam | Donor mechanism |
|---|---|
| CLI dispatch | ripgrep: parse once, dispatch one concrete mode. |
| Inventory | `ignore::WalkBuilder` plus raw `notify::RecommendedWatcher` hints. |
| Git | `ls-tree` / `diff-tree` / reused `cat-file --batch-command`; optional `gix` alternative. |
| Control | redb short write transactions and read snapshots. |
| Serialization/config | Serde/serde_json and bounded TOML typed decoding. |
| Qdrant | Existing Rust client, local BM25, filtered IDF, RRF/grouping and narrow mutation APIs. |
| Deadlines | Tokio `timeout_at` with one absolute instant. |
| Structure-aware units | Existing parsers plus Docling `HybridChunker` and Unstructured `by_title` mechanisms. |
| Code facts | `ast-grep outline` or direct Tree-sitter; import actual SCIP/LSP occurrences when available. |
| PDF | `pdfium-render` in the existing optional worker; OCR only missing text pages. |
| View lifetime | Lucene snapshot generation refcounts; Tantivy tracked-file GC. Copy the mechanism, not either backend. |

## 8. Delete replaced mechanisms

Delete with their last production consumers:

- legacy snapshot/sealed daemon harness applications and obsolete loopback/token protocols;
- generic custom sparse/BM25/frozen-corpus engine and qualification records;
- duplicate revision/preparation staging engines and old DIRECT formats;
- duplicate daemon publication machine, bridge operation ledger and caller-created proof receipts;
- raw route/epoch pin acquisition, durable continuation/handle histories and separate reclaimer journal;
- `search-retention` lease/purge/sweep/restore platform, `wave7-lifecycle`, restore composition and their
  module-shape/process fixtures;
- staged migration/cutover/rollback and old-root compatibility requirements;
- committed package maps, digest ledgers, partial Rust import/type analyser and source-shape tests;
- delivery-wave feature ladder after the connected baseline replaces it.

Do not replace deleted code with forwarding modules, copied archive trees, deprecation adapters, fake grants,
equal synthetic UUIDs, “allowed” booleans or another exception registry.

## 9. Implementation order

1. Connect acquired bytes to one retained revision and durable source head.
2. Produce one prepared artifact and structure-first units with stable occurrence incarnations.
3. Publish one complete lexical Qdrant view through real pending/readback/visible effects.
4. Connect CLI query to manifest lookup and retained excerpts.
5. Make one local edit retain unaffected retrieval points and update only changed evidence/input.
6. Replace pins/reclaimer/retention with current-view guards and bounded cleanup.
7. Add selected Git, code, PDF, overlay or model depth through the same owners.
8. Remove remaining legacy mechanisms and obsolete issue/workflow gates.

Each slice must end in a real caller using the new path and the replaced implementation deleted or clearly
scheduled in the same active cutover. Do not count documentation, constructors or closed issues as product
progress.

## 10. Verification

For changed production owners and actual consumers, run the pinned compiler and a minimal Clippy pass, for
example `cargo +1.98.0 check --locked -p <owner>` and the corresponding binary/features. Add focused
behavior checks with the integration, not a broad historical matrix.

Required connected cases:

- retained file survives restart and returns the correct excerpt;
- edit near file start does not rewrite an unrelated suffix;
- changed retrieval input creates a new point while old pinned evidence still opens;
- full upsert and narrow vector/payload mutations preserve unrelated data;
- publication restart before/after every durable/external boundary;
- new-generation switch keeps old readers and reclaims only after the last view guard;
- cancellation/deadline before and after possible external mutation;
- exact scan reports complete/incomplete scope truthfully;
- lost Qdrant index rebuilds from retained source/control state;
- offline backup restores the current format and rebuilds derived index data.

Measure useful-result latency, bytes read, units/points retained/created/retired per edit, model calls avoided,
Qdrant writes and peak resources. Do not claim Windows, live-Qdrant, scale or installed results from static
source review or compilation alone.
