# search-contracts — source/caller audit and cutover

`search-contracts` should contain only small immutable data shared by real current producers and consumers.
Public main instead uses it as the schema catalogue for the complete planned Architecture 8.4 system.

## Measured source

At `c8e38b6`:

| Group | Bytes | Contents |
|---|---:|---|
| common foundation | 127,513 | bounds, generic canonical value/encoding, generic digest families, IDs, errors/reasons/schema |
| authority/query/protocol/results/evidence/lifecycle | 168,258 | grants, eleven recipes, plans/fences, handles, results, V2 evidence and progressive frames |
| indexed subtree | 113,123 | frozen collection, payload, vector, eligibility, codec and readback model |
| source vocabulary | 16,361 | source/view/revision/occurrence records |
| **production `src/`** | **425,255 (~415.3 KiB)** | 25 Rust files |
| tests | 131,618 (~128.5 KiB) | ten files; `conformance.rs` alone is 66,570 bytes |
| **total Rust** | **556,873 (~543.8 KiB)** | before downstream codec/fixture code |

Repository search finds **47 Cargo manifests** depending directly on `search-contracts`. That fan-out does not
make every exported type a valid shared contract; it makes incompatible old decisions expensive to remove.

## Caller reality

Repository-wide symbol hits include documentation, swarm records and tests, so they are not direct-caller
counts. They still expose which families are self-contained:

- `EvidenceFrameV2`: three files — its definition, provider codec and protocol test;
- `RetrieveEvidenceRequestV2`: four files — definition, provider schema and contract/protocol tests;
- `SearchTaskPlan`: 22 files concentrated in old domain/planner/assignment/fixture code;
- `RecipeIdV1`: 50 files dominated by P00 docs, generated maps, old provider/planner/result code;
- `ProviderEnvelope`: 29 files in the pairing/grant provider stack and its fixtures;
- `SearchSourceHandle`: 35 files in old recipes/results/handles/continuation paths.

No current direct daemon handler consumes one coherent request/result family. The V2 evidence types were added
beside the frozen eleven-recipe V1 family instead of replacing it. More versioned wrappers would repeat the
problem.

## Module disposition

### Delete with old provider/query systems

| Source | Reason |
|---|---|
| `authority.rs` | durable provider binding and standalone grant-policy authority removed from trusted-local baseline |
| `recipes.rs` | closed eleven-recipe V1 registry; no direct current dispatch |
| most of `query.rs` | grants, snapshots, plans, legs, fingerprints, exact compile/execute and proof records |
| `results.rs` | recipe-specific graphs/comparisons/provenance plus validation receipts and opaque handles |
| `protocol.rs` | old hello/pairing/grant envelope, capability descriptor and durable source/continuation handles |
| `evidence.rs` | parallel V2 recipe/result family carrying old plan/fence/handle vocabulary |
| `evidence_frames.rs` | second progressive frame system consumed only by the old provider codec/tests |
| `lifecycle.rs` | planned readiness/optional-provider/membership lifecycle rather than current state |

Change the protocol/client/daemon together and delete these records rather than forwarding them into another
version family.

### Split the indexed subtree

`indexed/**` freezes one Qdrant qualification profile into provider-neutral global contracts: collection
shape, payload field/index registry, sparse/dense vector models, eligibility, schema/readback digests and
caller proof records.

Keep shared only the stable semantic values used across preparation/publication/query:

```text
PointId + full collision evidence
OccurrenceIncarnation
RetrievalRole / RetrievalProfileId / RetrievalInputId
ViewId / ViewSequence / validity interval
stable grouping/filter coordinates actually used by the product
ViewManifestEntry binding point → exact retained evidence
```

Move physical collection topology, vendor vector/payload/query/readback types, index names and write ordering
into the private Qdrant bridge/publication owner. Qdrant hits are nominations; the pinned manifest and object
reader establish evidence.

### Shrink the foundation

- `ids.rs`: retain only IDs with a real current owner and at least two consumers. Delete plan, grant, receipt,
  portfolio, legacy lifecycle and unused proof IDs with their fields.
