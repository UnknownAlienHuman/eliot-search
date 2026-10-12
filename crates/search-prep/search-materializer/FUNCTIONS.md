# Function contract — `search-materializer`

**Status:** W2/P04 baseline logical contract plus additive inert Markdown/JATS source preparation;
no document-provider acceptance or runtime qualification.

This package owns deterministic baseline text/source-code materialization from one exact immutable
`SourceRevision`, including representation profile identity, coordinate maps, loss maps, assurance and
content-free receipts. It does not read arbitrary paths, own revisions, unitize, index, rank or select an
optional document provider.

## Global rules

- every materialization binds exact source revision, residency, byte digest/length and profile revision;
- baseline source and canonical-output content digests remain direct BLAKE3-256 over exact bytes;
  baseline identity/profile/coordinate/loss commitments retain the v2 length-delimited BLAKE3 envelope;
- additive Markdown profile/inventory/product commitments use named versioned domains through the
  shared canonical CBOR/BLAKE3 producer, without reinterpreting baseline identities;
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
The v2 BLAKE3 envelope is domain length, domain, chunk count, then length and bytes for each chunk,
with every integer encoded as unsigned 64-bit little-endian. It is not compatible with historical v1 IDs.
The historical `ELIOT-MAT-V1` bytes remain immutable and are not decoded as v2. Existing v1 materializations
must be rebuilt from their exact retained source revision; the package never rewrites them in place.

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
The implementation recomputes direct BLAKE3-256 over the returned exact bytes and compares it with both
the port attestation and the validated request digest.

The package does not enumerate roots or read a pathname.

The receipt-bound UTF-8 `materialize` entry point also recomputes BLAKE3-256 over its supplied exact bytes
and rejects a mismatching digest before returning a revision-bound result.

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

## Legacy DIRECT preparation-store compatibility

The temporary T02 compatibility surface owns the frozen persisted preparation
schema and immutable artifact mechanics without widening materializer authority.

### `encode_legacy_preparation_binding(binding) -> [u8; 208]`

Emits exactly `ELSPRP02`, namespace/source/revision/content digest identities,
byte length and both profile digests in fixed order. No path, key material,
source body or authority receipt enters the record.

### `derive_legacy_preparation_lookup_key(binding, backend) -> [u8; 32]`

Uses the fixed `eliot-search/direct-preparation-ref/v2` ordered-parts preimage.
The concrete SHA-256 implementation is injected by daemon composition.

### `derive_legacy_preparation_object_id(binding, backend, manifest_digest) -> [u8; 32]`

Uses the fixed `eliot-search/direct-preparation-object/v2` preimage. Equal
manifest bytes under a different binding or protection backend cannot share an
object identity accidentally.

### `encode_legacy_preparation_reference(key, manifest_digest, manifest_bytes, protection) -> [u8; 81]`

Emits exactly `ELSPRF01`, lookup key, decoded manifest SHA-256, decoded manifest
length and the closed plaintext/protected tag. Noncanonical length/tag/key
shapes fail closed.

### `encode_legacy_preparation_manifest(binding, representation, profile_revisions, frame) -> Vec<u8>`

Emits the exact binding, representation ID, fixed digest-algorithm tags,
materializer/unitizer revisions and one bounded legacy preparation frame.
Unknown, zero or oversized shape is rejected before publication.

### `verify_legacy_preparation_manifest(manifest, binding, profile_revisions) -> LegacyPreparationManifestView`

Requires exact binding bytes, fixed algorithm tags, exact expected profile
revisions, valid frame and representation identity derived from that same
binding/frame marker. It returns a borrowed typed view rather than exposing
hard-coded daemon offsets.

### `read_legacy_preparation_artifact` / `publish_legacy_preparation_artifact`

Perform bounded exact read and same-directory no-clobber immutable publication
through injected native identity/locator/durability observations. Filesystem
state, DPAPI and source verification remain outside this package.

Compatibility tests freeze magic, sizes, field order, digest domains, locator
names, profile/algorithm tags, closed gaps, payload digest/length binding,
identity fencing and conflict behavior. This surface is not a provider
qualification, source admission, revision receipt or launch gate.

## Inert Markdown source operations

