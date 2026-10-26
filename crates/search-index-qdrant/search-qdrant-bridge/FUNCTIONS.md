# Qdrant bridge — one real 1.20 runtime and exact physical generations

`search-qdrant-bridge` is the sole package allowed to depend directly on `qdrant-client`; vendor request/response,
filter, index and schema types remain private. Publication and query share one daemon-owned client/runtime.

## Selected implementation baseline

R049 selects Qdrant Server and Rust client **1.20.x** for the walking-skeleton cut. The current repository lock
still contains client 1.19.0; upgrade dependency, lock, schema builders and real consumers together.

Qdrant 1.20 provides direct BM25 on a scoring full-text payload index plus `TextQuery` and the `code` tokenizer.
The baseline no longer generates sparse BM25 vectors or owns `avg_len`.

Required concrete operations:

```text
connect(endpoint, actual optional API key, transport settings, RequestContext)
create_physical_generation(name, explicit schema)
create_scoring_text_index(field, explicit tokenizer/stemmer/stopword settings)
insert_only_points(name, fixed bounded batch)
retrieve_exact_points(name, ids)
query_text(name, field, query, scope/idf params, limit)
collection_exists/get(name)
delete_collection_terminal(name)
```

Do not expose a generic data-plane trait, dynamic provider registry, qualification token or replay ledger.

## Connection and schema

Construct from endpoint, actual optional key, finite transport settings and one absolute deadline/cancellation.
Observe live server version/capabilities and verify the exact selected schema. Do not require executable hashes,
artifact receipts, build strings or process identity as runtime credentials.

The first generation contains:

- explicit derived point identity;
- retrieval text payload;
- compact scope/evidence keys needed for filters/readback;
- one scoring full-text index using explicit text-processing settings;
- required payload indexes for scope and `idf.corpus` filters.

The `code` tokenizer improves code lexical recall but does not replace Exact. Every retrieval-text/schema choice is
part of the retrieval profile and incompatible changes create a fresh generation.

<a id="scope-and-idf"></a>
## Scope and IDF

For each `TextQuery`:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

Known-empty scope returns empty without RPC. Unknown/incomplete scope does not widen. The corpus filter changes
statistics, not result eligibility, and may be broader than the retrieval filter.

Full-text BM25 statistics are physical-shard-local. In a multi-shard collection the same document may score
differently depending on its shard. `idf.corpus` cannot reach into another shard and can silently see partial or
empty local overlap. Therefore the baseline generation does not partition a portfolio by source/root shard key.
Do not add local cross-shard score fusion. Custom sharding is reconsidered only for a scope always queried as one
partition.

Measure connected cold/warm broad/narrow scope cost. Top-k does not prove bounded upstream statistics/filter work.

## Fresh-generation profile — Stages 2 and 3A

`PublishedView` names one exact physical collection. Queries address that collection directly. Qdrant aliases are
not serving/recovery authority because an alias switch cannot atomically install ELIOT's matching redb snapshot or
preserve old pinned views.

Build a never-reused collection:

```text
create exact schema
→ create scoring full-text/payload indexes
→ explicit insert_only bounded batches
→ exact point/schema/count readback
→ return settled generation observation to publication
```

Once visible, the collection is immutable. Any selected-state change builds another complete generation. No
`valid_until`, point deletion or stable cross-generation point identity is required.

Default upsert is forbidden. A lost-response retry repeats the identical fixed batch/point and accepts only exact
readback; foreign/conflicting state fails the generation. A failed/uncertain generation is never served and its
name is never reused.

## Stage 3B optional mutation surface

Only when Stage 3A fails declared budgets may the bridge add:

- explicit `insert_only` additions;
- one fixed non-creating `valid_until` closure with exact readback;
- compact/rebuild through another fresh generation.

Do not add point-mutation APIs speculatively. If the selected Rust client cannot express a non-creating closure
unambiguously, keep full-generation publication until a verified operation exists.

<a id="whole-generation-cleanup"></a>
## Whole-generation cleanup

Delete only an exact retired physical collection after publication proves:

- it is not durable current;
- no pending/future operation can target it;
- no live `PublishedView` or `QueryWindow` guard uses it;
- no owned unresolved attempt remains targetable.

Use terminal/wait semantics, then query exact collection existence/state and observe logical absence. Never reuse
the name. Uncertain deletion/absence remains unresolved/backpressured. Logical absence is not physical disk-erasure
proof. Execute delayed request versus collection-delete behavior on the selected 1.20 server before claiming
safety.

## Qdrant Edge boundary

Qdrant Edge 0.8.0 is an official beta candidate with in-process BM25 and on-disk shards, but is not the baseline.
Its released line has manual optimize/flush obligations and recently exposed WAL acknowledgement/replay defects.
After the server path works, an isolated spike may compare a newer released Edge version. It must replace this
bridge behind the same retained/prepared/evidence boundary, not create a dual backend.

## Remove with callers

Remove supervisor binding/key leases/fixed-build qualification, manual HTTP readiness, duplicated vendor wrappers,
process-local mutation receipt maps, supplied-readback proofs and old schema/point models after real publication
and query consumers move.

## Verification

Run Rust 1.98 locked check/Clippy for bridge plus actual publication/query/daemon consumers. Then execute Qdrant
1.20 cases: live capability/schema, scoring full-text index and code tokenizer, insert-only lost response and
foreign conflict, exact readback, scope/IDF empty and partial cases, physical generation query, old-view use,
terminal delete/absence, delayed request/delete race, cancellation and measured build/query resources.
