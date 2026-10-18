# Function contract — `search-qdrant-bridge`

**Source state:** oracle and real/vendor transport exist at `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`,
but the bridge still exposes its local point/payload/schema types. #262 adopts the delivered shared
S9.5/S11 contracts. PR #609's static artifact/configuration admission is not live pair qualification;
actual consumer effects and live evidence remain #263/#264.

Public operations use Eliot-owned contracts and ports. Vendor types stay private. This file records
the required behavior and the concrete mismatch; its operation names do not require another facade.

**Target simplification:** the [current local architecture](../../../docs/architecture/README.md) removes
executed qualification reports from runtime connection prerequisites and exact counts from ordinary
top-k. Change the current API and its callers rather than supplying fabricated `QualifiedGate` receipts.
`real::blocking::BlockingRealQueryPlane` already owns a reusable Tokio runtime; use it. The shared
payload/S11 mapping below describes actual existing types, not a mandate for another compatibility layer.

## Shared payload and identity cutover

The [old payload](src/mutation.rs) has nine fields; the shared
[S9.5 PointPayload](../../search-contracts/src/indexed/payload.rs) has 20 closed fields and a
19-index baseline. It is not a rename-compatible representation:

| Old bridge input | Shared disposition |
|---|---|
| `projection_membership_id`, validity epochs | Preserve the actual coordinate/interval through the shared typed validators. Published points require `valid_from >= 1` and optional `valid_until > valid_from`; absent is not null. |
| `access_partition_digest` | Not a UUID `AccessPartitionId`; do not truncate or reinterpret the digest. |
| `source_revision: u64`, `unit_ordinal: u64` | Not `SourceRevisionId` or `UnitId`. Obtain exact identities from the prepared owner output. |
| `source_membership_id`, `payload_digest` | Absent from S9.5. Shared `PointPayload::identity()` computes its internal identity from the closed canonical payload. |
| `identity_digest` | Only proven full S11 identity may become `point_identity_digest_256`; matching width/name is insufficient. |
| Missing installation/generation/source/representation/scoring/profile/kind/modality/language and optional metadata | Consume the exact shared payload already produced by the planner. Do not invent defaults, infer missing coordinates from an address, or expand the closed field set. |

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
Validate actual readback against that existing expected evidence at the named consumer. Do not trust
a payload-stored digest as proof of returned vector values or add fields to avoid the owner cutover.

Use `search_point_identity::s11::PointId128` for the address and its existing parse/display behavior.
[bridge_point_id](src/real/identity/point.rs) currently zero-pads Qdrant numeric IDs into 128 bits.
That is a legacy conversion, not an S11 UUID identity path: new S11 admission rejects numeric IDs.
Retain a legacy decoder only for an identified stored format/reader, separately from new point creation.
Preserve all 128 address bits, with no UUID version/variant rewriting.

Full S11 readback requires the 256-bit digest and all eight independent key coordinates. The payload
alone lacks key schema/role; `CollectionSchema` does not yet bind a separate S11 identity-profile
descriptor. The named generation/manifest/publication consumer must supply that exact binding as
specified in [S11_PROFILE.md](../search-point-identity/S11_PROFILE.md). Do not infer it from generation
UUID, address or copied digest. Use the existing S11 comparator; remove the local identity rule.

## Operation contract

| Operation | Required behavior |
|---|---|
| Connect/probe | Consume exact supervisor process/incarnation/endpoint identity and a bounded auth lease; never discover/start the process. Check actual installed/connected server capabilities and schema; executed qualification reports belong to evaluation/release, not runtime credentials. |
| Create/verify collection | Create one opaque non-admitted physical generation, every mandatory shared payload index, exact vector/strict/settings profile, then read back schema/settings/metadata before admission. Version strings alone prove no schema identity. |
| Upsert | Exact point IDs, full identity/collision checks, bounded canonical input, `wait=true`, strong ordering and exact readback. Same mutation identity/input is idempotent; changed input conflicts. Durable operation authority belongs to publication/control, not a process-local map. |
| Close/delete | Operate only on the exact bounded ID list. Close uses the shared exclusive epoch boundary. Broad-filter closure/deletion is absent from the correctness API; ordinary delete does not produce a security-purge receipt. |
| Readback | Return exact address/identity coordinates, closed S9.5 payload and actual named vectors, with explicit missing/unexpected IDs. Keep manifest-held expected vector digests separate from Qdrant payload metadata. |
| Query/count/scroll | Compile the accepted closed eligibility AST privately. Retrieval and `idf.corpus` use the same canonical base plan; unsupported/unindexed filters fail rather than scan. Ordinary top-k does not require exact count. Requested counts use the same scope. Results are bounded nominations, never source evidence. |

Each admitted operation binds the actual physical name, generation, schema/profile, connected artifact/pair,
settings and required route revision/incarnation. A truncated generated name or reconstructed local cache
is not durable authority. The enabled sparse/dense profile must match what the adapter actually serves.

Epochs remain shared validated integers in `0..=2^53`; point validity has the stricter interval rule
above. Cast only at the private Qdrant range boundary; no float round-trip acceptance or logical floats.
Keep payload scalar types and missing upper-bound semantics exact.

## Outcomes, configuration and evidence

Check cancellation before dispatch and between bounded pages/batches. After a mutation or collection
creation may have committed, report unknown outcome and resolve through the same operation identity
and exact owner/readback; cancellation/timeout is not rollback. Failures remain stable and content-free.

[Qdrant settings](../../../config/sections/qdrant_data.md) may vary batch/time limits only within the
admitted profile. Strict mode, wait-for-mutations and strong ordering remain correctness floors.

Keep the [probe manifest](../../../qualification/qdrant/probes.toml) as the capability inventory:
exact build/auth, topology/index/schema/strict readback, signed range/missing-field behavior, filtered
IDF/independent corpus, named vectors, wait/strong/exact CRUD and unknown-outcome recovery.
Add the causal shared-payload closure, UUID/numeric-ID distinction, full S11 collision/profile,
exact route/restart admission and vector-readback cases to the existing owner fixtures.
The [execution protocol](../../../docs/product/EXECUTION_PROTOCOL.md#4-implement-then-verify-the-production-closure)
selects production checks/guards and defers test/live execution until integrated implementation.
