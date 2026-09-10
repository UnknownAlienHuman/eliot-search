//! Exact revision acquisition and end-to-end baseline materialization.
//!
//! Source bytes arrive only through the injected [`RevisionReadPort`], which
//! reopens exactly the retained revision. This package verifies the
//! port-attested digest, length and residency against the validated request;
//! the cryptographic readback itself belongs to the revision pipeline, like
//! the receipt-bound raw path in the crate root. Equal exact
//! revision/profile/budget input produces byte-identical canonical
//! representation, maps and receipts. No durable publication occurs here.

use crate::MaterializationError;
use crate::assurance::{MaterializationAssurance, derive_assurance};
use crate::decode::{StepCounter, decode_text_or_code, detect_or_validate_encoding};
use crate::maps::{
    MapBundle, MapIdentities, build_coordinate_map, build_loss_map, validate_map_bundle,
};
use crate::normalize::{CanonicalRepresentation, normalize_representation};
use crate::profile::{
    MaterializerProfileId, SourceEncoding, ValidatedMaterializerProfile, digest32,
};
use crate::request::{CancellationToken, MaterializationBudget, ValidatedMaterializationRequest};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

/// Revision bytes attested by the revision pipeline for one exact revision.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredRevisionBytes {
    bytes: Vec<u8>,
    content_digest: Blake3Digest32,
    residency: OpaqueId,
}

impl StoredRevisionBytes {
    /// Builds port-attested revision bytes. The port owns digest correctness;
    /// [`open_exact_revision`] binds it to the validated request.
    #[must_use]
    pub const fn new(bytes: Vec<u8>, content_digest: Blake3Digest32, residency: OpaqueId) -> Self {
        Self {
            bytes,
            content_digest,
            residency,
        }
    }

    /// Attested bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Port-attested content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    /// Port-attested residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the attested bytes are empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl core::fmt::Debug for StoredRevisionBytes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("StoredRevisionBytes")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("content_digest", &self.content_digest)
            .field("residency", &self.residency)
            .finish()
    }
}

/// Injected exact revision-read port. Implementations reopen exactly the
/// retained revision and attest its digest and residency; they never
/// enumerate roots or read pathnames.
pub trait RevisionReadPort {
    /// Reads exactly the retained revision or fails with a typed error
    /// (notably [`MaterializationError::RevisionUnavailable`]).
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError>;
}

/// Bounded process-memory byte guard for one verified revision.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionBytesGuard {
    bytes: Vec<u8>,
    source_id: OpaqueId,
    revision: NonZeroRevision,
    residency: OpaqueId,
    content_digest: Blake3Digest32,
}

impl RevisionBytesGuard {
    /// Verified revision bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the guard into its verified bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the verified bytes are empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Verified source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Verified retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Verified residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Verified content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }
}

impl core::fmt::Debug for RevisionBytesGuard {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RevisionBytesGuard")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("residency", &self.residency)
            .field("content_digest", &self.content_digest)
            .finish()
    }
}

/// Reopens exactly the retained revision and binds it to the request.
///
/// Verifies port-attested residency, content digest and byte length against
/// the validated request. Mismatch, unavailable residency or retention loss
/// is an explicit typed error, never substituted content.
pub fn open_exact_revision(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    cancel: CancellationToken<'_>,
) -> Result<RevisionBytesGuard, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let stored = port.read_exact(
        request.source_id(),
        request.revision(),
        request.byte_count(),
    )?;
    if stored.residency != *request.residency() {
        return Err(MaterializationError::ResidencyMismatch);
    }
    if stored.content_digest != request.content_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    let actual =
        u64::try_from(stored.bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if actual != request.byte_count() {
        return Err(MaterializationError::InputLengthMismatch);
    }
    if stored.bytes.is_empty() {
        return Err(MaterializationError::EmptyInput);
    }
    Ok(RevisionBytesGuard {
        bytes: stored.bytes,
        source_id: request.source_id().clone(),
        revision: request.revision(),
        residency: stored.residency,
        content_digest: stored.content_digest,
    })
}