### `MarkdownProfile::new(raw, limits) -> Result<MarkdownProfile, MarkdownError>`

Require a validated strict Text-only/UTF-8-only raw profile: reject BOM, invalid sequences and loss;
preserve exact newlines with no Unicode normalization. Validate positive profile dimensions within
`MAX_MARKDOWN_LIMITS`, then compute the immutable identity internally. Bind raw profile, exact donor/
features/options, grammar/range/hierarchy/metadata decisions, all ceilings and exact source/fixture
definition bytes, including line endings. Private setup uses fixed 512 Ki logical work units and
16 KiB logical storage with no operation clock. `id()`, `raw_profile_id()` and `limits()` are read-only;
no caller digest or acceptance constructor exists.

### `MarkdownOperationLimits::validate(self) -> Result<Self, MarkdownError>`

Validate positive hard-bounded parser dimensions and canonical limits: per-frame at most
`MAX_MARKDOWN_PREIMAGE_BYTES` (2 MiB), aggregate at most `MAX_MARKDOWN_TOTAL_CANONICAL_BYTES` (4 MiB).
This method has no selected-profile argument. Operation admission separately rejects any parser
dimension above the selected profile before source acquisition. Profile/product frames retain their
tighter 16 KiB caps. Baseline budgets are separately enforced.

### `materialize_markdown(request, port, base_context, profile, limits, deadline) -> Result<MarkdownProduct, MarkdownError>`

Admit matching request/context/raw-profile identities, strict Text/UTF-8 and lower limits before reading.
Acquire through the existing exact revision port, preserve raw bytes/maps, parse bounded ordered
observations and compute complete shared commitments. Precharge actual row construction/copy work
before hash admission; charge actual successful domain/NUL/CBOR bytes once. Original cancellation,
absolute caller deadline and cumulative allowance continue across stages and terminal construction.
Failure drops provisional output.

Whole-event spans and selected mapped code/info/destination/title values remain observations.
`ObservedSpansAndSelectedValues` and `PotentiallyIncomplete` references do not imply exhaustive
references, decoded display-text coordinates or additive coverage. Ordinary decoded Text/HTML strings
are not retained as another representation; HTML/resources are never rendered, executed or fetched.

### `verify_markdown(product, request, port, base_context, profile, limits, deadline) -> Result<MarkdownVerificationReceipt, MarkdownError>`

Reopen/rebuild through the same preparation producer. Hold original inventory and residency during
replay and precharge complete comparison. Compare all raw/maps/profile/record/value/status/usage/
commitment fields, not stored digests alone. Mismatch is `ProductMismatch`; source unavailability,
limits and cancellation remain typed refusals. The immutable receipt describes exact-source replay,
not native identity, current filesystem state, access, publication or UnitSet authority.

### Private accounting and qualification boundary

Before-growth storage includes owner records/frames/selected strings, residency copies and canonical
row temporaries; original inventory remains held during replay. It excludes borrowed raw bytes,
existing raw/maps, allocator spare capacity and donor internals. Every donor next attempt, including
terminal None, is charged before advance, separately from yielded events. Original checkpoints bracket
donor construction/next, row preparation and terminal acceptance. Logical work is not CPU instrumentation;
synchronous donor calls have no hard-preemption or donor-memory guarantee.

Legacy public APIs/bytes and retained fixtures remain unchanged, apart from the final cancellation
poll in the baseline producer. This source API is not a UnitSet; the separately versioned structural
successor and JATS remain #226, persistence/adoption #331, and authored-but-uncompiled/unexecuted
fixtures plus document/native/resource/product qualification #215/#216.

## Additive inert JATS preparation (#444)

### JatsProfile::new(raw_profile, limits)

Require strict Text/UTF-8/no-BOM/no-loss/preserve-newline raw preparation, positive narrowing limits
and an internally computed profile commitment. Select Journal Publishing 1.4, XML 1.0 and the closed
element/attribute/parent tables. Bind exact quick-xml 0.42.0 with defaults off, explicit reader settings,
two retrieved official schema-file hashes (not complete imported closure), source/fixture definitions
and all finite limits. No runtime schema validation, remote resource or schema-conformance claim.

