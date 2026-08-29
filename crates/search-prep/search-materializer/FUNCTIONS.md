# Function contract — `search-materializer`

**Status:** W2/P04 baseline logical contract; no selected optional document provider or runtime evidence.

This package owns deterministic baseline text/source-code materialization from one exact immutable
`SourceRevision`, including representation profile identity, coordinate maps, loss maps, assurance and
content-free receipts. It does not read arbitrary paths, own revisions, unitize, index, rank or select an
optional document provider.

## Global rules

- every materialization binds exact source revision, residency, byte digest/length and profile revision;
- baseline text/code materialization works with optional workers absent;
- source bytes are acquired only through an injected exact revision-read port;
- every transformation declares encoding/newline/normalization behavior and coordinate/loss semantics;
- lossy or incomplete output lowers assurance and reports explicit omitted/unmapped regions;
- no transformation executes content, hooks, macros, build tools, shell, network or remote resources;
- equal exact input/profile/budget produces byte-identical canonical representation/maps/receipt;
- cancellation/budget exhaustion never returns a successful complete representation.

## Profile operations

### `validate_materializer_profile(input) -> Result<ValidatedMaterializerProfile, MaterializeError>`

Requires exact profile ID/revision, supported source kinds/encodings, BOM policy, invalid-sequence policy,
newline and Unicode normalization policy, maximum input/output/map limits, coordinate spaces, loss/
assurance taxonomy and golden fixture digest.

Implicit locale/platform defaults, silent replacement, unbounded expansion and unspecified coordinate
basis are rejected.

### `profile_digest(profile) -> MaterializerProfileId`

Domain-separated canonical digest over every load-bearing behavior and bound. Any encoding,
normalization, coordinate, loss, assurance or limit change creates a different profile.

### `classify_profile_change(old, new) -> MaterializerProfileChange`

Returns `NOOP`, `REPREPARATION_AND_REPROJECTION`, `OPTIONAL_PROVIDER_GATE_REQUIRED` or `REJECT`.
Existing representations are never reinterpreted under a changed profile.

## Request and revision acquisition

### `validate_materialization_request(request, registry_view, accepted_profiles, limits) -> Result<ValidatedMaterializationRequest, MaterializeError>`

Requires exact source/revision/residency/owner/view identity, declared source kind/encoding hint, accepted
baseline profile, finite byte/output/map/deadline budget and stable operation identity.

Unsaved bytes are rejected unless they have an explicit authenticated durable snapshot-admission
receipt owned by the overlay/revision pipeline. A path/current file cannot substitute for the revision.

### `open_exact_revision(request, revision_port, deadline, cancel) -> Result<RevisionBytesGuard, MaterializeError>`

Reopens exactly the retained revision, verifies source/revision/residency/byte digest/length and returns a
bounded process-memory byte guard. Mismatch, unavailable residency or retention loss is explicit.

The package does not enumerate roots or read a pathname.

## Baseline decoding and normalization

### `detect_or_validate_encoding(bytes, request, profile) -> Result<EncodingDecision, MaterializeError>`

Uses only the accepted profile's explicit BOM/declared-encoding/UTF validity rules. Ambiguous or
unsupported encoding fails or degrades exactly as declared; it does not guess from locale.

### `decode_text_or_code(bytes, encoding, profile, budget, cancel) -> Result<DecodedRepresentation, MaterializeError>`

Produces bounded canonical scalar/text units with a native-byte-to-decoded coordinate relation and
explicit invalid/replacement/loss records. Replacement is never invisible.

### `normalize_representation(decoded, profile, budget, cancel) -> Result<CanonicalRepresentation, MaterializeError>`

Applies exactly the profile's newline and Unicode rules. It records every offset-changing transform and
rejects output expansion beyond finite limits.

No case folding, language transformation, formatting, macro expansion or semantic rewrite occurs unless
explicitly part of a separately accepted profile.

## Coordinate and loss maps

### `build_coordinate_map(native, decoded, canonical, profile) -> Result<CoordinateMap, MaterializeError>`

Builds a versioned bounded map among native byte offsets, decoded scalar/text positions and canonical
positions. It supports exact, range, ambiguous and unmapped relationships and records coordinate basis.

### `build_loss_map(native, decoded, canonical, profile) -> Result<LossMap, MaterializeError>`

Records invalid sequences, replacements, removed BOM/newline changes, normalization merges/splits,
omitted regions and unsupported constructs. It does not claim reversible mapping where none exists.

### `derive_assurance(map_bundle, warnings, profile) -> MaterializationAssurance`

Computes the maximum allowed assurance from exactness/completeness/loss/warnings. Assurance is monotonic
downward; callers cannot upgrade it.