/// Materialization execution context: bound profile, finite budgets and
/// cooperative cancellation.
#[derive(Clone, Debug)]
pub struct MaterializationContext<'a> {
    /// Bound validated profile (must match the validated request).
    pub profile: ValidatedMaterializerProfile,
    /// Finite operation budgets.
    pub budget: MaterializationBudget,
    /// Cooperative cancellation token.
    pub cancel: CancellationToken<'a>,
}

/// Content-free surfaced warning. Warnings always accompany loss records;
/// loss without warnings is an assurance violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializationWarning {
    /// A leading byte-order mark was stripped and recorded.
    BomStripped,
    /// Bytes were transcoded exactly from the named encoding.
    Transcoded(SourceEncoding),
    /// A counted number of line terminators was normalized and recorded.
    NewlinesNormalized(u64),
}

impl MaterializationWarning {
    /// Stable short name used in receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BomStripped => "bom-stripped",
            Self::Transcoded(_) => "transcoded",
            Self::NewlinesNormalized(_) => "newlines-normalized",
        }
    }
}

/// Content-free resource and operation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceReceipt {
    /// Exact input bytes consumed.
    pub input_bytes: u64,
    /// Exact canonical output bytes produced.
    pub output_bytes: u64,
    /// Elementary decode/normalize/map steps consumed.
    pub steps_used: u64,
    /// Coordinate segments produced.
    pub segments: u64,
    /// Loss records produced.
    pub loss_records: u64,
}

/// Immutable materialization product with deterministic identity.
#[derive(Clone, Eq, PartialEq)]
pub struct MaterializationProduct {
    representation_id: Blake3Digest32,
    source_id: OpaqueId,
    revision: NonZeroRevision,
    profile_id: MaterializerProfileId,
    encoding: SourceEncoding,
    input_digest: Blake3Digest32,
    canonical_digest: Blake3Digest32,
    coordinate_digest: Blake3Digest32,
    loss_digest: Blake3Digest32,
    canonical: CanonicalRepresentation,
    maps: MapBundle,
    assurance: MaterializationAssurance,
    warnings: Vec<MaterializationWarning>,
    resource: ResourceReceipt,
}

impl MaterializationProduct {
    /// Deterministic identity over revision, profile, encoding, bytes and maps.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Materialized source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Materialized retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Bound profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Decided source encoding.
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Exact input digest bound from the validated request.
    #[must_use]
    pub const fn input_digest(&self) -> Blake3Digest32 {
        self.input_digest
    }

    /// Digest over the canonical text bytes.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Digest over the serialized coordinate segments.
    #[must_use]
    pub const fn coordinate_digest(&self) -> Blake3Digest32 {
        self.coordinate_digest
    }

    /// Digest over the serialized loss records.
    #[must_use]
    pub const fn loss_digest(&self) -> Blake3Digest32 {
        self.loss_digest
    }

    /// Canonical text.
    #[must_use]
    pub fn canonical_text(&self) -> &str {
        self.canonical.text()
    }

    /// Canonical representation with its offset-change record.
    #[must_use]
    pub const fn canonical(&self) -> &CanonicalRepresentation {
        &self.canonical
    }

    /// Coordinate and loss maps.
    #[must_use]
    pub const fn maps(&self) -> &MapBundle {
        &self.maps
    }

    /// Derived assurance bound.
    #[must_use]
    pub const fn assurance(&self) -> MaterializationAssurance {
        self.assurance
    }

    /// Surfaced content-free warnings.
    #[must_use]
    pub fn warnings(&self) -> &[MaterializationWarning] {
        &self.warnings
    }

    /// Content-free resource receipt.
    #[must_use]
    pub const fn resource_receipt(&self) -> ResourceReceipt {
        self.resource
    }
}

impl core::fmt::Debug for MaterializationProduct {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MaterializationProduct")
            .field("representation_id", &self.representation_id)
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("profile_id", &self.profile_id)
            .field("encoding", &self.encoding)
            .field(
                "canonical",
                &format_args!("<{} bytes>", self.resource.output_bytes),
            )
            .field("assurance", &self.assurance)
            .field("warnings", &self.warnings)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}

