# Qdrant bridge — one exact 1.20 server/client profile

`search-qdrant-bridge` is the sole package allowed to depend directly on `qdrant-client`; vendor schema, request,
response, filter and point types remain private. Publication and query share one daemon-owned client/runtime.

## Dependency cut

The repository currently locks client 1.19.0. The implementation cut upgrades the Rust client, Qdrant Server,
Cargo.lock, schema builders and actual publication/query consumers together to one exact **1.20.x** patch. Startup
observes the live server version/capabilities and refuses an incompatible selected profile.

At official Rust client tag `v1.20.0`, the selected API exists:

- `CreateCollectionBuilder` permits a vectorless collection;
- `TextIndexParamsBuilder::new(TokenizerType::Code).lowercase(true).bm25_scoring()`;
- `CreateFieldIndexCollectionBuilder`;
- `UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)`;
- `Query::new_text` / `TextQueryBuilder::{k,b}`;
- `QueryPointsBuilder::{using,filter,params,limit}`;
- `SearchParamsBuilder::idf(IdfParams { corpus })`;
- `PointStruct.vectors` is optional.

Compile the exact pinned patch with the real consumers; source inspection is not compiler/runtime evidence.

## Private operations

```text
connect(endpoint, optional actual API key, bounded transport, RequestContext)
create_vectorless_generation(name, explicit collection settings)
create_scoring_text_index(field, explicit TextIndexParams)
create_filter_indexes(explicit fields)
insert_only_points(name, fixed bounded batch, wait=true)
retrieve_exact_points(name, ids)
query_text(name, field, TextQuery, candidate_filter, idf_corpus, limit)
collection_info/exists(name)
delete_collection_terminal(name)
```

No generic data-plane trait, dynamic provider registry, qualification token, bridge mutation ledger or alias manager.

## Exact schema/profile

The first physical generation has one ordinary shard and no dense/sparse vector field. Each point has:

```text
generation-bound point ID
retrieval_text
source membership / access / optional path-kind filter fields
compact evidence routing key(s)
```

Create payload indexes before point ingestion:

```text
retrieval_text: TextIndexParams {
  tokenizer = Code
  lowercase = true
  ascii_folding = false
  stemmer = none
  stopwords = none
  scoring = BM25
  nested = false
}
source/access/path-kind fields: exact keyword/integer indexes as required
```

Query-time `k` and `b` are explicit retrieval-profile values. Any behavior-affecting schema/text-composition change
creates a fresh generation. Exact punctuation/location remains outside tokenized BM25.

## Scope and IDF

For every `TextQuery`:

```text
base population = selected sources ∩ real access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Source/access clauses appear in both candidates and IDF. Request-only clauses do not. If the base population is the
entire physical collection, omit `idf`; global statistics are then exactly the selected population. Explicit empty
returns empty without RPC. Unknown/incomplete never widens.

BM25 statistics are physical-shard-local. The baseline therefore uses one ordinary shard and no per-source/root
custom-shard portfolio. Different physical collections/profiles are different score populations; the bridge does
not fuse them.

## Stage 2/3A immutable generation

```text
create exact vectorless collection
→ create scoring/filter indexes
→ insert_only fixed bounded point batches with wait=true
→ exact point/count/schema readback
→ execute a deterministic TextQuery canary
→ return settled generation observation to publication
```

Readback of payload/count alone does not prove the scorer/index works. The canary checks the actual `using` field,
text query, filter/IDF form and expected point ordering on fixed inputs.

Once visible, the collection is immutable. A selected-state change builds a new never-reused collection. No
`valid_until`, point delete or stable cross-generation identity in Stage 2/3A. Default upsert is forbidden. A
lost-response retry repeats the identical `insert_only` batch; exact readback distinguishes same result from
foreign/conflicting state. Failed/uncertain generation is never served and its name is never reused.

Qdrant notes that text statistics can temporarily include stale updated/deleted data until segment optimization.
The immutable fresh-generation profile avoids this after visibility. Stage 3B must measure update/optimizer scoring
semantics if activated.

## Stage 3B conditional mutation

Only for corpus classes which fail predeclared Stage 3A budgets may the bridge add explicit insert-only additions
and one verified non-creating fixed closure. Do not expose point mutation speculatively. If the selected client/API
cannot express the closure unambiguously, retain fresh-generation mode until a verified operation exists.

## Whole-generation cleanup

Delete an exact retired physical collection only after publication proves it is not current/targetable and no live
view/window or unresolved owned attempt can use it. Use terminal/wait deletion, then exact info/exists observation.
Never reuse the name. Uncertainty retains the cleanup row/backpressure. Logical absence is not physical erasure.
Execute delayed request/build versus collection-delete behavior on the selected 1.20 patch.

## Connection and optional processes

Construct from endpoint, optional actual key, finite settings and one absolute context. Remove supervisor binding,
key leases, fixed-build qualification, manual HTTP readiness, duplicated wrappers, receipt maps and supplied
readback proofs after consumers move.

External endpoint is baseline. Managed child is an optional later small process owner. Qdrant Edge is an isolated
replacement experiment, not a fallback or dual backend.

## Verification

Run locked Rust 1.98 check/Clippy for bridge plus real publication/query/daemon consumers. Then execute vectorless
collection creation, exact code-tokenizer/scoring schema, insert-only replay/conflict, TextQuery OR and explicit
required/excluded filters, whole-collection and selected-source IDF, deterministic readiness canary, exact evidence
routing, cancellation, old-view use, delete/absence and measured cold/warm resource cases. R051 adds no execution
result.
