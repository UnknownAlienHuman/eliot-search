# search-qdrant-bridge

Translate the application's source scope, writes and queries into the existing Qdrant Rust client.
Keep one connection/runtime and bounded I/O. Source bytes, published-view selection and recovery of
unfinished application writes remain with their existing owners.

[FUNCTIONS.md](FUNCTIONS.md) maps the actual API and remaining integration work.
The [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md) govern the target; historical qualification
receipts, compatibility rules and file quotas do not define another runtime protocol.

## Use the server's search machinery

The real [`query_filtered`](src/real/queries/search.rs) already passes an explicit IDF corpus
through the pinned 1.19 client. Qdrant 1.19 supports
[filtered IDF](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-idf-statistics):
retrieval `filter` and `params.idf.corpus` are separate parameters. Reuse the selected published
scope for both; additional query-only restrictions need not redefine the IDF population.

Use native local `qdrant/bm25` text inputs and the Query API's fusion/grouping when those branches are
needed. These are new direct integrations of an existing dependency. Do not recreate a sparse encoder,
DF store or indexed-branch fusion engine to produce the same product capability.

The native sparse index performs exact dot-product retrieval and indexes writes in its mutable form.
Publish only after completed writes (`wait=true`), not queue acknowledgement. Sparse top-k need not
wait for background optimization or an exact count; it still cannot establish exact source absence.

Start with one local node and one shard. Filtered IDF is shard-local; nested fusion also runs per shard.
With one replica and a serial publisher awaiting completed writes, strong ordering is not a required
extra layer. Choose strict-mode cost limits deliberately; neither setting establishes source identity.
A collection for every source adds independent resource overhead and cross-collection query fanout.
A separate collection is useful for an actual distinct schema/lifecycle, not as a replacement for an
ordinary selected-scope filter.

Filtered statistics still cost work: the server resolves corpus points and intersects term postings.
Use indexed scope fields, bounded requests and limited query branches. It counts physical points with
the sparse vector, so final grouping does not remove duplicated inputs from IDF.

## Existing integration boundary

The private codec still differs from the shared planner payload/identity data; the
[existing field map](FUNCTIONS.md#shared-payload-and-identity-cutover) identifies that work.
Its 20-field/19-index S9 baseline describes current types. Simplify shared owner, planner and bridge
together; index the fields actually used for filtering/grouping rather than preserve those counts.
Adapt precomputed-vector inputs for native BM25 with their producer and readback consumer.

Keep actual endpoint/schema checks, source scope and mutation outcome handling.
Ordinary top-k does not require exact count or executed test-report credentials. The existing live
behavior cases remain useful evaluation material; these documentation changes do not establish a
working native Eliot query or a measured throughput result.
