# ELIOT Search — implementation

Read the [architecture manifesto](../architecture/README.md) for purpose and product requirements.
This guide owns what to change, where to change it and which existing implementations to use.
It replaces `PROJECT_COMPLETION`, `IMPLEMENTATION_STATUS` and `EXECUTION_PROTOCOL` as implementation guidance.

**Inspected source:** public `main` at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, 2026-10-18 UTC.
The instructions below are the refactoring target, not a claim that the new application already runs.
Preserve #598's active manager/private work and use its useful held-source result when integrating.
Historical waves and closed issues are neither missing-code blockers nor prerequisites.

## 1. Build one application path

Use the existing CLI, daemon and packages. Do not create another application or parallel framework.

**Ingestion:** selected root → stable read → immutable revision write → source head transaction →
materialization/UnitSet → bounded Qdrant write/readback → published view.

**Query:** typed request → resolve scope and pin published view → local Qdrant BM25/query →
candidates → manifest/unit lookup → retained-byte read and validation → locations/snippets.

A normal edit processes changed content. Index loss or a changed derived format rebuilds a generation.
DIRECT remains an explicit exact/source scan, not an indexed-success fallback.

### Preserve the useful requirements

The [historical master](../architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md) contains useful
workloads, not a package/dependency constitution. Implement them through the same path:

| Requirement retained from the old design | Small implementation |
|---|---|
| Local standalone use, S0–S2 | CLI/daemon and local Qdrant; ordinary text search requires no model service. |
| Large repositories and portfolios, S7.8/S16 | Selected roots and explicit commit/index/worktree views; one inventory/update scheduler with bounded batches. Portfolio is a query scope, not another copy of every repository. |
| Exact identifiers and explicit completeness, S12/S25 | Ranked lexical navigation plus the existing literal/regex exact scan over a stated retained inventory. Top-k never supplies negative proof. |
| Current incremental evidence, S13–S16 | Retain changed bytes, update affected units/points, publish one view; watcher hints accelerate reconciliation. |
| Orientation, subjects and comparison, S21–S22 | Locations and syntax outlines first; import actual semantic occurrences when available. Compare source-backed implementations and show missing facts. |
| Document coordinates, S17/S24 | Existing Markdown/JATS plus selected native PDF extraction. Page/text ranges remain useful without invented raw-PDF byte offsets. |
| Saved/unsaved edits, S18 | Saved edits enter the normal update queue; an explicitly selected editor overlay shadows matching sources in memory. Do not persist every keystroke or index it twice. |
| Rebuild and recovery, S28 | Keep source/control and unfinished publication; rebuild a lost derived index. Optional backup copies a coherent control/source snapshot, not an unbounded operation history. |
| Responsiveness, S30 | Interactive work has priority; use bounded batches, cancellation and one deadline. Optional parsing/models run in background only when selected. |

Old latency numbers are targets, not achieved performance. Measure useful result latency, bytes read and
points rewritten per edit, model inputs/calls actually avoided and peak resources. A short response or
extraction-cache hit does not bound earlier traversal, embedding or publication work.

### Existing owners and concrete work

