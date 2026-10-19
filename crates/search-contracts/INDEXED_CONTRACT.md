# Current indexed contract

The old indexed v1 contract froze one sparse-only qualification profile into shared provider-neutral types:
20 payload fields, 19 indexes, one-shard/strict/strong assumptions, schema/payload digests and a canonical
membership population. That profile is not a compatibility promise and should not remain the product spine.

Read the [contract direction](README.md),
[Qdrant bridge map](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md),
[publication map](../search-index-qdrant/search-publication/FUNCTIONS.md) and
[point-identity issue](https://github.com/UnknownAlienHuman/eliot-search/issues/259).

## Shared indexed values

Only stable cross-owner meaning belongs here:

```text
PointId                         // full stable identity value
PointIdentityEvidence           // sufficient collision verification
OccurrenceIncarnation
RetrievalRole / RetrievalProfileId / RetrievalInputId
ViewSequence
ValidityInterval { valid_from, valid_until? }
Stable grouping/filter coordinates actually used by preparation/query/publication
ViewManifestEntry
```

Actual Qdrant payload field names, index creation structs, write ordering, shard/replica/resource settings,
strict mode, SDK vector variants and vendor readback types stay private to the bridge.

## Stable point

A point identity binds stable retrieval meaning, not an exact mutable citation:

```text
source-stable identity / occurrence incarnation
retrieval role and published profile/input identity
stable logical/grouping coordinates
```

The Qdrant point carries that identity, selected filter/group fields, view validity and actual vectors or
native document input. Exact source revision, representation, occurrence range and excerpt verification live
in `ViewManifestEntry` for one published view.

A later view may retain the same point without mutating an earlier view's citation. A changed retrieval input
or identity-bearing occurrence creates a different point. Full identity evidence is read back and compared;
a payload digest echo is not collision proof.

## Retrieval profile

The published retrieval profile binds the semantics that must match preparation and query:

- vector/document names and roles;
- native BM25 model and document/query options;
- optional explicit vector shape/distance/multivector parameters;
- tokenizer/case/stemmer/stopword/folding options when used;
- stable field/filter/grouping interpretation;
- rank/fusion profile when externally observable.

A profile change which cannot coexist safely creates a new physical collection generation and ordinary
publication. Do not preserve old points by relabelling their profile.

## Scope and IDF

A query resolves one non-widening selected base population from the pinned `PublishedView`. That population
is used for retrieval eligibility and sparse/BM25 `idf.corpus`. Optional query-only narrowing may make the
retrieval filter smaller without silently redefining the intended IDF denominator.

Empty/unknown explicit scope fails. Do not turn one membership digest into access authority or create a
provider-neutral frozen sorted population record for every query.

## Physical schema

The selected Qdrant generation may contain sparse, dense or multivector fields required by its real profile.
Local baseline topology is one node/shard/replica and one serialized publisher, but that is configuration,
not a shared correctness type. `wait=true` plus exact readback and committed-view switching establish the
local mutation boundary; strong ordering is not universal.

Schema/profile/vector incompatibility builds a new generation. Adding a vector definition does not claim
existing points contain it. Publication verifies the complete selected generation before switching the
current view.

## Evidence and readback

Do not store or expose provider-neutral fields such as:

```text
source_revision
unit_ordinal used as citation authority
payload_digest / identity_digest
vector_digest_<name>
canonical schema receipt
qualification/acceptance digest
```

Publication reads back actual vectors, payload fields, validity and identity evidence required by each page.
Query treats a Qdrant hit as a nomination, resolves it through the pinned manifest and verifies retained
bytes/range.

## Removal

Delete the frozen `PAYLOAD_FIELDS`/index table, sparse-only mode, schema/payload canonical digest authority,
eligibility/qualification proof types and duplicate bridge definitions after direct consumers migrate.
Concrete Qdrant request/response conversion remains private in the bridge; shared contracts retain only the
stable semantic values above.

## Verification

Through the connected application, verify stable point readback, selected scope/IDF population, native BM25
text ingestion/query, point→manifest→retained excerpt, profile/generation rebuild and old-view survival. A
contract fixture or digest alone does not qualify Qdrant or the product.
