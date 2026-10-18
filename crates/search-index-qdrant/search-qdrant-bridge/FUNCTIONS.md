# Source map and implementation — search-qdrant-bridge

The real transport exists, but its local point/payload inputs still differ from the shared planner.
The field map below records that current integration gap; it does not require a compatibility layer.
S9/S11 describe current contracts, not a permanent 20-field payload or 19-index requirement. Simplify
membership/profile identity in the shared owner, planner and bridge together; keep one schema owner.
The [implementation guide](../../../docs/implementation/README.md) selects the product path.
Change concrete producers and consumers together; keep vendor conversion private and reuse
`real::blocking::BlockingRealQueryPlane`'s Tokio runtime.

## Native query and scope

| Existing or proposed call | Concrete behavior |
|---|---|
| Existing [`query_filtered`](src/real/queries/search.rs) | Calls SDK `query(QueryPoints)`; `IdfScope::ScopedToRetrieval` copies [`base_filter`](src/real/filter.rs) into `SearchParams { idf: Some(IdfParams { corpus: Some(filter) }), .. }`. This native capability is already called. Omitting corpus gives shard-wide statistics. |
| New native BM25 integration | Use SDK `DocumentBuilder::new(text, "qdrant/bm25").options(options)` in named vectors for `upsert_points`, and `Query::new_nearest(Document { .. })` for queries. The pinned server computes BM25 locally; [lexical preparation](../../search-lexical/FUNCTIONS.md) owns useful code expansion and explicit text options. |
| New native indexed-branch ranking | Use `PrefetchQuery`, `query::Variant::Rrf(Rrf { k, weights })` and `QueryPointGroups` for useful source grouping. Formula boosting is available when a concrete ranking need warrants it. These do not require a parallel local fusion graph. |

The [pinned SDK types](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant.rs)
include these inputs. Put the selected corpus in each sparse leaf's `params.idf`; an outer fusion
parameter does not substitute for leaf search parameters. Retrieval and IDF use the same published
base scope, with additional query-only filters when useful. Set finite prefetch limits that cover
the desired output; later fusion cannot recover candidates omitted by an earlier limit.

