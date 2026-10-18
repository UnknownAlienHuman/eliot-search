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

The prepared publication plan remains a discardable local result until the pending intent is durably written.
Current `submit` takes the coordinator's active slot in `Prepared`, but `abandon` and compensation
cannot clear that phase. A cancelled pre-intent task can leave the same coordinator permanently busy.
Remove that unnecessary reservation/state or provide its ordinary local discard with the actual caller.

Preserve actual vector values alongside planned identity/payload; the current `PointSpec` contains
vector digests, not uploadable vectors. Reuse the encoder product, not a new vector-proof API. Use
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

Use the existing `search-lexical` frozen profiles with `encode_query` / `encode_code_query`.
The profile must match published vectors, not merely a profile name supplied by the client.
Keep useful leg/fusion logic when multiple real retrieval modes use it; call pure transformations directly.
Use the same published evidence identity across contributing legs, including its actual scope/revision
and route semantics. Current candidate IDs include `leg_id`, preventing fusion of the same point from
two legs. Carry the final fused order through validation/projection: the projector currently sorts
again by raw score, which can destroy that order. Neither fix needs a ranking-proof object.

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
there is no dependency-upgrade task here.

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