| Responsibility | Existing location | Implementation result |
|---|---|---|
| Invocation and client | [CLI entry](../../bins/eliot-search/src/entry.rs), [public client](../../bins/eliot-search/src/public_client.rs), [daemon entry](../../bins/eliot-searchd/src/entry.rs), `search-config` | Parse once, use one command/config value, connect the actual local request to its handler. |
| Source acquisition | [native source](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs), [command](../../bins/eliot-searchd/src/owner_composition/kernel/native_source_command.rs), `search-safe-reader` | Pass the bytes already read to the revision writer; finish ingestion instead of returning admission metadata alone. |
| Retained objects | [immutable object I/O](../../crates/search-source/search-revision-store/src/immutable_object.rs), [current writer](../../bins/eliot-searchd/src/secure_revision_writer.rs), `search-revision-store` | One persistent writer/reader for the new format; source identity/head semantics remain with the source owner. |
| Preparation | [materializer pipeline](../../crates/search-prep/search-materializer/src/product/pipeline.rs), [UnitSet builder](../../crates/search-prep/search-unitizer/src/manifest/build.rs), `search-lexical` | Prepare retained bytes once; keep coordinates and useful identifier expansion. Use native BM25 instead of maintaining a second generic sparse encoder. |
| Durable control | [persistent control](../../crates/search-control-redb/src/persistent.rs), [transactions](../../crates/search-control-redb/src/persistent/transaction.rs), [publication records](../../crates/search-control-redb/src/persistent/publication.rs) | Typed current records and short redb transactions; unfinished external effects have one pending publication record. |
| Publication | [shared coordinator](../../crates/search-index-qdrant/search-publication/src/machine.rs), [daemon composition](../../bins/eliot-searchd/src/publication_composition.rs), `search-projection-planner` | Keep one publication transition owner; daemon performs real I/O. Fold useful daemon `Publisher` behavior into this path and remove its duplicate floor/journal machine. |
| Qdrant | [real bridge](../../crates/search-index-qdrant/search-qdrant-bridge/src/real.rs), [blocking runtime](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) | One private SDK translation/connection, actual writes/queries/readback, no second operation journal. |
| Query and results | [public dispatch](../../bins/eliot-searchd/src/public_runtime_service/kernel/dispatch.rs), [query composition](../../bins/eliot-searchd/src/query_composition.rs), `search-query-planner`, `search-retrieval-executor`, `search-candidate-validator` | One concrete handler completes the query and returns source-backed results. Change its incompatible inputs/outputs together. |

These are current code locations, not a mandate to keep every package. A useful owner and its immediate
consumers may change in one implementation. Avoid disconnected constructor/contract-only successors.

### Construct from real settings

Capture one invocation, decode/merge supported file/env/CLI values, validate actual fields and construct
owners with those settings. Current `effective_from_process` compares startup against defaults and
calls `try_activate` with no receipts: even permitted `query.limit=11` gets
`DAEMON_CONFIG_PARTIAL_REFUSED`. Remove that startup gate, not just its failing test. Connect the value
to the real handler; the handler must actually use it.

Use typed settings with the locked TOML/Serde decoder; remove generic config provenance hashes and
mandatory secret/reload machinery. A running owner supports concrete live changes or requires restart;
indexed profile changes use publication. Status reads the running settings. The
[config change map](../../crates/search-config/FUNCTIONS.md) covers actual callers, OS-string capture
and bounded file reading; no second config journal or acquisition framework is needed.

## 2. Finish source retention and preparation

`FreshRootContext::acquire_source_v2` already calls `acquire_with_cancel_v2`, obtains the body,
hashes/classifies it and then returns `AcquiredNativeSource` without the body. Change that handoff to
transfer the same verified bytes and metadata directly to the immutable writer. Do not reread a pathname
and pretend the second contents belong to the first observation.