- `bounds.rs`: keep small validation for actual wire/result limits. Owner-internal collections use ordinary
  Rust types plus operation limits; do not export a type-level bound family for every planned record.
- `canonical.rs`: delete the generic JSON/CBOR-like value/encoding framework after the old proof records are
  gone. Ordinary DTOs use Serde. An owner may define one exact preimage for a real content/artifact/profile
  identity.
- `digest.rs`: keep BLAKE3/SHA-256 only beside real content/object/point/profile consumers. Delete the global
  taxonomy of digest wrappers and stored-wire constructors used only by receipts/fingerprints.
- `error.rs` / `reasons.rs`: keep a small stable public failure/gap vocabulary. Redb, Qdrant, parser, process
  and storage details stay private to their owners.
- `schema.rs`: keep only shared semantic enums still present in actual request/results. Vendor and planned
  capability schema moves or disappears.
- `source.rs`: keep current source/revision/object/location/representation values; remove portfolio/history/
  ownership-cutover structures with their systems.

## Current shared application contract

The first connected product needs approximately:

```text
identity/location:
  SourceId, RevisionId, ObjectRef, RepresentationId
  ViewId/ViewSequence, PointId
  byte/line/page/region location and logical occurrence

requests:
  SearchRequest { selected sources, query, limit/page/timeout options }
  ExactRequest { selected sources, predicate, finite scan/output limits }
  ContinueRequest { WindowId, page size }
  ExpandRequest { WindowItemRef, bounded relative context }

results:
  EvidenceCard { rank, source/revision/representation/location, verified excerpt }
  GapSummary / truthful non-exhaustive or exact-complete coverage
  Search/Exact/Continue/Expand response variants
  stable public failure code/class

retrieval/evidence:
  RetrievalProfileId and role/input identity
  stable RetrievalPoint semantic data
  ViewManifestEntry with exact retained source/range
```

Runtime guards, cancellation, streams, clocks, redb rows, Qdrant SDK records, publication cursors and query
window contents belong to their owners. Optional orientation/comparison/model/document fields exist only when
a real handler consumes them.

## Serialization

Use one current Serde DTO family. The protocol package owns length framing and message envelope; contracts own
only the operation payload/result values shared by client and daemon.

Do not preserve exact pre-release V1/V2 JSON order/bytes. Bound the complete incoming frame before decode and
the encoded result before output. Canonical bytes are reserved for identities whose owner recomputes and
verifies them; ordinary requests, plans, grants and state transitions are not hashed into authority.

## Cutover order

1. Define the current `Search | Exact | Continue | Expand` payload/result values in the existing crate; do
   not create a V3 compatibility family.
2. Change provider protocol, client, CLI and daemon dispatcher together; delete V1/V2 recipes, provider
   envelope, grants, capability descriptors and old result/handle families.
3. Change query/evidence/window consumers to direct current values; delete plans/fences/coverage proof chains.
4. Change planner/publication/bridge/query together to the stable point + versioned manifest model; move
   concrete Qdrant records into the bridge.
5. Move redb/source/object/preparation/runtime-only records to their owners.
6. Remove unused IDs, bounds, canonical/digest wrappers and reason/schema values after their last caller.
7. Delete old conformance/golden/recipe/indexed/evidence tests and generated P00/coverage contracts.
8. Reassess the remaining crate. Keep it only if a small coherent set is still shared; otherwise merge the
   DTOs into the protocol/application boundary and remove the workspace member.

Do not add forwarding modules, deprecated aliases, raw-byte restoration constructors, feature-gated old
families or `contracts-v3`.

## Verification

During each slice compile and run strict Clippy for changed real producers/consumers, plus focused current
Serde bounds, identity/location and source-backed result cases. Delete the 66.6 KiB old conformance matrix,
recipe-count tests, wire/canonical goldens and frozen indexed schema fixtures with their APIs.

Completion is the default CLI/daemon/Qdrant/retained-evidence round trip using one current contract family,
not preservation of every old type spelling.