### materialize_jats(request, port, raw_context, profile, limits, deadline)

Preflight selectors, raw bounds and additional work before acquisition. Reuse the owning raw producer
and verifier, retain exact original XML/maps, then pull borrowed XML events. Reject unknown syntax,
namespaces/rebinding, DTD/DOCTYPE/PI/custom entities and unsupported declarations. Require XML attribute
whitespace and ordered declaration fields. Predefined/numeric normalization is explicit XML 1.0;
literal and decoded characters must be legal, numeric CR is not normalized twice. Mapped values obey
individual and cumulative byte ceilings before retained growth and never become exact subanchors.

Retain complete ordered elements/attributes/texts/comments/declaration/counters with original spans.
Encode every record through fixed 256-record chunks, full except last, with exact count/status header
and shared CBOR/BLAKE3 only. Charge cumulative logical storage before copies/reservations, construction
once before encoder-cap selection, and actual successful complete canonical bytes once. Raw max_steps
is independent; original cancellation/deadline checkpoints remain. Donor CPU/RSS are not measured limits.

### verify_jats(product, request, port, raw_context, profile, limits, deadline)

Hold and charge the supplied inventory, reopen the same exact revision and reconstruct the full product
through the same owning producer. Compare every raw/map/profile/ordered observation/counter/usage/
commitment field and poll the original operation before returning an immutable receipt. Rehashed
omission/order/span/value/declaration/counter mutation refuses; no partial-success receipt.

SOURCE_ONLY: JATS structural UnitSet remains a serial #226 successor, adoption/rebuild #331,
and ten authored private fixture functions remain NOT_COMPILED/NOT_RUN under #215/#216.

## Contextual legacy preparation-artifact compatibility (#468)

read_legacy_preparation_artifact_with_context(platform, context, path, maximum_bytes) and
publish_legacy_preparation_artifact_with_context(platform, context, directory, final_name,
temporary_name, bytes, maximum_bytes) are the additive opaque artifact-storage ports. They are the
legacy storage compatibility exception to the pure materialization operations above.

LegacyPreparationArtifactContext supplies the caller's original checkpoints, cumulative allowance
and possible-effect notification. Every mandatory ContextualLegacyPreparationArtifactPlatform<C>
method borrows that same &mut C and must charge/poll its internal platform closure. One private kernel
also serves the unchanged legacy APIs through private compatibility adapters. No context is renewed.

Charge filesystem/platform attempts, checked fallible logical buffer/locator capacity and exact byte
comparisons before dispatch/growth. Read/write attempts use at most 64 KiB slices; short/Interrupted
attempts and EOF probes consume requested work again. These are logical limits, not actual-transfer,
CPU/RSS or synchronous-preemption guarantees.

Exact reads fence length/time, locator and native identity. Publication preserves staged/final exact
readback, no-clobber linking, exact existing reuse and conflict refusal. Contextual publication holds
the original stage handle through verification and checks its locator before linking. It installs no
cleanup guard and never automatically unlinks staging on success, refusal, error or resource release.
Legacy cleanup/handle order, lowercase .tmp grammar and legacy WriteZero Display remain unchanged.

ContextualPreparationArtifactError renders only redacted PREPARATION_ARTIFACT_* codes and retains
paired object/platform failure plus original refusal. Read-only object_code, context_cause, effect
and outcome_unknown support later typed composition. Any failed call after a possible Directory,
Staging or Publication dispatch remains unknown. No enclosing request reconciliation is certified.
Private read/receipt fields expose exact bytes or observed identity/length/reuse only.
staging_cleanup_deferred reports this call's stage/no-unlink disposition; false proves neither stage
absence nor cleanup of earlier attempts, and true does not certify continued physical custody.

BYTE_COMPATIBLE: legacy types/signatures/errors, names, profiles, schema/digest domains and opaque bytes
remain unchanged. Native callers, #331, canonical CAS and #348 are separate. The six contextual groups,
including legacy/contextual WriteZero and 39/38 short-I/O/EOF accounting, are
AUTHORED_NOT_COMPILED/NOT_RUN under #215. Fresh #468 bounds and inclusive counts are recorded in AGENTS.md;
source adoption is not native/runtime/product qualification.
