# Function contract — `search-unitizer`

**Status:** W2/P04 logical contract; no unitization runtime or qualification evidence exists.

This package owns deterministic bounded unit occurrences and manifests derived from one exact canonical
representation. It owns no source/revision storage, materialization, semantic enrichment, lexical/model
encoding, ranking, Qdrant projection or query behavior.

## Global rules

- one unit is an occurrence in one exact representation/profile, not a globally stable semantic entity;
- every unit and manifest binds source revision, representation, coordinate-map and profile identities;
- boundaries, kinds, ordinals, overlap/continuation policy and size handling are profile-defined;
- same exact representation/profile/budget produces byte-identical ordered units and manifest;
- anchors always identify coordinate basis and source-revision lineage;
- no unit carries ranking score, vendor payload or corpus/access policy;
- cancellation/budget exhaustion never yields a successful complete manifest.

## Profile operations

### `validate_unitizer_profile(input) -> Result<ValidatedUnitizerProfile, UnitizeError>`

Requires exact profile ID/revision, accepted representation kinds, boundary algorithm, unit kinds,
minimum/maximum bytes/scalars/lines, overlap/continuation policy, empty/oversize handling, anchor policy,
configuration-predicate attachment rules, finite unit/count/depth limits and golden fixture digest.

Implicit language/tokenizer/parser defaults and zero-as-unlimited bounds are rejected.

### `profile_digest(profile) -> UnitizerProfileId`

Domain-separated canonical digest over all behavior and limits. Any boundary, size, overlap, kind,
anchor, predicate or limit change yields a new profile.

### `classify_profile_change(old, new) -> UnitizerProfileChange`

Returns `NOOP`, `REUNITIZE_AND_REPROJECT` or `REJECT`. Existing unit IDs/manifests are never
reinterpreted under a changed profile.

## Request validation

### `validate_unitization_request(request, representation_receipt, profile, limits) -> Result<ValidatedUnitizationRequest, UnitizeError>`

Requires exact source/revision/representation/profile/map/assurance identities, finite representation
length, unit/count/output/deadline budget and stable operation identity.

A representation with invalid/mismatched coordinate/loss map or insufficient assurance for the requested
unit kind is rejected or explicitly restricted according to profile. Current path/Qdrant payload is not
an input.

## Boundary and occurrence operations

### `scan_boundaries(representation, profile, budget, cancel) -> Result<BoundarySequence, UnitizeError>`

Produces canonical ordered boundary candidates using only the accepted profile and representation. It
does not execute code, invoke language servers/parsers not already represented by an accepted
structural product or infer compiler semantics.

Cancellation returns an incomplete scan that cannot be converted into a complete unit manifest.

### `build_occurrence(boundary, ordinal, representation, maps, profile) -> Result<UnitOccurrenceDraft, UnitizeError>`

Creates one bounded occurrence with kind, canonical span, native anchor relation, ordinal, continuation/
overlap metadata and explicit configuration/structure hints supplied by the representation/profile.

### `derive_unit_id(context, occurrence) -> Result<UnitId, UnitizeError>`

Domain-separates source revision, representation ID, unitizer profile, occurrence ordinal/kind/span and
load-bearing structural identity hints. Path/display text and ranking score are forbidden inputs.

A changed representation/profile/occurrence identity changes the unit ID. The function does not claim
semantic identity across revisions/reparses.

### `validate_unit_anchor(unit, coordinate_map, loss_map, representation) -> Result<AnchorValidationReceipt, UnitizeError>`

Proves canonical/native spans are in bounds, map to the exact source revision/representation, respect
loss/ambiguity and do not exceed the unit's assurance. An ambiguous/unmapped native region remains
explicit.

### `validate_configuration_predicate(unit, predicate_schema) -> Result<PredicateValidationReceipt, UnitizeError>`

Checks bounded canonical predicate syntax/identity and assurance. Unknown cfg/build predicates remain
unknown/conditional; the unitizer never evaluates compiler truth.

## Manifest operations

### `build_unit_manifest(request, boundaries, context) -> Result<UnitManifest, UnitizeError>`

Builds every occurrence in canonical order and validates:

- finite unit count/encoded bytes;
- unique ordinals and unit IDs;
- profile-compliant sizes/overlap/continuation;
- complete accounting of represented and intentionally omitted ranges;
- exact source/revision/representation/map/profile identity;
- no duplicate/conflicting anchors or predicates.