fn warnings_for(bundle: &MapBundle, encoding: SourceEncoding) -> Vec<MaterializationWarning> {
    let mut warnings = Vec::new();
    let mut newlines = 0_u64;
    for record in bundle.loss_map().records() {
        match record.kind {
            crate::maps::LossKind::RemovedBom => warnings.push(MaterializationWarning::BomStripped),
            crate::maps::LossKind::TranscodedEncoding => {
                warnings.push(MaterializationWarning::Transcoded(encoding));
            }
            crate::maps::LossKind::NormalizedNewline => {
                newlines = newlines.saturating_add(1);
            }
        }
    }
    if newlines > 0 {
        warnings.push(MaterializationWarning::NewlinesNormalized(newlines));
    }
    warnings
}

fn digest_coordinate_map(bundle: &MapBundle) -> Blake3Digest32 {
    let mut bytes = Vec::with_capacity(bundle.coordinate_map().segments().len() * 49);
    for segment in bundle.coordinate_map().segments() {
        bytes.extend_from_slice(&segment.native_start.to_le_bytes());
        bytes.extend_from_slice(&segment.native_end.to_le_bytes());
        bytes.extend_from_slice(&segment.decoded_start.to_le_bytes());
        bytes.extend_from_slice(&segment.decoded_end.to_le_bytes());
        bytes.extend_from_slice(&segment.canonical_start.to_le_bytes());
        bytes.extend_from_slice(&segment.canonical_end.to_le_bytes());
        bytes.push(match segment.relation {
            crate::maps::SegmentRelation::Exact => 1,
            crate::maps::SegmentRelation::Range => 2,
            crate::maps::SegmentRelation::Ambiguous => 3,
            crate::maps::SegmentRelation::Unmapped => 4,
        });
    }
    Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/coordinates/v1",
        &[&bytes],
    ))
}

fn digest_loss_map(bundle: &MapBundle) -> Blake3Digest32 {
    let mut bytes = Vec::with_capacity(bundle.loss_map().records().len() * 49);
    for record in bundle.loss_map().records() {
        bytes.push(match record.kind {
            crate::maps::LossKind::RemovedBom => 1,
            crate::maps::LossKind::TranscodedEncoding => 2,
            crate::maps::LossKind::NormalizedNewline => 3,
        });
        bytes.extend_from_slice(&record.native_start.to_le_bytes());
        bytes.extend_from_slice(&record.native_end.to_le_bytes());
        bytes.extend_from_slice(&record.decoded_start.to_le_bytes());
        bytes.extend_from_slice(&record.decoded_end.to_le_bytes());
        bytes.extend_from_slice(&record.canonical_start.to_le_bytes());
        bytes.extend_from_slice(&record.canonical_end.to_le_bytes());
    }
    Blake3Digest32::from_bytes(digest32(b"eliot-search/materializer/loss/v1", &[&bytes]))
}

fn digest_representation(
    request: &ValidatedMaterializationRequest,
    encoding: SourceEncoding,
    canonical_text: &str,
    coordinate_digest: &Blake3Digest32,
    loss_digest: &Blake3Digest32,
) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/product/v1",
        &[
            request.source_id().as_str().as_bytes(),
            &request.revision().get().to_le_bytes(),
            request.profile().id().as_bytes(),
            &[encoding_tag(encoding)],
            request.content_digest().as_bytes(),
            canonical_text.as_bytes(),
            coordinate_digest.as_bytes(),
            loss_digest.as_bytes(),
        ],
    ))
}

const fn encoding_tag(encoding: SourceEncoding) -> u8 {
    match encoding {
        SourceEncoding::Utf8 => 1,
        SourceEncoding::Utf16Le => 2,
        SourceEncoding::Utf16Be => 3,
    }
}