The native command currently records admission and reports `retained_revision:false`.
`--sync-source-roots` reaches
[run_source_sync_preflight](../../bins/eliot-searchd/src/owner_composition/kernel/source_sync.rs), which
ends in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE` even after successful checks. Replace this refusal-only
custody/callback sequence with actual sync; do not add another admission stage.

`SourceRootCatalog::refresh` probes root availability; it does not enumerate or reconcile source contents.
Use one `notify::RecommendedWatcher` for hints and a shared dirty-path/scan queue. `ignore::WalkBuilder`
supplies traversal and explicit ignore behavior; feed bounded batches to the source owner. Startup,
resume, missed events and periodic checks use that same scan path. Clear dirty state only after a completed
scan/update; an inaccessible directory is not evidence that all its files were deleted. Retain hints that
arrive during a scan for the next update. Do not add `PollWatcher` beside an existing periodic inventory
scan or a supervisor/receipt log per root. The
[reconciliation map](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) names actual gaps.

For the first Git commit view, use controlled Git plumbing: one tree listing, `diff-tree` for changed
entries and a reused `cat-file --batch-command` child for raw blobs. Resolve refs once to immutable OIDs;
do not read unchanged blobs or run checkout, textconv, filters or lazy fetch. This replaces the old blanket
ban on the Git executable. `gix-diff`/`gix-odb` are an alternative for a selected no-Git installation,
not a second mandatory backend. See the [Git change map](../../crates/search-source/search-safe-reader/FUNCTIONS.md).

Use one immutable-byte writer/reader built from the existing `search-revision-store` mechanics.
Materializer's `legacy_artifact` contains a second staging/write/sync/link/readback engine; retire it
with its daemon preparation callers. Current plaintext preparation rereads the new object in staging,
after installation and again in the daemon. Keep one useful verified readback and move its buffer/range
forward. Protected-storage decoding is a separate real effect when that optional mode is selected.
The [revision-store](../../crates/search-source/search-revision-store/FUNCTIONS.md) and
[materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md) maps identify these paths.

The in-memory `RevisionStore` and source-history's `register_source_revision` are not a production
persistent backend. Old DIRECT mutation methods and public-service mutation dispatch are test-only;
removing their `cfg(test)` without connecting the new owners does not finish ingestion.

Keep only data that the application uses:

| Record/artifact | Purpose |
|---|---|
| Format/settings and registered roots | Open the current format and know the selected sources. |
| Source ID, locator, current revision, availability | Incremental update, rename and removal from current search. |
| Revision ID, exact content digest/length, object locator | Reopen the bytes that produced evidence. |
| Prepared artifact with revision/profile and coordinates, or a gap | Reuse preparation and rebuild it after a profile change. Text/maps remain files, not a second corpus inside redb. |
| Published generation/epoch and pending external publication | Consistent queries and restart recovery of actual unfinished work. |

Write the object before committing its revision/source-head reference. A crash before that reference
leaves an unreferenced object; a crash after it leaves a revision that preparation can retry. No durable
`PreReadQualified`, `ReceiptIssued` or `BodyServingReady` record is needed for each intermediate step.

Pick one source/content-revision model across this path. A source-record revision is not a content
revision. Native UUIDs, old DIRECT hex revisions and materializer `OpaqueId + NonZeroRevision` are not
interchangeable casts. Change the real producer and preparation/UnitSet consumers; do not build a
universal legacy converter.

Call the existing materializer and pass its owned prepared product into unit construction. Current
Markdown/JATS UnitSet builders invoke `verify_markdown` / `verify_jats`, which reopen and rebuild the
whole materialization before constructing raw and structural units. Remove this in-process full replay and its receipt
parameters. Validate loaded bytes/artifacts at their real storage boundary; retain source coordinates
and transformation checks, not repeated production of the same result.

The raw UnitSet ingress currently requires byte-identical UTF-8, `ExactBytes`, an empty loss map and
equal input/canonical digests. Broader decoding alone does not support transformed UnitSets. Change
that producer/consumer together, reusing actual decode/normalization/coordinate code. Start with
supported text/code profiles and report unsupported input. Old `--prepare-*` helpers are donor code,
not a reason to require their storage layout.

### Add useful code and document depth

For code orientation, replace the line/brace parser in `search-code-enricher` with one syntax producer:
selected `ast-grep outline` over bounded immutable-file batches, or direct Tree-sitter for a concrete
embedding need. Cache facts by retained revision and extractor settings. Import SCIP/LSP occurrences
only with the indexed snapshot/document version; never attach stale facts by pathname. The inspected
rust-analyzer SCIP writer has no symbol relationships, so it cannot supply an implementation/call graph.
Use [code-enricher's map](../../crates/search-prep/search-code-enricher/FUNCTIONS.md) for exact APIs/limits.
Resolver/comparator currently consume prepared observations in tests; connect real facts and source
ranges rather than inventing behavior proofs or another graph database.

Use those prepared records as a cheap source guide: path/title, format/language, headings, definitions
and their locations. Orientation groups selected records and suggests a few exact next reads; it does
not generate a hierarchy of corpus summaries. The Markdown preparation libraries already
[build section parents and observed link destinations](../../crates/search-prep/search-unitizer/src/markdown_unitset/build.rs);
their production wiring and source-guide reader are still missing. Resolve selected local links/symbols
within the pinned scope, preserving direction, source occurrence and missing facts. The current Markdown
reference set is potentially incomplete. Matching names nominate comparisons, not one merged entity.

Keep retrieval input separate from quoted source. A bounded title/heading/signature prefix can help an
isolated chunk make sense to lexical or selected model retrieval; it is derived context, not bytes at
the chunk's source range. Use existing prepared artifacts and source maps. A model-generated summary
is an optional derived artifact with its actual inputs/settings, never an indexing prerequisite.

Reuse expensive model outputs by exact input and the context/model/task/options actually used. Current
`UnitId` includes revision/provenance and coordinates; an unchanged chunk in an edited file can receive
a new occurrence ID. Rebind source/revision/range separately. A changed heading or full-document input
invalidates context-dependent reuse even if the central chunk is unchanged. Reused inference may still
need new occurrence-bearing points; measure computation and publication separately. The
[unitizer map](../../crates/search-prep/search-unitizer/FUNCTIONS.md#context-and-reuse) and
[model map](../../crates/search-model-provider/FUNCTIONS.md) describe the actual changes.

For the first PDF, use `pdfium-render` in the existing optional doc worker and return page/text locations.
Its current binary only exits unavailable. Finish the real parser → materializer/UnitSet → query result
in #227; a generic echo-worker supervisor is not the deliverable. OCR only missing usable page text;
layout/table models and GROBID scientific structure are selected additions. Xberg/Docling are useful
alternative extractors/artifact donors, not a reason to start every parser/service. The
[document map](../../bins/eliot-search-doc-worker/FUNCTIONS.md) records APIs and coordinate limits;
[optional settings](../../config/sections/optional_profiles.md) replaces the old qualification toggles.

## 3. Keep control state small and publication real

Use the existing `search-control-redb` owner with typed methods for source/head, settings and publication.
Within a write transaction, read needed conditions, change the relevant tables and commit. Use read
transactions for snapshots. Do not hold the single redb writer while waiting for Qdrant.

Current `execute_scoped` builds a read-phase plan/fingerprint, rechecks under write, saves an operation
receipt and rereads it after commit. Its permanent `OPERATIONS` history is never pruned; baseline limits
are 65,536 operations and 64 MiB of operation bytes. New writes eventually fail with
`IdempotencyCapacityExceeded` even when current records are small. Remove the lifetime replay archive
for ordinary local updates. Keep only unfinished work and, where a client can genuinely retry a mutation,
a bounded retry window owned by that operation.

A successful abort before commit is cancellation/failure, not an unknown commit. An error from commit
or a possible Qdrant write may need recovery. Do not propagate the latter uncertainty to every read or
pure function. Retire generic fingerprints, `ControlCommitReceipt` and port receipts whose only consumer
is the removed journal.

Use one publisher: the shared publication transition logic, called by daemon composition, writes a
pending intent through the real persistent owner, performs bounded Qdrant mutations/readback, then
commits the visible route/epoch. Simplify that coordinator with its callers: keep pending/visible state
and actual effect outcomes, not every existing receipt argument or intermediate phase. Preserve needed
incremental validity intervals and live pins.
Qdrant sparse retrieval can search its mutable index after a completed write; background optimization
and a corpus count are not publication prerequisites. An accepted asynchronous operation is not yet a
completed write. Sparse dot-product exactness does not establish exact source matching or absence.
The [old daemon Publisher](../../bins/eliot-searchd/src/publication_composition/kernel/publisher.rs)
duplicates state; its [publication_codec::cas](../../crates/search-control-redb/src/publication_codec.rs)
only computes/checks a new floor, not a redb commit. Remove that parallel machine and file journal.

The prepared publication plan remains a discardable local result until the pending intent is durably written.
Current `submit` takes the coordinator's active slot in `Prepared`, but `abandon` and compensation
cannot clear that phase. A cancelled pre-intent task can leave the same coordinator permanently busy.
Remove that unnecessary reservation/state or provide its ordinary local discard with the actual caller.

Preserve uploadable input alongside planned identity/payload. For native BM25 this is bounded prepared
text and actual encoder options; the current planner requires precomputed vectors/digests and `PointSpec`
does not carry uploadable values. Change preparation/planner/bridge together to accept native inference
input. Check the returned named vector and point/source identity; do not compute BM25 a second time merely
to manufacture the old expected hash. A selected external encoder still supplies its actual values. Use
bounded batches and manifest pages through the existing artifact owner. Current manifest CBOR
limits one membership to 4,096 entries despite the planner's 100,000-point budget; `prepared_digest`
also copies old/new manifest bytes into one 8 MiB canonical object. Remove that whole-object dependency
and diff ordered entries incrementally; increasing one quota does not make the path scale.

Reclamation reads exact retired IDs from committed control/manifest state. The current
[validator](../../crates/search-index-qdrant/search-index-reclaimer/src/manifest.rs) only compares supplied
digest/proof fields, not their binding to `point_ids` or an actual commit. Remove the duplicate proof
object; do not replace it with another attestation. Admit/release query pins through the same guarded
view lifecycle, so no unsafe old pin can appear after reclamation is authorized. Remove released pin
records, not just their live marker. The [pin](../../crates/search-index-qdrant/search-epoch-pins/FUNCTIONS.md)
and [reclaimer](../../crates/search-index-qdrant/search-index-reclaimer/FUNCTIONS.md) maps show the current gaps.

On restart, inspect pending intent and actual stored/indexed state before resuming. Alias movement or
a supplied commit observation is not the durable boundary. Reuse one bounded delete/readback operation
with cancellation/deadline; never infer rollback of a possible external write from a timeout.

## 4. Simplify and connect the query

### One request model and one dispatch

Reuse the useful scope/question/cards structures in
[search-contracts::evidence](../../crates/search-contracts/src/evidence.rs), changing the actual model.
There is no requirement to preserve V1 bytes or introduce V3 beside the existing V1/V2 trees.

Current `validate_global_budget` requires nonzero exact-scan budgets even for retrieval/orientation.
Remove that requirement from ordinary queries. Remove mandatory plan IDs/fingerprints, result fingerprints,
ranking traces, hashed group IDs and persistent handles when no selected consumer needs them.
Keep explicit scope, query, real limits and useful result data: source/revision, location/range, excerpt,
freshness and gaps. A continuation can be a bounded in-memory window; restart may expire it.

The existing recipe registry consumes `RecipeIdV1`, while #213 uses V2 evidence forms. Update decoder,
dispatch, handler and result encoding together. A new registry entry alone cannot connect them.
Replace the current public client's tab/hex and substring event parsing with one typed framed exchange.
Use the owned child-stdio route first; a persistent owner-only local pipe is an extension of the same
application where needed. Mandatory MAC/pairing/grants are removed from trusted-local constructors,
not filled with synthetic values. Restricted-client features enforce real restrictions separately.
The [protocol settings reference](../../config/sections/protocol.md) identifies another missing connection:
`pairing_required` is a documented/example field, not a setting consumed by the current daemon registry.
Change the actual settings/session/callers; setting it to `false` does not implement this local mode.

### Direct planning and execution

Change the existing query planner to accept resolved scope, pinned published view, actual profile,
query and finite limits. `compile_canonical_plan` currently requires an externally supplied task plan,
grant, request/snapshot/plan fingerprints and capability dependency evidence. The lexical path needs
query encoding, retrieval and validation, not a separately manufactured set of proof documents.

Use Qdrant 1.19's local `qdrant/bm25` through the existing SDK `Document` input for ingestion and query.
The pinned server handles it without a remote inference address or model worker. Use the same explicit
tokenization, case, stopword/stemmer and BM25 settings at both ends; neutral/code input must not silently
inherit English stemming and stopwords. Keep a small code identifier expansion only for useful behavior.
Retire the generic custom sparse hashing/BM25/frozen-corpus and proof-profile path with its consumers;
rebuild derived vectors rather than preserve its bytes. Exact literal/regex/Unicode semantics remain in
their actual source-verification owners. [Lexical implementation](../../crates/search-lexical/FUNCTIONS.md)
and the [donor index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) record concrete options/source.

Use native prefetch/RRF/grouping when the contributing legs are Qdrant queries. Keep a small local merge
only for actual direct/overlay results; no parallel ranking framework or old score compatibility.
Use the same published evidence identity across contributing legs, including its actual scope/revision
and route semantics. Current candidate IDs include `leg_id`, preventing fusion of the same point from
two legs. Carry the final fused order through validation/projection: the projector currently sorts
again by raw score, which can destroy that order. Neither fix needs a ranking-proof object.

`execute_pinned_leg` currently takes `ProcessTestIndexedPort`. Connect the real existing
`IndexedRetrievalPort` to `BlockingRealQueryPlane` with a small private adapter, or call the executor
directly from the handler. Do not add another runtime, factory or generic query host.

Resolve one compatible published scope into one filter population. Current `search-access` emits a leg
per source membership without `OverlapFreeRouteProof`; its grouped alternative is then refused by an
executor that accepts only one eligibility plan. Remove that artificial fan-out/denial with the actual
callers. The existing bridge already constructs membership unions. Split only across genuinely distinct
routes or incompatible retrieval settings, not to satisfy a proof type.

In the existing Qdrant bridge:

- use one shared identity/payload/eligibility model; change it with planner and bridge when removing
  unused coordinates. The current 20 fields / 19 indexes are not a target to preserve; index fields
  actually used by scope/filter/group queries. Never fill missing identities by renaming old fields;
- remove `QualifiedGate` test-report inputs from connect/callers; keep actual endpoint/owner binding,
  authentication, server capability and schema checks;
- remove the process-local mutation-receipt ledger as a second replay owner; publication owns retries
  and unknown external outcomes;
- convert the admitted deadline once to an absolute instant and propagate it through RPCs/pages;
- remove ordinary top-k's unconditional `count_exact` and change `IndexedLegOutput`/`LegCompletion`
  with it. Do not leave `CompleteCandidateScope` or invent a denominator.

For one local node, one replica and one publisher awaiting each completed write, use default/weak
ordering; a second distributed serialization policy adds no needed order. Change the current hardcoded
strong setting with its consumers. Strict mode is a chosen cost/limit guard, not source correctness.
These choices do not imply the same policy for concurrent writers or replicated deployments.

The shared field-level [bridge map](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md)
explains current payload/index/vector mismatches. Use the resolved eligibility in both retrieval and
`params.idf.corpus`; the retrieval filter alone does not scope IDF. Keep one shard for the initial local
corpus. Native IDF counts physical points with the sparse vector, not distinct source documents or result
groups, and scans the selected corpus/postings. Grouping cannot repair duplicate scoring populations.
Do not index another membership copy merely to place a repository in a second portfolio; change the
existing projection/eligibility relationship where it forces that duplication. Retain a real distinct
projection only when its content/profile/view differs. No custom global IDF service is needed.

Explicit exact checking reads the retained inventory in bounded batches and uses the existing Aho-Corasick
literal/regex owners. The separate `proof.rs` path still has a second per-offset literal matcher and takes
an already accumulated `Vec<ExactItemReadback>`. Remove that duplicate matcher and pre-buffering;
preserve complete/incomplete/invalidated outcomes and one stated case/range semantics. See the
[exact map](../../crates/search-query/search-exact/FUNCTIONS.md); a count or empty top-k cannot replace it.

### Turn a point into evidence

Implement the real lookup: point ID → published manifest entry → unit/representation coordinates →
retained revision. Executor nominations alone lack the data required by the candidate validator.
Use actual shared manifest/source records, never fixture IDs, source-record version numbers substituted
for content revisions or Qdrant payload treated as source text.

[reopen_and_verify](../../crates/search-query/search-candidate-validator/src/lib.rs) currently compares
digest fields returned by `RevisionReadbackPort`; it does not hash `readback.bytes`. The real retained
reader must verify actual bytes and the range, or move that verification into the existing validator.
Do it once at a clear owner. Returning the expected digest from the request beside arbitrary bytes is
not source validation. Remove redundant residency/proof fields together with their consumers, while
preserving the actual revision/byte/coordinate connection.
The readback trait also lacks explicit deadline/cancellation inputs. Connect the request context through
the real reader; a byte ceiling alone does not bound waiting on I/O.

Select the candidate page and useful neighboring sections/definitions before reading their bodies.
Bound visited relations and source reads as well as returned context; a high-degree node must not cause
an unrestricted graph scan. Pass remaining item/byte limits and the deadline into those lookups/reads;
do not fetch everything and truncate afterward. Group selected ranges by retained revision and
representation, merge same-basis overlaps, and preserve each result's locator and final rank.

Current public expansion reaches
[`read_revision_range`](../../bins/eliot-searchd/src/secure_direct_store/kernel/read.rs), which loads the
whole revision through `read_revision_detailed` and then copies the requested slice. The
[projector](../../crates/search-query/search-result-projector/FUNCTIONS.md) also receives all verified
bytes before applying its budget and emits handles without those excerpts. Change both consumers to
share the actual read result. Where the source owner requires whole-object verification, share one
cancellable streaming pass across selected ranges; a seek alone cannot verify a whole-object digest.
Count all scanned bytes, including discarded ones, against actual read work and report unfinished reads
when the budget expires. This does not require rehashing every object for every excerpt. Return verified
excerpts immediately and retain only bounded useful excerpts in the same query window. Do not add a
second context service, full-corpus buffer or range-proof tree.

Optional semantic retrieval/reranking uses this same query. `search-model-provider` is documentation
only, and its worker exits unavailable; planner leg flags are not model integration. Select a concrete
runtime, encode each distinct query input once and rerank a finite subset of already read snippets.
The real `query_filtered` interface is sparse-only: change its actual producer/bridge for dense inputs
instead of casting vectors or adding another backend. [Model implementation](../../crates/search-model-provider/FUNCTIONS.md)
owns those changes. The ordinary request does not rewrite itself with an LLM or generate a research answer.

### One short-lived result window

Reuse the daemon DIRECT cursor and commit-after-delivery behavior in one query window owner. Its
current public `cmd_continue` still does three complete `list_sources` traversals per page through
continuation fences and handle minting. Replace those catalogue hashes with the actual pinned view
and direct source/revision lookups needed for that page. Shared continuation's repeated scan from the
window's start also disappears; a cursor advances through the retained ordering.

Return source/revision/range directly. An optional opaque expansion locator references the same window
item, not a second handle store. Bound window count/bytes and TTL; remove terminal records. Current
shared `HandleStore` expires flags while retaining records/indexes and quotas count only Active entries.
Do not preserve that unlimited history. Finishing paging must not delete backing items still needed
by issued live locators. See the [single-owner map](../../crates/search-query/search-continuation/README.md#one-temporary-owner).

## 5. Use these donors at the named seams

These are mechanisms, not whole applications to import. Versions describe the inspected/locked source;
there is no dependency-upgrade task here. The [context donor comparison](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#context-source-guides-and-model-work)
covers EdgeQuake, LightRAG, GraphRAG, NotebookLM, Cloudflare AI Search and contextual retrieval.

| Eliot seam | Donor/API | What disappears / limit |
|---|---|---|
| Local control | [redb 2.6.3](https://github.com/cberner/redb/blob/v2.6.3/README.md): `begin_write`, tables, `commit`; `begin_read` | Generic per-update transaction/receipt/replay platform. redb does not make files and Qdrant one transaction. |
| Request/result/events | [Serde 1.0.229](https://github.com/serde-rs/serde/blob/v1.0.229/README.md#serde-in-action), [serde_json 1.0.151](https://github.com/serde-rs/json/blob/v1.0.151/src/de.rs): derives, `from_slice`, `to_writer` | Manual JSON grammars/escaping/fixed field order and substring parsing. Add direct dependencies to the existing owners: presence in Cargo.lock is not integration. Bound the frame first; do not wait for EOF with `from_reader` on a live connection. |
| Invocation | [ripgrep 14.1.1 dispatch](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs): one parsed invocation → concrete mode | Repeated `args_os`, `strip_config_args_os`, `maybe_run` and argv fingerprints. Preserve `OsString` paths; no new invocation crate. |
| Indexed I/O | [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs): `upsert_points`, `get_points`; `query` | Parallel SDK types/operation owners. Produce batches incrementally; a chunk helper that takes the whole corpus vector is not streaming ingestion. |
| Settings | [toml 1.1.7](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/mod.rs): `from_slice` / `from_str` into Serde structs | Custom event receiver, double string decoding and generic config trees. It allocates tokens/events/tree before typed output: bound input first, retain the default recursion guard and validate real fields. Already locked for xtask; production integration remains. |
| Request time | [Tokio 1.53.1](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs): `timeout_at` | Renewed per-RPC durations and another budget framework. Keep concrete byte/queue/candidate/output limits. |
| Snapshot organisation | [Tantivy 0.22.0 example](https://github.com/quickwit-oss/tantivy/blob/17d5869ad61ea9f1072ef40546ddcada3a14b067/examples/basic_search.rs): writer/commit/query snapshot | A design reference for coherent view ownership, not a second backend or a ready-made cross-store recovery solution. |

Existing `globset`, `aho-corasick`, regex, Unicode, Markdown/XML and shared hash owners already serve
real callers. Do not reimplement or “adopt” them again. Hash exact source/artifact bytes where their
identity/integrity is needed; ordinary JSON records need no canonical hash. For actual Git, code-fact or
document extensions, use the [donor index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) only when
that capability is selected. Do not turn future profiles into today's framework.

## 6. Delete replaced mechanisms and consolidate

- Retire general legacy staging/cutover/rollback and old-format readers from the new application's
  required path. An unsupported old root gets a clear error before writes and remains intact. New
  roots/generations rebuild from available sources. No compatibility shim, export implementation or
  migration issue must precede the new product.
- Remove mandatory six-domain storage, provider ownership transfer, unused DEK/secret lifecycle,
  executable-hash owner succession and the old writer's co-lock when that writer is retired.
  New local storage has one concrete writer/reader; optional encryption is not its default dependency.
- Remove obsolete daemon/CLI applications, loopback/token-file protocols and their exclusive fixtures.
  Preserve the useful DIRECT exact behavior through the current application, not an entire legacy app.
- Remove per-site digest ledgers, map/status gates, partial Rust import/type analysis and source-shape
  tests with their required consumers. Use Cargo metadata, Rust visibility and actual behavior checks.
- Drop the delivery-wave feature ladder. The supported default contains the real baseline; optional
  features represent selected dependencies. A feature flip must not expose an unconnected handler.

After deleting mechanisms, combine packages that merely divide one responsibility: source records/
history/provenance around one source model; materializer/unitizer around one prepared product; query
planning/validation/projection/paging around one operation and window. Family `AGENTS.md` and README
files now describe this consolidation, not one permanent crate per agent. Keep OS/vendor adapters and
isolated document workers separate where dependencies require it. Shared types need multiple real
consumers; pure functions need no port/receipt.

Do not satisfy old constructors using equal synthetic domain IDs, “allowed” booleans, fake grants or
fixture digests. Remove the obsolete field/check and change its producers/consumers. No forwarding
module, copied archive tree, deprecation train or formal exception registry replaces the deleted code.

## 7. Verification and delivery

The first connected result is a real local file retained across restart, prepared, indexed in Qdrant,
found from the CLI and reopened as the correct excerpt. Then exercise an edit/incremental publication,
restart, lost index, excluded scope, missing revision and cancellation through the same application.
Normal top-k does not claim completeness. A timeout after a possible write leaves recoverable pending work.

Use the pinned compiler/Clippy for changed production owners and actual consumers, for example
`cargo +1.98.0 check --locked -p <owner> --lib`; choose real binary/features where applicable.
Run focused behavior cases after integration. Source-shape tests and obsolete all-feature harnesses are
not merge requirements. Do not claim native, live-Qdrant or scale results from mocks or compilation.

Check that completed local updates can continue after a bounded retry window fills: completed receipt
history must not become a lifetime write limit. Test real bytes and publication state, not a proof object's
constructor. Existing originals/sole retained copies remain untouched when an old format is unsupported.

Ship capabilities that work. Add selected orientation/comparison, Git, document, overlay or semantic
depth through these same owners; they do not all precede a useful release. Describe the result, removed
mechanisms, meaningful checks and actual remaining limitations in the existing PR. No new task taxonomy,
compatibility programme, LOC target or qualification-token process is needed.
