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

**Query:** typed request → resolve scope and pin published view → encode with its lexical profile →
Qdrant candidates → manifest/unit lookup → retained-byte read and validation → locations/snippets.

A normal edit processes changed content. Index loss or a changed derived format rebuilds a generation.
DIRECT remains an explicit exact/source scan, not an indexed-success fallback.

### Existing owners and concrete work

| Responsibility | Existing location | Implementation result |
|---|---|---|
| Invocation and client | [CLI entry](../../bins/eliot-search/src/entry.rs), [public client](../../bins/eliot-search/src/public_client.rs), [daemon entry](../../bins/eliot-searchd/src/entry.rs), `search-config` | Parse once, use one command/config value, connect the actual local request to its handler. |
| Source acquisition | [native source](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs), [command](../../bins/eliot-searchd/src/owner_composition/kernel/native_source_command.rs), `search-safe-reader` | Pass the bytes already read to the revision writer; finish ingestion instead of returning admission metadata alone. |
| Retained objects | [immutable object I/O](../../crates/search-source/search-revision-store/src/immutable_object.rs), [current writer](../../bins/eliot-searchd/src/secure_revision_writer.rs), `search-revision-store` | One persistent writer/reader for the new format; source identity/head semantics remain with the source owner. |
| Preparation | [materializer pipeline](../../crates/search-prep/search-materializer/src/product/pipeline.rs), [UnitSet builder](../../crates/search-prep/search-unitizer/src/manifest/build.rs), `search-lexical` | Prepare retained bytes once, preserve source coordinates and feed the existing projection planner. |
| Durable control | [persistent control](../../crates/search-control-redb/src/persistent.rs), [transactions](../../crates/search-control-redb/src/persistent/transaction.rs), [publication records](../../crates/search-control-redb/src/persistent/publication.rs) | Typed current records and short redb transactions; unfinished external effects have one pending publication record. |
| Publication | [shared coordinator](../../crates/search-index-qdrant/search-publication/src/machine.rs), [daemon composition](../../bins/eliot-searchd/src/publication_composition.rs), `search-projection-planner` | Keep one publication transition owner; daemon performs real I/O. Fold useful daemon `Publisher` behavior into this path and remove its duplicate floor/journal machine. |
| Qdrant | [real bridge](../../crates/search-index-qdrant/search-qdrant-bridge/src/real.rs), [blocking runtime](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) | One private SDK translation/connection, actual writes/queries/readback, no second operation journal. |
| Query and results | [public dispatch](../../bins/eliot-searchd/src/public_runtime_service/kernel/dispatch.rs), [query composition](../../bins/eliot-searchd/src/query_composition.rs), `search-query-planner`, `search-retrieval-executor`, `search-candidate-validator` | One concrete handler completes the query and returns source-backed results. Change its incompatible inputs/outputs together. |

These are current code locations, not a mandate to keep every package. A useful owner and its immediate
consumers may change in one implementation. Avoid disconnected constructor/contract-only successors.

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

Use the atomic immutable-file mechanics already in `search-revision-store`. Its in-memory
`RevisionStore` and source-history's `register_source_revision` are not a production persistent backend.
The old DIRECT mutation methods and public-service mutation dispatch are test-only; removing their
`cfg(test)` without connecting the new owners does not finish ingestion.

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

Call `materialize_text_or_code`, then `prepare_unit_set_input` / `build_unit_manifest`.
The current UnitSet ingress accepts byte-identical UTF-8 with `ExactBytes`, an empty loss map and equal
input/canonical digests. Broader materializer decoding does not make every transformed output admissible.
Connect their retained-byte input to this reader or simplify the existing input API. Reuse the actual
decode/normalization/coordinate code. Start with the supported text/code profiles; expose unsupported
input honestly. Existing standalone `--prepare-*` code for old retained revisions is donor code, not
a reason to require that old storage layout.

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
The [old daemon Publisher](../../bins/eliot-searchd/src/publication_composition/kernel/publisher.rs)
duplicates state; its [publication_codec::cas](../../crates/search-control-redb/src/publication_codec.rs)
only computes/checks a new floor, not a redb commit. Remove that parallel machine and file journal.

Do not treat a supplied commit observation, alias movement or mock receipt as durable publication.
On restart, inspect pending intent and actual stored/indexed state before resuming. Delete unreferenced
staging through the same owner; never infer rollback of a possible external write from a timeout.

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

Use the existing `search-lexical` frozen profiles with `encode_query` / `encode_code_query`.
The profile must match published vectors, not merely a profile name supplied by the client.
Keep useful leg/fusion logic when multiple real retrieval modes use it; call pure transformations directly.

`execute_pinned_leg` currently takes `ProcessTestIndexedPort`. Connect the real existing
`IndexedRetrievalPort` to `BlockingRealQueryPlane` with a small private adapter, or call the executor
directly from the handler. Do not add another runtime, factory or generic query host.

In the existing Qdrant bridge:

- consume the shared S11/payload/eligibility/manifest contracts; the old nine-field payload cannot supply
  missing UUIDs or coordinates by renaming fields;
- remove `QualifiedGate` test-report inputs from connect/callers; keep actual endpoint/owner binding,
  authentication, server capability and schema checks;
- remove the process-local mutation-receipt ledger as a second replay owner; publication owns retries
  and unknown external outcomes;
- convert the admitted deadline once to an absolute instant and propagate it through RPCs/pages;
- remove ordinary top-k's unconditional `count_exact` and change `IndexedLegOutput`/`LegCompletion`
  with it. Do not leave `CompleteCandidateScope` or invent a denominator.

The shared field-level [bridge map](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md)
explains current payload/index/vector mismatches. Reuse its real data obligations; do not restore old
process/compatibility requirements from it. Search and IDF use the same resolved eligibility population.

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

## 5. Use these donors at the named seams

These are mechanisms, not whole applications to import. Versions describe the inspected/locked source;
there is no dependency-upgrade task here.

| Eliot seam | Donor/API | What disappears / limit |
|---|---|---|
| Local control | [redb 2.6.3](https://github.com/cberner/redb/blob/v2.6.3/README.md): `begin_write`, tables, `commit`; `begin_read` | Generic per-update transaction/receipt/replay platform. redb does not make files and Qdrant one transaction. |
| Request/result/events | [Serde 1.0.229](https://github.com/serde-rs/serde/blob/v1.0.229/README.md#serde-in-action), [serde_json 1.0.151](https://github.com/serde-rs/json/blob/v1.0.151/src/de.rs): derives, `from_slice`, `to_writer` | Manual JSON grammars/escaping/fixed field order and substring parsing. Add direct dependencies to the existing owners: presence in Cargo.lock is not integration. Bound the frame first; do not wait for EOF with `from_reader` on a live connection. |
| Invocation | [ripgrep 14.1.1 dispatch](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs): one parsed invocation → concrete mode | Repeated `args_os`, `strip_config_args_os`, `maybe_run` and argv fingerprints. Preserve `OsString` paths; no new invocation crate. |
| Indexed I/O | [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs): `upsert_points`, `get_points`; `query` | Parallel SDK types/operation owners. Produce batches incrementally; a chunk helper that takes the whole corpus vector is not streaming ingestion. |
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

After deleting mechanisms, combine packages that merely divide one responsibility. Source
records/history/provenance do not each need a lifecycle platform. Query planning/validation/projection/
paging can be modules around one query operation when no independent dependency boundary remains.
Keep OS/vendor adapters and isolated document workers separate where their dependencies require it.
Keep shared types only when multiple real consumers need them. Pure functions need no port/receipt.

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