/// Runs exact revision open, encoding decision, decode, normalization, map
/// construction, assurance derivation and canonical digest generation.
///
/// Cancellation or budget exhaustion never returns a successful complete
/// representation. No durable publication occurs here.
pub fn materialize_text_or_code(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    context: &MaterializationContext<'_>,
) -> Result<MaterializationProduct, MaterializationError> {
    let budget = context.budget.validate()?;
    if context.cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    if request.profile().id() != context.profile.id() {
        return Err(MaterializationError::ProfileMismatch);
    }
    let profile = &context.profile;
    let guard = open_exact_revision(request, port, context.cancel)?;
    let bytes = guard.into_bytes();
    let decision = detect_or_validate_encoding(&bytes, request.declared_encoding(), profile)?;
    let mut steps = StepCounter::new(budget.effective_steps(profile.limits().max_steps));
    let decoded = decode_text_or_code(
        &bytes,
        &decision,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    if context.cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let canonical =
        normalize_representation(&decoded, profile, &budget, &mut steps, context.cancel)?;
    let identities = MapIdentities {
        source_id: request.source_id().clone(),
        revision: request.revision(),
        profile_id: profile.id(),
    };
    let coordinate_map = build_coordinate_map(
        &decoded,
        &canonical,
        &identities,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    let loss_map = build_loss_map(
        &decoded,
        &canonical,
        &identities,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    let bundle = MapBundle::from_parts(coordinate_map, loss_map);
    let warnings = warnings_for(&bundle, decision.encoding());
    let warnings_count =
        u64::try_from(warnings.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let assurance = derive_assurance(&bundle, warnings_count, profile)?;
    validate_map_bundle(&canonical, &bundle, profile)?;
    let canonical_digest = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/canonical/v1",
        &[canonical.text().as_bytes()],
    ));
    let coordinate_digest = digest_coordinate_map(&bundle);
    let loss_digest = digest_loss_map(&bundle);
    let representation_id = digest_representation(
        request,
        decision.encoding(),
        canonical.text(),
        &coordinate_digest,
        &loss_digest,
    );
    let segments = u64::try_from(bundle.coordinate_map().segments().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let loss_records = u64::try_from(bundle.loss_map().records().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let output_bytes =
        u64::try_from(canonical.text().len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    Ok(MaterializationProduct {
        representation_id,
        source_id: request.source_id().clone(),
        revision: request.revision(),
        profile_id: profile.id(),
        encoding: decision.encoding(),
        input_digest: request.content_digest(),
        canonical_digest,
        coordinate_digest,
        loss_digest,
        canonical,
        maps: bundle,
        assurance,
        warnings,
        resource: ResourceReceipt {
            input_bytes: request.byte_count(),
            output_bytes,
            steps_used: steps.used(),
            segments,
            loss_records,
        },
    })
}

/// Deterministic canonical bytes for descriptors, maps and warnings.
///
/// Source content travels by digest only: technical receipts never embed
/// content or paths.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalMaterializationBytes {
    bytes: Vec<u8>,
}

impl CanonicalMaterializationBytes {
    /// Canonical bytes borrowed without copying.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Canonical byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the canonical bytes are empty (never for valid output).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl core::fmt::Debug for CanonicalMaterializationBytes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CanonicalMaterializationBytes")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Serializes descriptors, maps and warnings deterministically with source
/// content referenced by digest rather than embedded.
pub fn canonicalize_materialization(
    product: &MaterializationProduct,
) -> Result<CanonicalMaterializationBytes, MaterializationError> {
    let mut out = Vec::new();
    out.extend_from_slice(b"ELIOT-MAT-V1");
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(product.profile_id().as_bytes());
    out.extend_from_slice(product.representation_id().as_bytes());
    out.extend_from_slice(product.input_digest().as_bytes());
    out.extend_from_slice(product.canonical_digest().as_bytes());
    out.extend_from_slice(product.coordinate_digest().as_bytes());
    out.extend_from_slice(product.loss_digest().as_bytes());
    out.push(encoding_tag(product.encoding()));
    out.push(product.assurance().ceiling().rank());
    let segments = u64::try_from(product.maps().coordinate_map().segments().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let losses = u64::try_from(product.maps().loss_map().records().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let warnings = u64::try_from(product.warnings().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    push_u64(&mut out, segments);
    push_u64(&mut out, losses);
    push_u64(&mut out, warnings);
    push_u64(&mut out, product.resource_receipt().output_bytes);
    for segment in product.maps().coordinate_map().segments() {
        push_u64(&mut out, segment.native_start);
        push_u64(&mut out, segment.native_end);
        push_u64(&mut out, segment.decoded_start);
        push_u64(&mut out, segment.decoded_end);
        push_u64(&mut out, segment.canonical_start);
        push_u64(&mut out, segment.canonical_end);
        out.push(match segment.relation {
            crate::maps::SegmentRelation::Exact => 1,
            crate::maps::SegmentRelation::Range => 2,
            crate::maps::SegmentRelation::Ambiguous => 3,
            crate::maps::SegmentRelation::Unmapped => 4,
        });
    }
    for record in product.maps().loss_map().records() {
        out.push(match record.kind {
            crate::maps::LossKind::RemovedBom => 1,
            crate::maps::LossKind::TranscodedEncoding => 2,
            crate::maps::LossKind::NormalizedNewline => 3,
        });
        push_u64(&mut out, record.native_start);
        push_u64(&mut out, record.native_end);
        push_u64(&mut out, record.decoded_start);
        push_u64(&mut out, record.decoded_end);
        push_u64(&mut out, record.canonical_start);
        push_u64(&mut out, record.canonical_end);
    }
    for warning in product.warnings() {
        match warning {
            MaterializationWarning::BomStripped => out.push(1),
            MaterializationWarning::Transcoded(encoding) => {
                out.push(2);
                out.push(encoding_tag(*encoding));
            }
            MaterializationWarning::NewlinesNormalized(count) => {
                out.push(3);
                push_u64(&mut out, *count);
            }
        }
    }
    Ok(CanonicalMaterializationBytes { bytes: out })
}

/// Content-free verification receipt: the output belongs to the exact source
/// revision and profile. It cannot prove current filesystem state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationVerificationReceipt {
    representation_id: Blake3Digest32,
    profile_id: MaterializerProfileId,
    coordinate_segments: u64,
    loss_records: u64,
    assurance: MaterializationAssurance,
}

impl MaterializationVerificationReceipt {
    /// Verified representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Verified profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Validated coordinate segment count.
    #[must_use]
    pub const fn coordinate_segments(&self) -> u64 {
        self.coordinate_segments
    }

    /// Validated loss record count.
    #[must_use]
    pub const fn loss_records(&self) -> u64 {
        self.loss_records
    }

    /// Verified assurance bound.
    #[must_use]
    pub const fn assurance(&self) -> MaterializationAssurance {
        self.assurance
    }
}

/// Recomputes identities and digests, validates maps and assurance, and
/// proves the output belongs to the exact source revision and profile.
pub fn verify_materialization(
    product: &MaterializationProduct,
    request: &ValidatedMaterializationRequest,
    profile: &ValidatedMaterializerProfile,
) -> Result<MaterializationVerificationReceipt, MaterializationError> {
    if profile.id() != request.profile().id() || profile.id() != product.profile_id() {
        return Err(MaterializationError::ProfileMismatch);
    }
    if request.content_digest() != product.input_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    if request.revision() != product.revision() || *request.source_id() != *product.source_id() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    let canonical_digest = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/canonical/v1",
        &[product.canonical_text().as_bytes()],
    ));
    if canonical_digest != product.canonical_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    if digest_coordinate_map(product.maps()) != product.coordinate_digest()
        || digest_loss_map(product.maps()) != product.loss_digest()
    {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    let receipt = validate_map_bundle(product.canonical(), product.maps(), profile)?;
    let warnings = u64::try_from(product.warnings().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let assurance = derive_assurance(product.maps(), warnings, profile)?;
    if assurance.ceiling() != product.assurance().ceiling() {
        return Err(MaterializationError::AssuranceViolation);
    }
    Ok(MaterializationVerificationReceipt {
        representation_id: product.representation_id(),
        profile_id: product.profile_id(),
        coordinate_segments: receipt.coordinate_segments(),
        loss_records: receipt.loss_records(),
        assurance,
    })
}

/// Content-addressed immutable publication plan.
///
/// Prepared for the revision/preparation owner. This plan does not write CAS
/// or control state and does not claim admission; unknown artifact-write
/// outcomes remain the caller's operation and readback responsibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationAdmissionPlan {
    representation_id: Blake3Digest32,
    canonical_digest: Blake3Digest32,
    profile_id: MaterializerProfileId,
    canonical_bytes_len: u64,
    operation_id: OpaqueId,
}

impl MaterializationAdmissionPlan {
    /// Planned representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Planned canonical content digest.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Planned profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Planned canonical byte length.
    #[must_use]
    pub const fn canonical_bytes_len(&self) -> u64 {
        self.canonical_bytes_len
    }

    /// Owning operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }
}

/// Creates a content-addressed immutable publication plan without writing.
pub fn prepare_admission(
    product: &MaterializationProduct,
    operation_id: &OpaqueId,
    deadline_steps: u64,
) -> Result<MaterializationAdmissionPlan, MaterializationError> {
    if deadline_steps == 0 {
        return Err(MaterializationError::RequestInvalid);
    }
    Ok(MaterializationAdmissionPlan {
        representation_id: product.representation_id(),
        canonical_digest: product.canonical_digest(),
        profile_id: product.profile_id(),
        canonical_bytes_len: product.resource_receipt().output_bytes,
        operation_id: operation_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::baseline_profile_descriptor;
    use crate::request::{
        AcceptedProfiles, DEFAULT_MATERIALIZATION_BUDGET, MaterializationRequest,
        validate_materialization_request,
    };

    fn profile() -> ValidatedMaterializerProfile {
        crate::profile::validate_materializer_profile(&baseline_profile_descriptor(
            "product-test",
            1,
        ))
        .expect("profile")
    }

    struct FakePort {
        bytes: Vec<u8>,
        digest: Blake3Digest32,
        residency: OpaqueId,
    }

    impl RevisionReadPort for FakePort {
        fn read_exact(
            &self,
            _source: &OpaqueId,
            _revision: NonZeroRevision,
            _byte_count: u64,
        ) -> Result<StoredRevisionBytes, MaterializationError> {
            Ok(StoredRevisionBytes::new(
                self.bytes.clone(),
                self.digest,
                self.residency.clone(),
            ))
        }
    }

    struct FailingPort;

    impl RevisionReadPort for FailingPort {
        fn read_exact(
            &self,
            _source: &OpaqueId,
            _revision: NonZeroRevision,
            _byte_count: u64,
        ) -> Result<StoredRevisionBytes, MaterializationError> {
            Err(MaterializationError::RevisionUnavailable)
        }
    }

    fn digest_for(bytes: &[u8]) -> Blake3Digest32 {
        let mut out = [0_u8; 32];
        for (index, byte) in bytes.iter().enumerate() {
            out[index % 32] ^= byte.wrapping_add(index.to_le_bytes()[0]);
        }
        Blake3Digest32::from_bytes(out)
    }

    fn validated(
        bytes: &[u8],
        profile: &ValidatedMaterializerProfile,
    ) -> ValidatedMaterializationRequest {
        let request = MaterializationRequest {
            source_id: OpaqueId::new("source:test").expect("source"),
            revision: NonZeroRevision::new(1).expect("revision"),
            residency: OpaqueId::new("residency:test").expect("residency"),
            content_digest: digest_for(bytes),
            byte_count: u64::try_from(bytes.len()).expect("len"),
            declared_kind: crate::profile::SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: profile.id(),
            operation_id: OpaqueId::new("operation:test").expect("operation"),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        validate_materialization_request(
            &request,
            &AcceptedProfiles::new(vec![profile.clone()]),
            &DEFAULT_MATERIALIZATION_BUDGET,
        )
        .expect("request")
    }

    fn materialize(bytes: &[u8]) -> MaterializationProduct {
        let profile = profile();
        let request = validated(bytes, &profile);
        let port = FakePort {
            bytes: bytes.to_vec(),
            digest: request.content_digest(),
            residency: request.residency().clone(),
        };
        let context = MaterializationContext {
            profile,
            budget: DEFAULT_MATERIALIZATION_BUDGET,
            cancel: CancellationToken::never(),
        };
        materialize_text_or_code(&request, &port, &context).expect("materialize")
    }

    #[test]
    fn revision_open_binds_digest_residency_length() {
        let profile = profile();
        let bytes = b"exact\n".as_slice();
        let request = validated(bytes, &profile);
        let guard = open_exact_revision(
            &request,
            &FakePort {
                bytes: bytes.to_vec(),
                digest: request.content_digest(),
                residency: request.residency().clone(),
            },
            CancellationToken::never(),
        )
        .expect("open");
        assert_eq!(guard.bytes(), bytes);
        assert_eq!(
            open_exact_revision(&request, &FailingPort, CancellationToken::never()),
            Err(MaterializationError::RevisionUnavailable)
        );
        let wrong_digest = FakePort {
            bytes: bytes.to_vec(),
            digest: Blake3Digest32::from_bytes([9; 32]),
            residency: request.residency().clone(),
        };
        assert_eq!(
            open_exact_revision(&request, &wrong_digest, CancellationToken::never()),
            Err(MaterializationError::RevisionDigestMismatch)
        );
        let wrong_residency = FakePort {
            bytes: bytes.to_vec(),
            digest: request.content_digest(),
            residency: OpaqueId::new("residency:other").expect("residency"),
        };
        assert_eq!(
            open_exact_revision(&request, &wrong_residency, CancellationToken::never()),
            Err(MaterializationError::ResidencyMismatch)
        );
    }

    #[test]
    fn end_to_end_product_is_deterministic() {
        let first = materialize(b"same\nbytes\n");
        let second = materialize(b"same\nbytes\n");
        assert_eq!(first.representation_id(), second.representation_id());
        assert_eq!(first.canonical_digest(), second.canonical_digest());
        assert_eq!(
            canonicalize_materialization(&first)
                .expect("canonical")
                .as_slice(),
            canonicalize_materialization(&second)
                .expect("canonical")
                .as_slice()
        );
        let changed = materialize(b"same\nBYTES\n");
        assert_ne!(first.representation_id(), changed.representation_id());
    }

    #[test]
    fn verify_proves_revision_and_profile_binding() {
        let profile = profile();
        let bytes = b"verify\nme\n".as_slice();
        let request = validated(bytes, &profile);
        let product = materialize(bytes);
        let receipt = verify_materialization(&product, &request, &profile).expect("verify");
        assert_eq!(receipt.representation_id(), product.representation_id());
        assert_eq!(receipt.assurance().ceiling(), product.assurance().ceiling());
        let other_profile = crate::profile::validate_materializer_profile(
            &baseline_profile_descriptor("other-verify", 1),
        )
        .expect("other");
        assert_eq!(
            verify_materialization(&product, &request, &other_profile),
            Err(MaterializationError::ProfileMismatch)
        );
    }

    #[test]
    fn admission_plan_is_content_addressed_and_deadline_bound() {
        let product = materialize(b"plan\n");
        let operation = OpaqueId::new("operation:plan").expect("operation");
        let plan = prepare_admission(&product, &operation, 100).expect("plan");
        assert_eq!(plan.representation_id(), product.representation_id());
        assert_eq!(plan.canonical_digest(), product.canonical_digest());
        assert_eq!(
            prepare_admission(&product, &operation, 0),
            Err(MaterializationError::RequestInvalid)
        );
    }

    #[test]
    fn resource_receipt_reports_bounded_work() {
        let product = materialize(b"work\nreport\n");
        let resource = product.resource_receipt();
        assert_eq!(resource.input_bytes, 12);
        assert_eq!(resource.output_bytes, 12);
        assert!(resource.steps_used > 0);
        assert_eq!(resource.segments, 1);
        assert_eq!(resource.loss_records, 0);
    }

    #[test]
    fn debug_views_stay_content_free() {
        let product = materialize(b"top-secret-bytes\n");
        assert!(!format!("{product:?}").contains("top-secret-bytes"));
        let canonical = canonicalize_materialization(&product).expect("canonical");
        assert!(!format!("{canonical:?}").contains("top-secret"));
    }
}