Success contains immutable unit descriptors and digests, not source bodies or ranking data.

### `canonicalize_unit_manifest(manifest) -> Result<CanonicalUnitManifestBytes, UnitizeError>`

Serializes schema/profile/source/representation/map identities, ordered unit descriptors, omission/gap
metadata and counts deterministically.

### `manifest_digest(manifest) -> UnitManifestDigest`

Domain-separated digest over canonical manifest bytes.

### `verify_unit_manifest(manifest, request, representation, maps, profile) -> Result<UnitManifestVerificationReceipt, UnitizeError>`

Recomputes unit IDs, ordering, spans, anchors, predicates, count and manifest digest and proves complete
profile-defined accounting. It does not assert filesystem currentness or indexed publication.

### `diff_unit_manifests(old, new) -> Result<UnitManifestDiff, UnitizeError>`

Returns exact created/retained/retired unit IDs and changed identity reasons. Retained requires exact unit
identity and descriptor digest; heuristic span/name similarity cannot retain a unit.

## Main operation

### `unitize(request, representation_port, context) -> Result<UnitizationProduct, UnitizeError>`

Reopens or borrows the exact verified representation, scans boundaries, builds/validates occurrences and
returns the immutable manifest, canonical digest and content-minimized resource/operation receipt.

The package performs no durable admission/publication/indexing. Representation bytes remain guarded and
are released after completion/cancellation.

## Batch operations

### `unitize_batch(requests, representation_port, limits, deadline, cancel) -> Result<UnitizationBatch, UnitizeError>`

Processes finite canonical requests and returns one explicit outcome per representation. Cancellation
marks unprocessed/incomplete items; it never drops them or labels the batch complete.

## Redaction and disclosure

### `redacted_manifest_view(manifest, disclosure) -> RedactedUnitManifestView`

Returns opaque source/revision/representation/profile/unit IDs, kinds, counts, span length classes,
assurance/gaps and digests. It excludes source text and unrestricted paths.

## Cancellation, deadline and retry

Profile/request/manifest verification is pure and retry-safe. Boundary/unitization loops use finite
budgets and cancellation checkpoints. There is no durable mutation or unknown commit outcome. Retrying
equal exact input is safe; changed representation/profile under the same operation identity is rejected
by caller/admission logic.

## Typed failures

- `UNITIZER_PROFILE_INVALID`
- `UNITIZER_PROFILE_MISMATCH`
- `UNITIZATION_REQUEST_INVALID`
- `UNSUPPORTED_REPRESENTATION`
- `REPRESENTATION_ASSURANCE_INSUFFICIENT`
- `UNIT_BOUNDARY_INVALID`
- `UNIT_SIZE_POLICY_VIOLATION`
- `UNIT_COUNT_LIMIT_EXCEEDED`
- `UNIT_ID_CONFLICT`
- `ANCHOR_MAPPING_FAILED`
- `CONFIGURATION_PREDICATE_INVALID`
- `UNIT_MANIFEST_INCOMPLETE`
- `UNIT_MANIFEST_DIGEST_MISMATCH`
- `UNITIZATION_NONDETERMINISTIC`
- `UNITIZATION_BUDGET_EXHAUSTED`
- `UNITIZATION_CANCELLED`
- `UNITIZATION_BATCH_INCOMPLETE`

## Required tests / qualification evidence

- canonical profile, unit ID and manifest byte/digest goldens;
- same representation/profile produces identical units/order/manifest;
- every load-bearing representation/profile/ordinal/kind/span change changes unit identity;
- path/display/ranking/vendor data excluded from unit ID/public schema;
- line/block/paragraph/code-boundary, empty, overlap, continuation and oversize fixtures;
- native/canonical anchor exact/ambiguous/unmapped/loss fixtures;
- unknown cfg/build predicate preserved, never treated unconditional/compiler truth;
- complete accounting of represented/omitted ranges;
- duplicate ID/ordinal/anchor/predicate conflicts fail closed;
- cancellation/budget/oversize returns no fake complete manifest;
- batch accounts every item;
- changed profile requires reunitization/reprojection;
- no source store, lexical/model, ranking, Qdrant or vendor dependency;
- content/path absent from manifest technical/debug/log views;
- fake representation/cancellation ports and deterministic property tests.