Remove the [access compiler's](../../search-query/search-access/src/lib.rs) proof-dependent
one-membership fanout with its [executor consumer](../../search-query/search-retrieval-executor/src/indexed.rs),
which currently rejects grouped multi-plan legs. One compatible selected scope can be one native
retrieval/IDF population. Split only for actual different physical routes or incompatible search setup.

[Filtered IDF](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-idf-statistics)
requires a sparse vector with the IDF modifier, is local to the queried shard and does not fall back
to global statistics for an empty corpus. An ordinary retrieval filter alone does not scope IDF.
The [server implementation](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
enumerates matching vector-bearing points and intersects term postings, using sorted IDs or a segment
bitmask. Index corpus-filter fields; do not describe filtered IDF as a free lookup.

Use one shard for the local baseline. Native [nested fusion](https://qdrant.tech/documentation/search/hybrid-queries/)
runs per shard; top-level fusion merges across shards. Native RRF weights/ties differ from
`fuse_safe_legs`; accept the selected ranking policy without promising old scores or tie ordering.
Final grouping does not deduplicate physical points before IDF. Keep any genuinely needed bounded
merge with direct-source results in Eliot. Collections add resource overhead and cross-collection
fanout; do not create one per membership to avoid a scope predicate.

Qdrant's [sparse index](https://qdrant.tech/documentation/manage-data/indexing/#sparse-vector-index)
is exact over encoded vectors and immediately indexes applied writes in its mutable form. Wait for
completed mutations (`wait=true`) before publication; an acknowledged queued write is insufficient.
Do not wait for immutable-index optimization or a full count merely to serve sparse top-k. Exact
vector ranking remains distinct from exact source matching and complete negative evidence.

## Shared payload and identity cutover

The [old payload](src/mutation.rs) has nine fields; the current shared
[S9.5 PointPayload](../../search-contracts/src/indexed/payload.rs) has 20 closed fields and a
19-index baseline. The following map explains today's mismatch; select the new payload by actual
source/query needs and index only fields used by filters/grouping. Do not freeze these counts:

| Old bridge input | Shared disposition |
|---|---|
| `projection_membership_id`, validity epochs | Preserve the actual coordinate/interval through the shared typed validators. Published points require `valid_from >= 1` and optional `valid_until > valid_from`; absent is not null. |
| `access_partition_digest` | Not a UUID `AccessPartitionId`; do not truncate or reinterpret the digest. |
| `source_revision: u64`, `unit_ordinal: u64` | Not `SourceRevisionId` or `UnitId`. Obtain exact identities from the prepared owner output. |
| `source_membership_id`, `payload_digest` | Absent from S9.5. Shared `PointPayload::identity()` computes its internal identity from the closed canonical payload. |
| `identity_digest` | Only proven full S11 identity may become `point_identity_digest_256`; matching width/name is insufficient. |
| Missing installation/generation/source/representation/scoring/profile/kind/modality/language and optional metadata | The current planner supplies these coordinates. Simplify the shared model and its consumers together; do not invent values or maintain a second bridge schema to fill a retired field. |

[PointSpec.expected_readback.payload_cbor](../search-projection-planner/src/model.rs) already carries
the shared payload bytes; `PointPayload::from_canonical_cbor` restores and validates them. The planner's
payload builder is private and need not become another public adapter API. Use shared payload/index/
vector/eligibility constants and validators, retaining only private Qdrant value conversion.

The old [EligibilityFilter](src/query.rs) carries an access digest and source-membership strings.
Compile the owner's validated shared [EligibilityPopulation](../../search-contracts/src/indexed/eligibility.rs)
through its getters: incarnation/generation, access/scoring partitions, profile, visible epoch and
allowed projection memberships. Preserve its pre-applied restrictive fences. Source memberships are
not projection memberships, and an access digest is not a partition UUID; the immediate consumer must
supply the actual population. The bridge translates predicates and does not derive access policy.

The current [encoder](src/real/codec/payload.rs) writes extra `vector_digest_{name}` payload fields,
and [vector decoder](src/real/codec/vectors.rs) requires them. Remove that extension with its readers:
S9.5 excludes vector digests. Expected digests already live in the planner's `ExpectedReadbackShape`
and `ProjectionManifestEntry`; shared `PointReadback` carries actual typed named-vector values.
Compare actual readback with expected values for client-computed vectors. Native BM25 instead receives
text/options and produces its vector on the server: adapt the planner's precomputed-vector input and
check the generated vector's presence/shape. Do not run a second encoder to manufacture an expected
hash. A payload-stored digest does not validate returned vector values or replace source identity.

Use `search_point_identity::s11::PointId128` for the address and its existing parse/display behavior.
[bridge_point_id](src/real/identity/point.rs) currently zero-pads Qdrant numeric IDs into 128 bits.
That is a legacy conversion, not an S11 UUID identity path: new S11 admission rejects numeric IDs.
Preserve all 128 address bits, with no UUID version/variant rewriting.

The current S11 comparator requires the 256-bit digest and all eight independent key coordinates. The payload
alone lacks key schema/role; `CollectionSchema` does not yet bind a separate S11 identity-profile
descriptor. If keeping that model, supply its real binding from the owner described in
[S11_PROFILE.md](../search-point-identity/S11_PROFILE.md). If reducing it, change the shared comparator
and its consumers together. Do not infer identity from an address or add another local identity rule.

## Partial point changes

Qdrant `upsert_points` is a complete point replacement for an existing ID. The current bridge builds a
new `PointStruct` containing one payload and one named-vector set, so using it to add an optional dense
vector or repair one payload field can remove unrelated vectors or payload data that are not present in
the replacement. Full upsert is correct only for a new point or when Eliot owns and supplies the complete
authoritative point.

Use the vendor's narrower operations through this same bridge:

- `update_vectors` adds or replaces only the named vectors supplied and leaves the others intact;
- `set_payload` changes only the selected payload fields;
- full upsert replaces the complete point after assembling every required vector and payload field.

The sole publisher serializes dependent changes and performs exact readback of the affected names/fields.
Do not create a second migration or vector-backfill service. A selected semantic model can backfill its
named vector with `update_vectors` under the normal pending/visible publication. Failure or timeout after
dispatch remains an unresolved external effect for that publication.

Keep citation/source coordinates in the versioned published manifest. A reusable retrieval point may be
retained across source revisions only when its immutable retrieval/filter identity remains valid; each
pinned view still maps it to the exact revision/range that supplied its evidence. Do not mutate an old
view's source meaning in place merely to save a point write, and do not force a new point solely because
an unrelated absolute byte offset shifted.

Pinned SDK source: [`UpsertPoints`, `UpdatePointVectors`, `PointVectors` and `SetPayloadPoints`](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/proto/points.proto).

## Operation contract

| Operation | Required behavior |
|---|---|
| Connect/probe | Use the configured endpoint and its actual credentials, and check the capabilities/schema the selected path needs. Process startup belongs to the supervisor. No auth-lease or qualification-report protocol is required by every local connection. |
| Create/verify collection | Create the selected physical generation with the named vectors and filter/group indexes actually used. Read back those chosen settings and schema. Reuse the shared schema owner; do not manufacture 19 indexes or exact profile credentials to satisfy the old adapter. |
| Upsert/update | Bind the actual source/unit identity, submit bounded complete-point or narrow vector/payload input and await `wait=true` completion. Full upsert supplies the complete authoritative point; partial named-vector/payload changes use their dedicated operation. Verify the relevant stored result and resolve uncertain effects through publication/control, not a second bridge operation journal. |
| Close/delete | Operate only on the exact bounded ID list. Close uses the shared exclusive epoch boundary. Broad-filter closure/deletion is absent from the correctness API; ordinary delete does not produce a security-purge receipt. |
| Readback | Return exact address/identity coordinates, closed S9.5 payload and actual named vectors, with explicit missing/unexpected IDs. Keep manifest-held expected vector digests separate from Qdrant payload metadata. |
| Query/count/scroll | Translate the resolved scope privately as above. Ordinary top-k does not require exact count; a requested count uses that scope. Enforce actual request limits and installed schema constraints. Results are bounded nominations; evidence comes from retained source. |

Use the physical route and text/vector settings selected by the published view. Check those actual
values on the connected server; an artifact-pair or profile receipt does not replace them. Keep the
published route in its existing owner rather than reconstructing it from an adapter cache.

Epochs remain shared validated integers in `0..=2^53`; point validity has the stricter interval rule
above. Cast only at the private Qdrant range boundary; no float round-trip acceptance or logical floats.
Keep payload scalar types and missing upper-bound semantics exact.

## Outcomes, configuration and evidence

Check cancellation before dispatch and between bounded pages/batches. After a mutation or collection
creation may have committed, report unknown outcome and resolve through the same operation identity
and exact owner/readback; cancellation/timeout is not rollback. Failures remain stable and content-free.

For one node, one replica and one publisher that awaits each completed write before sending the next,
default/weak ordering is sufficient: the pinned [replica update code](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/lib/collection/src/shards/replica_set/update.rs)
selects the same local peer; strong adds leader serialization already supplied by this caller. This
conclusion does not cover concurrent writers or replicated deployments. Keep `wait=true` completion;
resolve a timed-out write before advancing dependent publication work.

The current [status codec](src/real/codec/status.rs) hardcodes strong and the
[schema verifier](src/real/errors_schema/verify.rs) requires strict mode. Change those assumptions with
their configuration consumers. Strict mode is a configurable cost/limit guard, not source correctness.
Choose its limits and rejection of unindexed filters deliberately; check the selected settings rather
than require the old defaults. [Qdrant configuration](https://qdrant.tech/documentation/operations/configuration/)
exposes these controls independently. Index fields used by the chosen scope/filter/group queries.

Keep useful live behavior cases from the [probe manifest](../../../qualification/qdrant/probes.toml):
scope/IDF, text options, named vectors, writes/readback and unknown-outcome recovery.
Connection requires actual endpoint/schema checks, not an executed qualification report credential.
The [implementation guide](../../../docs/implementation/README.md#7-verification-and-delivery)
selects verification of the integrated application.