### `validate_map_bundle(representation, coordinate_map, loss_map, profile) -> Result<MapValidationReceipt, MaterializeError>`

Checks bounds, coverage, monotonic segments where required, no overlapping contradictory mappings,
revision/profile identities and that every declared loss affects assurance consistently.

## Materialization and receipt

### `materialize_text_or_code(request, revision_port, context) -> Result<MaterializationProduct, MaterializeError>`

Runs exact revision open, encoding decision, decode, normalization, map construction, assurance and
canonical digest generation.

Success returns:

- immutable representation descriptor/ID;
- bounded canonical representation through a guarded product;
- coordinate and loss maps;
- source/revision/residency/profile/input/output digests;
- assurance/warnings/omitted-region classes;
- content-free resource and operation receipt.

No durable publication occurs here.

### `canonicalize_materialization(product) -> Result<CanonicalMaterializationBytes, MaterializeError>`

Serializes descriptors/maps/warnings deterministically with source content referenced by digest/guarded
artifact rather than embedded in technical receipts.

### `verify_materialization(product, request, profile) -> Result<MaterializationVerificationReceipt, MaterializeError>`

Recomputes identities/digests, validates maps/assurance and proves output belongs to the exact source
revision/profile. It cannot prove current filesystem state.

### `prepare_admission(product, artifact_port, operation, deadline) -> Result<MaterializationAdmissionPlan, MaterializeError>`

Creates a content-addressed immutable publication plan for the revision/preparation owner. It does not
write CAS/control state or claim admission. Unknown artifact-write outcomes remain the caller's
operation/readback responsibility.

## Optional provider boundary

### `validate_provider_descriptor(descriptor, qualification, accepted_p15) -> Result<QualifiedDocumentProvider, MaterializeError>`

A future P17-only seam requiring exact provider/runtime/artifact/license/Windows/no-execute/input/output/
coordinate/loss/assurance/resource/fuzz identities, dedicated ADR, accepted P15 and independent review.

No provider is selected by baseline code/config. Python/Node/runtime/vendor types do not enter the public
baseline API.

### `classify_provider_failure(failure, baseline_capability) -> ProviderFallbackDecision`

Returns explicit optional document gap, supported baseline text/code materialization or request failure.
It never silently switches to a different provider or relabels lossy output.

## Cancellation, deadline and retry

Validation/canonicalization are pure. Revision read and transformation use finite deadlines/budgets and
cooperative cancellation. Cancellation returns no successful complete product and releases byte/output
buffers. Equal exact request may be retried; a changed revision/profile under the same operation identity
is rejected by the caller/admission boundary.

The package owns no durable mutation and has no unknown commit outcome.

## Typed failures

- `MATERIALIZER_PROFILE_INVALID`
- `MATERIALIZER_PROFILE_MISMATCH`
- `MATERIALIZATION_REQUEST_INVALID`
- `SOURCE_REVISION_UNAVAILABLE`
- `SOURCE_REVISION_DIGEST_MISMATCH`
- `SOURCE_RESIDENCY_MISMATCH`
- `MATERIALIZATION_UNSUPPORTED`
- `SOURCE_ENCODING_AMBIGUOUS`
- `SOURCE_ENCODING_UNSUPPORTED`
- `MATERIALIZATION_INVALID_SEQUENCE`
- `MATERIALIZATION_LOSS`
- `COORDINATE_MAP_INVALID`
- `LOSS_MAP_INVALID`
- `MATERIALIZATION_ASSURANCE_VIOLATION`
- `MATERIALIZATION_BUDGET_EXHAUSTED`
- `MATERIALIZATION_CANCELLED`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`
- `PROVIDER_NOT_QUALIFIED`
- `PROVIDER_OUTPUT_INVALID`

## Required tests / qualification evidence

- UTF-8/BOM/declared encoding/invalid sequence goldens;
- CRLF/LF, Unicode normalization and offset-changing coordinate fixtures;
- exact, ambiguous and unmapped coordinate segments;
- every lossy transform creates loss record and lowers assurance;
- malformed/oversize/expansion/cancellation returns no fake complete product;
- equal source revision/profile produces deterministic representation/maps/digests;
- changed revision/profile changes representation identity;
- exact retained revision required; current path/Qdrant payload substitution rejected;
- unsaved bytes require explicit admitted snapshot and cannot be durably materialized otherwise;
- no process/toolchain/macro/shell/network/remote-resource execution;
- baseline text/code works with optional provider absent;
- optional provider descriptor remains gated/unselected and removal falls back explicitly;
- content/path absent from technical receipts/debug/log fixtures;
- fake revision/artifact/cancellation ports and no registry/index/ranking dependency.
