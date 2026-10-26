# Qdrant bridge — exact 1.20 vectorless scoring-text boundary

`search-qdrant-bridge` is the only package allowed to depend directly on `qdrant-client`. Vendor schema/request/
response/filter/point types remain private. Publication and query share one daemon-owned client/runtime.

## Dependency cut

The repository still locks client 1.19.0. Upgrade Rust client, Qdrant Server, Cargo.lock, schema builders and real
publication/query consumers together to one exact **1.20.x** patch. Startup observes live server capabilities and
refuses an incompatible selected profile.

Official Rust tag `v1.20.0` provides the selected surface:

- vectorless `CreateCollectionBuilder`; optional `PointStruct.vectors`;
- `TextIndexParamsBuilder`, `KeywordIndexParamsBuilder`, `Memory`;
- `CreateFieldIndexCollectionBuilder`;
- `TextQueryBuilder`, `Query::new_text`, `QueryPointsBuilder`;
- `SearchParamsBuilder::idf`;
- `UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)`;
- explicit payload/vector response selectors.

Compile the exact pinned patch with real consumers. Source inspection is not build evidence.

## Physical profile V1

Stage 2 creates one vectorless one-shard collection and one scoring text field. Use exact builder semantics:

```rust
let text = TextIndexParamsBuilder::new(TokenizerType::Code)
    .lowercase(true)
    .ascii_folding(false)
    .phrase_matching(false)
    .disabled_stemmer()
    .enable_hnsw(false)
    .memory(Memory::Pinned)
    .bm25_scoring()
    .build();

CreateFieldIndexCollectionBuilder::new(collection, "retrieval_text", FieldType::Text)
    .field_index_params(text)
    .wait(true)
```

Verify readback shows:

```text
tokenizer=Code
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords=None
min/max token length=None
nested=None
scoring=BM25
enable_hnsw=false
memory=Pinned (Stage 2)
```

Text/keyword payload indexes default HNSW to true and memory to Pinned; omission is not accepted as a profile.
Stage 3A may select `Pinned`, `Cached` or `Cold` per declared corpus class, but it sets and measures the choice
explicitly. A physical schema/resource-profile change creates a fresh generation.

Stage 2 has one file and no candidate filter, so it creates no ceremonial source/access indexes. Stage 3A creates
only filter indexes used by real scope clauses. Exact keyword indexes use `KeywordIndexParamsBuilder` with
`enable_hnsw(false)`, explicit memory and no prefix/tenant optimization unless a concrete query shape requires it.

Do not use deprecated `on_disk`, dummy vectors, sparse vector schema, custom shards or aliases.

## Private operations

```text
connect(endpoint, optional actual key, bounded transport, RequestContext)
create_vectorless_generation(name, exact physical profile)
create_scoring_text_index(field, exact params)
create_required_filter_indexes(exact params)
insert_only_points(name, fixed bounded batch, wait=true)
retrieve_exact_points(name, ids)
query_text(name, TextQuery, candidate_filter, idf_corpus, response_projection, limit)
collection_info/exists(name)
delete_collection_terminal(name)
```

No generic data-plane trait, provider registry, qualification token, alias manager or bridge mutation ledger.

## Point and response shape

Each point contains generation-bound ID plus bounded payload:

```text
retrieval_text
source_id
other real scope/evidence-routing fields only
```

Query explicitly requests:

```rust
.with_vectors(false)
.with_payload(vec!["source_id" /* plus only required routing keys */])
```

Do not return vectors or `retrieval_text`; retained evidence/object reads supply excerpts. Bound response points and
payload bytes before allocation/output.

## Query-scoring profile

Index schema and query BM25 parameters are distinct. Stage 2 fixes query-time:

```text
k=1.2
b=0.75
```

Client validates finite `k >= 0` and finite `0 <= b <= 1` before building `TextQuery`. Changing only k/b does not
rebuild the collection; capture the query profile in `RequestContext`. Do not set a baseline score threshold.

`TextQuery` ranks documents containing any query token. A full-text `MatchText` filter requires all filter-query
subwords in any order. With Code tokenizer, ranking queries also receive joined camel/snake tokens while filters
receive subwords only. Therefore typed required/excluded text filters are not exact identifiers or phrases.

## Scope and IDF

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Source/access clauses appear in both candidate filter and IDF corpus. Request-only clauses do not. If base is the
entire physical collection, omit `idf`. Explicit empty returns without RPC; unknown/incomplete never widens.
Every filter field is indexed. One ordinary shard keeps one BM25 statistics population.

## Stage 2/3A immutable generation

```text
create exact vectorless collection
→ create/verify scoring text index
→ create/verify only required filter indexes
→ InsertOnly fixed point batches with wait=true
→ exact point/payload/count/schema readback
→ deterministic TextQuery canary with expected order
→ return settled generation observation
```

Readback alone does not prove scorer/index wiring. Once visible, the collection is immutable. A selected-state
change creates a new never-reused collection. Generation-bound IDs suffice; no sequence/closure/stable occurrence
identity. Retry repeats identical InsertOnly input and accepts only exact matching readback. Failed/uncertain
collections are never served or name-reused.

Qdrant text statistics can include stale updated/deleted data until segment optimization. Stage 3B, if selected,
qualifies scoring convergence in addition to mutation readback.

## Cleanup and process boundary

Delete a retired collection only after publication proves no durable/current/pending/future target, live guard or
unresolved attempt can use it. Delete terminally, verify exact absence, never reuse name and retain cleanup row on
uncertainty. Logical absence is not physical erase proof.

External endpoint is baseline. Managed child is an optional later small owner. Qdrant Edge is an isolated
replacement experiment, never fallback/dual backend.

## Verification

Run locked Rust 1.98 check/Clippy for bridge plus real publication/query/daemon consumers. Execute vectorless
creation; complete explicit schema/readback; HNSW disabled; Pinned/Cached/Cold profiles; minimal payload/no vectors;
InsertOnly replay/conflict; Code tokenizer document/ranking/filter behavior; k/b validation; any-token TextQuery;
all-token filters; whole/scoped IDF; deterministic canary; cancellation; old-view use; delete/absence; and measured
cold/warm memory/cache/disk cost. R052 changes docs only.
