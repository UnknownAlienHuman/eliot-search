# Evidence resolution — pinned truth, real bytes, explicit domains

This file owns excerpt truth and evidence failure scope. [Windowing](../search-continuation/FUNCTIONS.md) owns
page progress and item lifetime. The replacement is not connected in the normal product path yet.

## Inspected current behavior

At `44d7416c`, `search-candidate-validator` accepts public nominations and expected digest/profile/residency/
assurance fields, copies those expectations into a readback request and compares them with metadata returned
beside `SourceReadback.bytes`. It does not establish the relation between those bytes and the digest claims by
using the real immutable-object format. Every reader error becomes `SourceUnreadable`, and `validate` turns it
into a candidate gap.

The daemon's separate result-handle path compounds this:

- handle creation and expansion list/hash the entire current source catalogue;
- expansion rejects an old result when the latest source is inactive or has another revision;
- the record contains raw revision identity/length but not the emitted representation, logical occurrence or
  original evidence range;
- callers may request arbitrary absolute offsets anywhere in the revision.

The raw-slice validator also requires `bytes.len() == source_end - source_start` and an `exact` map Boolean.
That can validate raw bytes, but not transformed text. UTF-16LE `41 00` becoming UTF-8 `41` has a two-byte raw
span and a one-byte represented span. This is a contract counterexample, not a reproduced application failure.

Keep useful range, literal and completeness checks where they apply. Remove caller-created proof authority.

## Direct operation

```text
resolve_evidence(
    RankedHit,
    pinned PublishedView,
    ResolvedScope,
    retained/prepared reader,
    RequestContext
) -> VerifiedEvidenceItem | ItemGap | BackendResultFailure | RequestStop
```

1. Validate route, generation, profile, selected scope and stable point identity against the pinned view.
2. Point-read the exact authoritative `(source_id, point_id)` row from that view's redb snapshot. The source ID
   in Qdrant is only a routing hint. On a miss, do not search another source or the latest view.
3. Obtain the exact retained source/prepared `ObjectRef`, revision, representation, occurrence and ranges from
   that row.
4. Reserve the shared request's remaining read/item/output allowance before I/O. Use the original absolute
   deadline and cancellation; no per-candidate reset.
5. Read through the real immutable-object/prepared-representation owner. Verify content/address/integrity under
   the format actually stored, not echoed metadata. Group selected reads by object when useful; do not rehash a
   multi-megabyte object once per excerpt merely to create another receipt.
6. Validate the range in its own coordinate domain and return actual excerpt bytes/text with final rank and
   contribution metadata.

A current source-head change does not invalidate an old pinned view. The result remains valid while the exact
retained object/representation and view binding exist. Freshness is reported separately; current-state scans or
catalogue digests do not authorize evidence.

## Coordinate domains

Use the existing preparation maps and transform records; do not create another universal mapping platform.

- **Raw source bytes:** validate against the retained raw object and raw-byte bounds. Equal excerpt/source span
  lengths are meaningful only here.
- **Decoded/normalized text:** validate against the exact retained prepared representation. Map its clipped
  character-safe range back through the coordinate/loss records; do not add displayed UTF-8 lengths to raw
  offsets.
- **Extracted document text:** bind page/region/representation coordinates and declared gaps/loss. Do not claim
  exact original bytes or one contiguous source range when the mapping is approximate or non-contiguous.

A `VerifiedEvidenceItem` carries its domain, representation, exact retained references, truthful location,
actual excerpt and rank metadata. Expansion reuses this binding through `(window_id, item_index)`; it never
opens arbitrary revision offsets or re-resolves against latest source state.

## Preserve failure scope

| Observation | Meaning |
|---|---|
| Foreign route/generation/scope, absent or contradictory pinned point row, malformed/duplicate backend identity | Backend/view inconsistency. Fail or contaminate that backend result; do not silently skip and claim a clean ranked set. |
| Pinned row exists but selected retained object/representation/range is genuinely missing or unsupported | Item-local gap with the actual cause. It may advance the page only when represented in the delivered response. |
| Object digest/address mismatch, invalid map or impossible range | Corruption/integrity failure; emit no excerpt and preserve the cause. |
| Cancellation, session close, absolute deadline, aggregate read/output exhaustion | Request/page stop at the unhandled item. Not evidence that the item is absent and not permission to refill indefinitely. |
| Temporary storage/system failure | Request/backend failure or finite retry according to the real owner; do not flatten it into permanent missing evidence. |

Charge work already performed. A bounded partial response may contain earlier handled cards/gaps plus a request
stop reason; the untouched tail remains unprocessed. Candidate loss, bounded top-k and window exhaustion never
become exhaustive-negative evidence.

Projection receives final-order `VerifiedEvidenceItem`s and bounded gap summaries. It does not re-sort raw
scores, mint handles, accept caller coverage Booleans or create fallback candidate IDs. If formatting is small,
inline it into query/protocol composition.

## Remove and verify

Change actual executor/view/reader/result consumers together, then remove:

- public nomination/context/expected-readback structs;
- echoed digest/profile/residency/coordinate/assurance fields;
- request/plan/grant/checkpoint permits fixed by the view/session;
- per-call ordinal/read-budget resets;
- count-only refill/completeness and validation receipts;
- leg/profile-derived fallback identities;
- current-source fences, arbitrary result handles and latest-source expansion checks.

After implementation and minimal pinned compiler/Clippy, check correct raw evidence, UTF-16/newline-normalized
and extracted excerpts, clipped mapped locations, unrelated current-source mutation, foreign composite keys,
wrong bytes with matching echoed metadata, missing versus corrupt versus cancelled reads, temporary failures,
aggregate limits and same-view expansion. The named source was inspected; no runtime success is claimed.
