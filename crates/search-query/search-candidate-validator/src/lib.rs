//! Source-backed validation of untrusted candidate nominations.
//!
//! A nomination can enter evidence-bearing result fields only after exact
//! retained-revision readback, digest/range/profile verification, and a final
//! live security recheck. Cached snippets and Qdrant payload text are ignored.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeSet;

use search_access::{
    recheck_live_access, AccessCheckpoint, AccessError, AccessPermit, LiveSecurityState,
    RequestSecurityFence,
};
use search_contracts::{
    blake3_canonical, blake3_exact_bytes, Blake3Digest32, Blake3ExactInputLimit, BoundedBytes,
    BoundedList, CandidateId, CanonicalDigestDomain, CanonicalValue, CollectionGenerationId,
    DigestInputLimit, Epoch, ObjectResidencyKeyDigest, ProfileId, ReceiptRef, RepresentationId,
    RequestId, SourceMembership, SourceMembershipId, SourceOwnerFence, SourceOwnerGeneration,
    SourceRevision, SourceRevisionId, SourceRevisionRef, UnitId, MAX_CANONICAL_BYTES,
};
use search_unitizer::{
    raw_v3_unit_content_digest, V3AnchorPolicy, V3AttachmentPolicy, V3OmissionPolicy,
    V3OverlapPolicy, VerifiedUnitSet,
};

const CANDIDATE_VALIDATION_RECEIPT_DOMAIN: &str = "eliot/cbor/candidate-validation-receipt/v1";

/// Closed candidate-validation failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValidationError {
    RequestMismatch,
    PlanMismatch,
    LegContaminated,
    MembershipDenied,
    CollectionMismatch,
    ProfileMismatch,
    EpochInvalid,
    OverlayShadowed,
    CandidateBudgetExceeded,
    RevisionUnavailable,
    SourceUnreadable,
    ReadbackTimeout,
    RevisionMismatch,
    RepresentationMismatch,
    UnitMismatch,
    ContentDigestMismatch,
    LengthMismatch,
    AnchorOutOfBounds,
    ResidencyDenied,
    CoordinateMapMismatch,
    AssuranceInsufficient,
    DisclosureExceeded,
    AccessRevoked,
    Purged,
    EmissionFenceStale,
    OwnerMetadataRequired,
    OwnerBindingMismatch,
    InvalidScore,
    ReceiptComputationFailed,
}

impl ValidationError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RequestMismatch => "VALIDATION_REQUEST_MISMATCH",
            Self::PlanMismatch => "VALIDATION_PLAN_MISMATCH",
            Self::LegContaminated => "VALIDATION_LEG_CONTAMINATED",
            Self::MembershipDenied => "VALIDATION_MEMBERSHIP_DENIED",
            Self::CollectionMismatch => "VALIDATION_COLLECTION_MISMATCH",
            Self::ProfileMismatch => "VALIDATION_PROFILE_MISMATCH",
            Self::EpochInvalid => "VALIDATION_EPOCH_INVALID",
            Self::OverlayShadowed => "VALIDATION_OVERLAY_SHADOWED",
            Self::CandidateBudgetExceeded => "VALIDATION_BUDGET_EXCEEDED",
            Self::RevisionUnavailable => "VALIDATION_REVISION_UNAVAILABLE",
            Self::SourceUnreadable => "VALIDATION_SOURCE_UNREADABLE",
            Self::ReadbackTimeout => "VALIDATION_READBACK_TIMEOUT",
            Self::RevisionMismatch => "VALIDATION_REVISION_MISMATCH",
            Self::RepresentationMismatch => "VALIDATION_REPRESENTATION_MISMATCH",
            Self::UnitMismatch => "VALIDATION_UNIT_MISMATCH",
            Self::ContentDigestMismatch => "VALIDATION_CONTENT_DIGEST_MISMATCH",
            Self::LengthMismatch => "VALIDATION_LENGTH_MISMATCH",
            Self::AnchorOutOfBounds => "VALIDATION_ANCHOR_OUT_OF_BOUNDS",
            Self::ResidencyDenied => "VALIDATION_RESIDENCY_DENIED",
            Self::CoordinateMapMismatch => "VALIDATION_COORDINATE_MAP_MISMATCH",
            Self::AssuranceInsufficient => "VALIDATION_ASSURANCE_INSUFFICIENT",
            Self::DisclosureExceeded => "VALIDATION_DISCLOSURE_EXCEEDED",
            Self::AccessRevoked => "VALIDATION_ACCESS_REVOKED",
            Self::Purged => "VALIDATION_PURGED",
            Self::EmissionFenceStale => "VALIDATION_EMISSION_FENCE_STALE",
            Self::OwnerMetadataRequired => "VALIDATION_OWNER_METADATA_REQUIRED",
            Self::OwnerBindingMismatch => "VALIDATION_OWNER_BINDING_MISMATCH",
            Self::InvalidScore => "VALIDATION_INVALID_SCORE",
            Self::ReceiptComputationFailed => "VALIDATION_RECEIPT_COMPUTATION_FAILED",
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ValidationError {}

impl From<AccessError> for ValidationError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::LivePurge => Self::Purged,
            AccessError::LiveRevocation | AccessError::SecurityFailClosed => Self::AccessRevoked,
            _ => Self::EmissionFenceStale,
        }
    }
}

/// Assurance floor expected from exact readback.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CandidateAssurance {
    Derived,
    SourceBacked,
    Exact,
}

/// Untrusted candidate nomination from an executed leg.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateNomination {
    pub request_id: RequestId,
    pub plan_digest: [u8; 32],
    pub leg_id: usize,
    pub point_id: [u8; 16],
    pub source_membership_id: SourceMembershipId,
    pub collection_generation_id: CollectionGenerationId,
    pub valid_from_epoch: Epoch,
    pub valid_until_epoch_exclusive: Option<Epoch>,
    pub profile_id: ProfileId,
    pub profile_digest: Blake3Digest32,
    pub source_revision_id: SourceRevisionId,
    pub representation_id: RepresentationId,
    pub unit_id: UnitId,
    pub unit_byte_start: u64,
    pub unit_byte_end: u64,
    pub expected_content_digest: Blake3Digest32,
    pub expected_unit_digest: Blake3Digest32,
    /// Expected exact-byte excerpt digest, absent only for the versioned
    /// whole-unit raw-v3 verification path.
    pub expected_excerpt_digest: Option<Blake3Digest32>,
    pub expected_assurance: CandidateAssurance,
    pub expected_residency_digest: Blake3Digest32,
    pub raw_score: f32,
}

/// Captured validation context before source readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationContext {
    pub request_id: RequestId,
    pub plan_digest: [u8; 32],
    pub collection_generation_id: CollectionGenerationId,
    pub visible_epoch: Epoch,
    pub allowed_memberships: BTreeSet<SourceMembershipId>,
    pub shadowed_units: BTreeSet<UnitId>,
    pub max_candidates: usize,
    pub candidate_ordinal: usize,
    pub request_security: RequestSecurityFence,
    pub live_security: LiveSecurityState,
    pub contaminated_legs: BTreeSet<usize>,
}

/// Result of validation before source bytes are read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationPrecheck {
    Proceed,
    Gap(ValidationError),
    ContaminatedLeg,
}

/// Checks all load-bearing metadata available before source readback.
#[must_use]
pub fn precheck(
    nomination: &CandidateNomination,
    context: &ValidationContext,
) -> ValidationPrecheck {
    if nomination.request_id != context.request_id {
        return ValidationPrecheck::Gap(ValidationError::RequestMismatch);
    }
    if nomination.plan_digest != context.plan_digest {
        return ValidationPrecheck::Gap(ValidationError::PlanMismatch);
    }
    if context.contaminated_legs.contains(&nomination.leg_id) {
        return ValidationPrecheck::ContaminatedLeg;
    }
    if context.candidate_ordinal >= context.max_candidates {
        return ValidationPrecheck::Gap(ValidationError::CandidateBudgetExceeded);
    }
    if !context
        .allowed_memberships
        .contains(&nomination.source_membership_id)
    {
        return ValidationPrecheck::Gap(ValidationError::MembershipDenied);
    }
    if nomination.collection_generation_id != context.collection_generation_id {
        return ValidationPrecheck::Gap(ValidationError::CollectionMismatch);
    }
    if nomination.valid_from_epoch > context.visible_epoch
        || nomination
            .valid_until_epoch_exclusive
            .is_some_and(|until| context.visible_epoch >= until)
    {
        return ValidationPrecheck::Gap(ValidationError::EpochInvalid);
    }
    if context.shadowed_units.contains(&nomination.unit_id) {
        return ValidationPrecheck::Gap(ValidationError::OverlayShadowed);
    }
    if recheck_live_access(
        &context.request_security,
        &context.live_security,
        AccessCheckpoint::BeforeSourceReadback,
    )
    .is_err()
    {
        return ValidationPrecheck::Gap(ValidationError::AccessRevoked);
    }
    ValidationPrecheck::Proceed
}

/// Exact immutable revision readback request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRevisionReadbackRequest {
    pub source_membership_id: SourceMembershipId,
    pub source_revision_id: SourceRevisionId,
    pub representation_id: RepresentationId,
    pub unit_id: UnitId,
    pub byte_start: u64,
    pub byte_end: u64,
    pub expected_content_digest: Blake3Digest32,
    pub expected_unit_digest: Blake3Digest32,
    /// Expected exact-byte excerpt digest, absent only for the versioned
    /// whole-unit raw-v3 verification path.
    pub expected_excerpt_digest: Option<Blake3Digest32>,
    pub expected_profile_id: ProfileId,
    pub expected_profile_digest: Blake3Digest32,
    pub expected_residency_digest: Blake3Digest32,
    pub expected_assurance: CandidateAssurance,
    pub max_read_bytes: u64,
}

/// Builds a readback request that cannot substitute the latest path/revision.
pub fn build_readback_request(
    nomination: &CandidateNomination,
    max_read_bytes: u64,
) -> Result<ExactRevisionReadbackRequest, ValidationError> {
    if nomination.unit_byte_start >= nomination.unit_byte_end
        || nomination
            .unit_byte_end
            .saturating_sub(nomination.unit_byte_start)
            > max_read_bytes
        || max_read_bytes == 0
    {
        return Err(ValidationError::AnchorOutOfBounds);
    }
    Ok(ExactRevisionReadbackRequest {
        source_membership_id: nomination.source_membership_id,
        source_revision_id: nomination.source_revision_id,
        representation_id: nomination.representation_id,
        unit_id: nomination.unit_id,
        byte_start: nomination.unit_byte_start,
        byte_end: nomination.unit_byte_end,
        expected_content_digest: nomination.expected_content_digest,
        expected_unit_digest: nomination.expected_unit_digest,
        expected_excerpt_digest: nomination.expected_excerpt_digest,
        expected_profile_id: nomination.profile_id.clone(),
        expected_profile_digest: nomination.profile_digest,
        expected_residency_digest: nomination.expected_residency_digest,
        expected_assurance: nomination.expected_assurance,
        max_read_bytes,
    })
}

/// Exact source revision readback returned by an injected port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReadback {
    pub source_membership_id: SourceMembershipId,
    pub source_revision_id: SourceRevisionId,
    pub representation_id: RepresentationId,
    pub unit_id: UnitId,
    pub content_digest: Blake3Digest32,
    pub unit_digest: Blake3Digest32,
    pub excerpt_digest: Blake3Digest32,
    pub profile_id: ProfileId,
    pub profile_digest: Blake3Digest32,
    pub residency_digest: Blake3Digest32,
    pub assurance: CandidateAssurance,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
    pub coordinate_map_exact: bool,
    pub residency_authorized: bool,
    pub bytes: Vec<u8>,
}

/// Vendor-neutral exact revision readback seam.
pub trait RevisionReadbackPort {
    type Error;

    fn read_exact(
        &mut self,
        request: &ExactRevisionReadbackRequest,
    ) -> Result<SourceReadback, Self::Error>;
}

/// Source-backed verified slice. Fields are private so raw nominations cannot
/// be inserted into the evidence type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSourceSlice {
    source_membership_id: SourceMembershipId,
    source_revision_id: SourceRevisionId,
    representation_id: RepresentationId,
    unit_id: UnitId,
    byte_start: u64,
    byte_end: u64,
    content_digest: Blake3Digest32,
    unit_digest: Blake3Digest32,
    excerpt_digest: Blake3Digest32,
    profile_id: ProfileId,
    profile_digest: Blake3Digest32,
    residency_digest: Blake3Digest32,
    assurance: CandidateAssurance,
    bytes: Vec<u8>,
}

impl VerifiedSourceSlice {
    #[must_use]
    pub const fn source_membership_id(&self) -> SourceMembershipId {
        self.source_membership_id
    }

    #[must_use]
    pub const fn source_revision_id(&self) -> SourceRevisionId {
        self.source_revision_id
    }

    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation_id
    }

    #[must_use]
    pub const fn unit_id(&self) -> UnitId {
        self.unit_id
    }

    #[must_use]
    pub const fn byte_range(&self) -> (u64, u64) {
        (self.byte_start, self.byte_end)
    }

    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    #[must_use]
    pub const fn unit_digest(&self) -> Blake3Digest32 {
        self.unit_digest
    }

    #[must_use]
    pub const fn excerpt_digest(&self) -> Blake3Digest32 {
        self.excerpt_digest
    }

    /// Exact profile identity checked against the readback request.
    #[must_use]
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    /// Exact profile digest checked against the readback request.
    #[must_use]
    pub const fn profile_digest(&self) -> Blake3Digest32 {
        self.profile_digest
    }

    /// Residency digest checked against the readback request.
    #[must_use]
    pub const fn residency_digest(&self) -> Blake3Digest32 {
        self.residency_digest
    }

    #[must_use]
    pub const fn assurance(&self) -> CandidateAssurance {
        self.assurance
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Reopens and verifies one legacy nomination with an owner-supplied excerpt digest.
///
/// This entrypoint refuses nominations without `expected_excerpt_digest`. Use
/// [`reopen_and_verify_raw_v3_full_unit_v1`] only when the caller has bound the
/// request range to one complete raw-v3 unit from a verified unit set.
pub fn reopen_and_verify<P: RevisionReadbackPort>(
    request: &ExactRevisionReadbackRequest,
    port: &mut P,
) -> Result<VerifiedSourceSlice, ValidationError> {
    if request.expected_excerpt_digest.is_none() {
        return Err(ValidationError::ContentDigestMismatch);
    }
    let readback = port
        .read_exact(request)
        .map_err(|_| ValidationError::SourceUnreadable)?;
    verify_source_slice(request, readback)
}

/// Reopens one complete raw-v3 unit and derives its standard excerpt digest.
///
/// The caller must set `byte_start..byte_end` to the complete unit span from the
/// matching verified raw-v3 unit set. This API verifies that the returned bytes
/// have the owner's v3 unit-content digest and computes the separate standard
/// exact-byte excerpt digest. It cannot determine from this request alone that
/// the profile is accepted or that an arbitrary subspan is a complete unit.
pub fn reopen_and_verify_raw_v3_full_unit_v1<P: RevisionReadbackPort>(
    request: &ExactRevisionReadbackRequest,
    port: &mut P,
) -> Result<VerifiedSourceSlice, ValidationError> {
    if request.expected_assurance != CandidateAssurance::Exact
        || !raw_v3_profile_matches(
            &request.expected_profile_id,
            request.expected_profile_digest,
        )
    {
        return Err(ValidationError::ProfileMismatch);
    }
    let readback = port
        .read_exact(request)
        .map_err(|_| ValidationError::SourceUnreadable)?;
    verify_source_slice_raw_v3_full_unit_v1(request, readback)
}

fn validate_readback_metadata(
    request: &ExactRevisionReadbackRequest,
    readback: &SourceReadback,
) -> Result<usize, ValidationError> {
    if readback.source_membership_id != request.source_membership_id
        || readback.source_revision_id != request.source_revision_id
    {
        return Err(ValidationError::RevisionMismatch);
    }
    if readback.representation_id != request.representation_id {
        return Err(ValidationError::RepresentationMismatch);
    }
    if readback.unit_id != request.unit_id {
        return Err(ValidationError::UnitMismatch);
    }
    if readback.content_digest != request.expected_content_digest {
        return Err(ValidationError::ContentDigestMismatch);
    }
    if readback.profile_id != request.expected_profile_id
        || readback.profile_digest != request.expected_profile_digest
    {
        return Err(ValidationError::ProfileMismatch);
    }
    if readback.residency_digest != request.expected_residency_digest
        || !readback.residency_authorized
    {
        return Err(ValidationError::ResidencyDenied);
    }
    if readback.assurance < request.expected_assurance {
        return Err(ValidationError::AssuranceInsufficient);
    }
    if readback.source_byte_start != request.byte_start
        || readback.source_byte_end != request.byte_end
        || readback.source_byte_start >= readback.source_byte_end
        || !readback.coordinate_map_exact
    {
        return Err(ValidationError::CoordinateMapMismatch);
    }
    let expected_length = usize::try_from(request.byte_end - request.byte_start)
        .map_err(|_| ValidationError::LengthMismatch)?;
    if readback.bytes.len() != expected_length
        || u64::try_from(readback.bytes.len()).unwrap_or(u64::MAX) > request.max_read_bytes
    {
        return Err(ValidationError::LengthMismatch);
    }
    Ok(expected_length)
}

fn verify_source_slice(
    request: &ExactRevisionReadbackRequest,
    readback: SourceReadback,
) -> Result<VerifiedSourceSlice, ValidationError> {
    let expected_excerpt_digest = request
        .expected_excerpt_digest
        .ok_or(ValidationError::ContentDigestMismatch)?;
    validate_readback_metadata(request, &readback)?;
    if readback.unit_digest != request.expected_unit_digest
        || readback.excerpt_digest != expected_excerpt_digest
    {
        return Err(ValidationError::ContentDigestMismatch);
    }
    Ok(VerifiedSourceSlice {
        source_membership_id: readback.source_membership_id,
        source_revision_id: readback.source_revision_id,
        representation_id: readback.representation_id,
        unit_id: readback.unit_id,
        byte_start: readback.source_byte_start,
        byte_end: readback.source_byte_end,
        content_digest: readback.content_digest,
        unit_digest: readback.unit_digest,
        excerpt_digest: readback.excerpt_digest,
        profile_id: readback.profile_id,
        profile_digest: readback.profile_digest,
        residency_digest: readback.residency_digest,
        assurance: readback.assurance,
        bytes: readback.bytes,
    })
}

fn verify_source_slice_raw_v3_full_unit_v1(
    request: &ExactRevisionReadbackRequest,
    readback: SourceReadback,
) -> Result<VerifiedSourceSlice, ValidationError> {
    if request.expected_assurance != CandidateAssurance::Exact
        || !raw_v3_profile_matches(
            &request.expected_profile_id,
            request.expected_profile_digest,
        )
    {
        return Err(ValidationError::ProfileMismatch);
    }
    let expected_length = validate_readback_metadata(request, &readback)?;
    let unit_digest = raw_v3_unit_content_digest(&readback.bytes, expected_length)
        .map_err(|_| ValidationError::LengthMismatch)?;
    if unit_digest != request.expected_unit_digest || readback.unit_digest != unit_digest {
        return Err(ValidationError::ContentDigestMismatch);
    }
    let excerpt_limit =
        Blake3ExactInputLimit::new(expected_length).map_err(|_| ValidationError::LengthMismatch)?;
    let excerpt_digest = blake3_exact_bytes(&readback.bytes, excerpt_limit)
        .map_err(|_| ValidationError::LengthMismatch)?;
    // The returned excerpt-digest field is legacy metadata. The raw-v3 path
    // derives the verified value from the returned bytes and compares only the
    // nomination's optional expected digest against that computation.
    if request
        .expected_excerpt_digest
        .is_some_and(|expected| expected != excerpt_digest)
    {
        return Err(ValidationError::ContentDigestMismatch);
    }
    Ok(VerifiedSourceSlice {
        source_membership_id: readback.source_membership_id,
        source_revision_id: readback.source_revision_id,
        representation_id: readback.representation_id,
        unit_id: readback.unit_id,
        byte_start: readback.source_byte_start,
        byte_end: readback.source_byte_end,
        content_digest: readback.content_digest,
        unit_digest,
        excerpt_digest,
        profile_id: readback.profile_id,
        profile_digest: readback.profile_digest,
        residency_digest: readback.residency_digest,
        assurance: readback.assurance,
        bytes: readback.bytes,
    })
}

fn raw_v3_profile_matches(profile_id: &ProfileId, profile_digest: Blake3Digest32) -> bool {
    profile_id
        .as_str()
        .strip_prefix("unitizer-v3-")
        .and_then(|digest| Blake3Digest32::parse_hex(digest).ok())
        == Some(profile_digest)
}

/// Final emission permit bound to live security state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmissionPermit {
    pub access_permit: AccessPermit,
    pub source_membership_id: SourceMembershipId,
    pub source_revision_id: SourceRevisionId,
}

/// Rechecks live security immediately before result projection.
pub fn recheck_before_emission(
    verified: &VerifiedSourceSlice,
    request_security: &RequestSecurityFence,
    live_security: &LiveSecurityState,
) -> Result<EmissionPermit, ValidationError> {
    let permit = recheck_live_access(
        request_security,
        live_security,
        AccessCheckpoint::BeforeResultEmission,
    )?;
    Ok(EmissionPermit {
        access_permit: permit,
        source_membership_id: verified.source_membership_id,
        source_revision_id: verified.source_revision_id,
    })
}

/// Owner-derived validated candidate with a canonical validation receipt.
///
/// Fields are private so a caller cannot mutate evidence after validation.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSearchCandidate {
    candidate_id: CandidateId,
    request_id: RequestId,
    plan_digest: [u8; 32],
    leg_id: usize,
    raw_score: f32,
    source: VerifiedSourceSlice,
    emission_permit: EmissionPermit,
    source_revision_ref: SourceRevisionRef,
    source_owner_fence: SourceOwnerFence,
    object_residency_key_digest: ObjectResidencyKeyDigest,
    candidate_validation_receipt_ref: ReceiptRef,
}

impl ValidatedSearchCandidate {
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    #[must_use]
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan_digest
    }

    #[must_use]
    pub const fn candidate_id(&self) -> CandidateId {
        self.candidate_id
    }

    #[must_use]
    pub const fn leg_id(&self) -> usize {
        self.leg_id
    }

    #[must_use]
    pub const fn raw_score(&self) -> f32 {
        self.raw_score
    }

    #[must_use]
    pub const fn source(&self) -> &VerifiedSourceSlice {
        &self.source
    }

    #[must_use]
    pub const fn emission_permit(&self) -> EmissionPermit {
        self.emission_permit
    }

    #[must_use]
    pub const fn source_revision_ref(&self) -> SourceRevisionRef {
        self.source_revision_ref
    }

    #[must_use]
    pub const fn source_owner_generation(&self) -> SourceOwnerGeneration {
        self.source_owner_fence.source_owner_generation
    }

    #[must_use]
    pub const fn source_owner_fence(&self) -> SourceOwnerFence {
        self.source_owner_fence
    }

    #[must_use]
    pub const fn object_residency_key_digest(&self) -> ObjectResidencyKeyDigest {
        self.object_residency_key_digest
    }

    #[must_use]
    pub const fn candidate_validation_receipt_ref(&self) -> &ReceiptRef {
        &self.candidate_validation_receipt_ref
    }
}

/// Content-free validation gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateValidationGap {
    pub leg_id: usize,
    pub source_membership_id: SourceMembershipId,
    pub reason: ValidationError,
}

/// Replan signal for a restrictively contaminated leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplanSignal {
    pub leg_id: usize,
}

/// Exactly one validation outcome.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationOutcome {
    Validated(ValidatedSearchCandidate),
    Gap(CandidateValidationGap),
    ContaminatedLeg(ReplanSignal),
}

/// Legacy validation API without source-owner metadata; always fails closed.
///
/// Use [`validate_raw_v3_full_unit_v1`] when the caller can provide the
/// canonical C09/C03 source, membership, owner-generation, and verified-unit
/// inputs. This API never reads source bytes or creates a validated candidate.
pub fn validate<P: RevisionReadbackPort>(
    nomination: &CandidateNomination,
    context: &ValidationContext,
    _max_read_bytes: u64,
    _port: &mut P,
    _final_live_security: &LiveSecurityState,
) -> ValidationOutcome {
    match precheck(nomination, context) {
        ValidationPrecheck::ContaminatedLeg => ValidationOutcome::ContaminatedLeg(ReplanSignal {
            leg_id: nomination.leg_id,
        }),
        ValidationPrecheck::Gap(reason) => ValidationOutcome::Gap(CandidateValidationGap {
            leg_id: nomination.leg_id,
            source_membership_id: nomination.source_membership_id,
            reason,
        }),
        ValidationPrecheck::Proceed => ValidationOutcome::Gap(CandidateValidationGap {
            leg_id: nomination.leg_id,
            source_membership_id: nomination.source_membership_id,
            reason: ValidationError::OwnerMetadataRequired,
        }),
    }
}

/// Complete owner-qualified validation for one whole-unit raw-v3 nomination.
///
/// The caller supplies the candidate ID and typed C09/C03 owner records. Their
/// source, membership, revision, profile, residency, and exact unit range are
/// compared before the readback port is called. A successful value carries a
/// content-bound receipt computed after the final live permit is issued.
pub fn validate_raw_v3_full_unit_v1<P: RevisionReadbackPort>(
    nomination: &CandidateNomination,
    context: &ValidationContext,
    candidate_id: CandidateId,
    source_membership: &SourceMembership,
    source_revision: &SourceRevision,
    source_revision_ref: &SourceRevisionRef,
    source_owner_fence: SourceOwnerFence,
    unit_set: &VerifiedUnitSet,
    max_read_bytes: u64,
    port: &mut P,
    final_live_security: &LiveSecurityState,
) -> ValidationOutcome {
    match precheck(nomination, context) {
        ValidationPrecheck::ContaminatedLeg => {
            return ValidationOutcome::ContaminatedLeg(ReplanSignal {
                leg_id: nomination.leg_id,
            });
        }
        ValidationPrecheck::Gap(reason) => {
            return ValidationOutcome::Gap(CandidateValidationGap {
                leg_id: nomination.leg_id,
                source_membership_id: nomination.source_membership_id,
                reason,
            });
        }
        ValidationPrecheck::Proceed => {}
    }
    if !raw_v3_owner_binding_matches(
        nomination,
        context,
        source_membership,
        source_revision,
        source_revision_ref,
        source_owner_fence,
        unit_set,
    ) {
        return ValidationOutcome::ContaminatedLeg(ReplanSignal {
            leg_id: nomination.leg_id,
        });
    }
    if !nomination.raw_score.is_finite() {
        return ValidationOutcome::Gap(CandidateValidationGap {
            leg_id: nomination.leg_id,
            source_membership_id: nomination.source_membership_id,
            reason: ValidationError::InvalidScore,
        });
    }

    let result = build_readback_request(nomination, max_read_bytes)
        .and_then(|request| reopen_and_verify_raw_v3_full_unit_v1(&request, port))
        .and_then(|source| {
            if source.profile_id != *unit_set.manifest().profile_id()
                || source.profile_digest != nomination.profile_digest
                || source.residency_digest.as_bytes()
                    != source_revision.object_residency_key_digest.as_bytes()
            {
                return Err(ValidationError::OwnerBindingMismatch);
            }
            let emission_permit =
                recheck_before_emission(&source, &context.request_security, final_live_security)?;
            let receipt = candidate_validation_receipt_ref(
                candidate_id,
                nomination,
                context,
                source_membership,
                source_revision,
                source_revision_ref,
                source_owner_fence,
                &source,
                emission_permit,
            )?;
            Ok(ValidatedSearchCandidate {
                candidate_id,
                request_id: nomination.request_id,
                plan_digest: nomination.plan_digest,
                leg_id: nomination.leg_id,
                raw_score: nomination.raw_score,
                source,
                emission_permit,
                source_revision_ref: *source_revision_ref,
                source_owner_fence,
                object_residency_key_digest: source_revision.object_residency_key_digest,
                candidate_validation_receipt_ref: receipt,
            })
        });
    match result {
        Ok(candidate) => ValidationOutcome::Validated(candidate),
        Err(reason) => ValidationOutcome::Gap(CandidateValidationGap {
            leg_id: nomination.leg_id,
            source_membership_id: nomination.source_membership_id,
            reason,
        }),
    }
}

fn raw_v3_owner_binding_matches(
    nomination: &CandidateNomination,
    context: &ValidationContext,
    source_membership: &SourceMembership,
    source_revision: &SourceRevision,
    source_revision_ref: &SourceRevisionRef,
    source_owner_fence: SourceOwnerFence,
    unit_set: &VerifiedUnitSet,
) -> bool {
    let manifest = unit_set.manifest();
    let provenance = manifest.provenance();
    let binding = provenance.binding();
    let Some(unit) = unit_set
        .units()
        .iter()
        .find(|unit| unit.unit_id() == nomination.unit_id)
    else {
        return false;
    };
    let profile = manifest.profile();
    let profile_id = manifest.profile_id();
    let profile_digest = profile_id
        .as_str()
        .strip_prefix("unitizer-v3-")
        .and_then(|digest| Blake3Digest32::parse_hex(digest).ok());
    let expected_revision_id = source_revision_ref.revision_id;

    context
        .request_security
        .memberships
        .contains(&source_membership.source_membership_id)
        && source_membership.source_membership_id == nomination.source_membership_id
        && source_owner_fence.source_namespace_id == source_revision_ref.source_namespace_id
        && source_membership.source_id == source_revision_ref.source_id
        && source_revision.source_id == source_revision_ref.source_id
        && source_revision.revision_id == source_revision_ref.revision_id
        && source_revision.content_digest == source_revision_ref.content_digest
        && source_revision.byte_length == source_revision_ref.byte_length
        && nomination.source_revision_id == expected_revision_id
        && nomination.expected_content_digest == source_revision_ref.content_digest
        && binding.source_namespace_id == source_revision_ref.source_namespace_id
        && binding.source_id == source_revision_ref.source_id
        && binding.source_revision_id == source_revision_ref.revision_id
        && binding.representation_id == unit_set.representation().representation_id
        && unit_set.representation().unitizer_profile_id == *profile_id
        && unit_set.manifest_digest() == unit_set.representation().unit_manifest_digest
        && provenance.input_digest() == source_revision_ref.content_digest
        && provenance.canonical_digest() == source_revision_ref.content_digest
        && provenance.native_bytes() == source_revision_ref.byte_length
        && provenance.canonical_bytes() == source_revision_ref.byte_length
        && unit_set.representation().representation_id == nomination.representation_id
        && unit.unit_id() == nomination.unit_id
        && unit.source_start() == nomination.unit_byte_start
        && unit.source_end() == nomination.unit_byte_end
        && unit.unit_content_digest() == nomination.expected_unit_digest
        && source_revision.object_residency_key_digest.as_bytes()
            == nomination.expected_residency_digest.as_bytes()
        && profile_id == &nomination.profile_id
        && profile_digest == Some(nomination.profile_digest)
        && raw_v3_profile_matches(profile_id, nomination.profile_digest)
        && nomination.expected_assurance == CandidateAssurance::Exact
        && profile.anchor_policy == V3AnchorPolicy::ExactTextBytes
        && profile.attachment_policy == V3AttachmentPolicy::Absent
        && profile.omission_policy == V3OmissionPolicy::Forbidden
        && profile.overlap_policy == V3OverlapPolicy::None
        && nomination.unit_byte_start < nomination.unit_byte_end
        && nomination.unit_byte_end <= source_revision_ref.byte_length
}

fn candidate_validation_receipt_ref(
    candidate_id: CandidateId,
    nomination: &CandidateNomination,
    context: &ValidationContext,
    source_membership: &SourceMembership,
    source_revision: &SourceRevision,
    source_revision_ref: &SourceRevisionRef,
    source_owner_fence: SourceOwnerFence,
    source: &VerifiedSourceSlice,
    emission_permit: EmissionPermit,
) -> Result<ReceiptRef, ValidationError> {
    let leg_id =
        u64::try_from(nomination.leg_id).map_err(|_| ValidationError::ReceiptComputationFailed)?;
    let visible_epoch = canonical_signed(context.visible_epoch.get());
    let permit_checkpoint = canonical_checkpoint(emission_permit.access_permit.checkpoint)?;
    let assurance: u64 = match source.assurance {
        CandidateAssurance::Derived => 0,
        CandidateAssurance::SourceBacked => 1,
        CandidateAssurance::Exact => 2,
    };
    let values = vec![
        CanonicalValue::U64(1),
        canonical_bytes(candidate_id.as_bytes())?,
        canonical_bytes(nomination.request_id.as_bytes())?,
        canonical_bytes(&nomination.plan_digest)?,
        CanonicalValue::U64(leg_id),
        canonical_bytes(&nomination.point_id)?,
        canonical_bytes(nomination.collection_generation_id.as_bytes())?,
        visible_epoch,
        CanonicalValue::U64(context.request_security.planned_generation),
        canonical_bytes(source_membership.source_membership_id.as_bytes())?,
        canonical_bytes(source_membership.source_id.as_bytes())?,
        canonical_bytes(source_membership.workspace_id.as_bytes())?,
        CanonicalValue::U64(source_membership.membership_revision.get()),
        canonical_bytes(source_revision_ref.source_namespace_id.as_bytes())?,
        canonical_bytes(source_owner_fence.source_namespace_id.as_bytes())?,
        canonical_bytes(source_revision_ref.source_id.as_bytes())?,
        canonical_bytes(source_owner_fence.source_owner_generation.as_bytes())?,
        canonical_bytes(source_revision.revision_id.as_bytes())?,
        CanonicalValue::U64(source_revision.occurrence_sequence),
        canonical_bytes(source_revision_ref.content_digest.as_bytes())?,
        CanonicalValue::U64(source_revision_ref.byte_length),
        canonical_bytes(source_revision.object_residency_key_digest.as_bytes())?,
        canonical_bytes(source_revision_ref.revision_id.as_bytes())?,
        canonical_bytes(source.representation_id.as_bytes())?,
        canonical_bytes(source.unit_id.as_bytes())?,
        CanonicalValue::U64(source.byte_start),
        CanonicalValue::U64(source.byte_end),
        canonical_bytes(source.profile_id.as_str().as_bytes())?,
        canonical_bytes(source.profile_digest.as_bytes())?,
        canonical_bytes(source.residency_digest.as_bytes())?,
        CanonicalValue::U64(assurance),
        canonical_bytes(source.content_digest.as_bytes())?,
        canonical_bytes(source.unit_digest.as_bytes())?,
        canonical_bytes(source.excerpt_digest.as_bytes())?,
        CanonicalValue::U64(u64::from(nomination.raw_score.to_bits())),
        CanonicalValue::Array(
            BoundedList::new(vec![
                permit_checkpoint,
                CanonicalValue::U64(emission_permit.access_permit.live_generation),
                canonical_bytes(
                    emission_permit
                        .access_permit
                        .live_snapshot_digest
                        .as_bytes(),
                )?,
                canonical_bytes(emission_permit.source_membership_id.as_bytes())?,
                canonical_bytes(emission_permit.source_revision_id.as_bytes())?,
            ])
            .map_err(|_| ValidationError::ReceiptComputationFailed)?,
        ),
    ];
    let value = CanonicalValue::Array(
        BoundedList::new(values).map_err(|_| ValidationError::ReceiptComputationFailed)?,
    );
    let domain = CanonicalDigestDomain::parse(CANDIDATE_VALIDATION_RECEIPT_DOMAIN)
        .map_err(|_| ValidationError::ReceiptComputationFailed)?;
    let limit = DigestInputLimit::new(MAX_CANONICAL_BYTES)
        .map_err(|_| ValidationError::ReceiptComputationFailed)?;
    let digest = blake3_canonical(&domain, &value, limit)
        .map_err(|_| ValidationError::ReceiptComputationFailed)?;
    ReceiptRef::new(format!("candidate-validation:v1:{digest}"))
        .map_err(|_| ValidationError::ReceiptComputationFailed)
}

fn canonical_bytes(value: &[u8]) -> Result<CanonicalValue, ValidationError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| ValidationError::ReceiptComputationFailed)
}

fn canonical_signed(value: i64) -> CanonicalValue {
    u64::try_from(value).map_or(CanonicalValue::I64(value), CanonicalValue::U64)
}

fn canonical_checkpoint(checkpoint: AccessCheckpoint) -> Result<CanonicalValue, ValidationError> {
    let name = match checkpoint {
        AccessCheckpoint::RequestAdmission => b"request_admission".as_slice(),
        AccessCheckpoint::BeforeLegDispatch => b"before_leg_dispatch".as_slice(),
        AccessCheckpoint::AfterLegCompletion => b"after_leg_completion".as_slice(),
        AccessCheckpoint::BeforeSourceReadback => b"before_source_readback".as_slice(),
        AccessCheckpoint::AfterSourceReadback => b"after_source_readback".as_slice(),
        AccessCheckpoint::BeforeResultEmission => b"before_result_emission".as_slice(),
        AccessCheckpoint::BeforeExactEmission => b"before_exact_emission".as_slice(),
        AccessCheckpoint::HandleExpansion => b"handle_expansion".as_slice(),
        AccessCheckpoint::ContinuationExpansion => b"continuation_expansion".as_slice(),
    };
    canonical_bytes(name)
}

/// Material effect of losing validated candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageChange {
    Unchanged,
    RefillWithinBudget,
    ReplanRequired,
    ExplicitIncomplete,
}

/// Determines whether candidate loss requires refill, replan, or explicit gap.
#[must_use]
pub const fn material_coverage_change(
    before_validated: usize,
    after_validated: usize,
    target_candidates: usize,
    refill_budget_remaining: usize,
    contaminated_leg: bool,
) -> CoverageChange {
    if contaminated_leg {
        return CoverageChange::ReplanRequired;
    }
    if after_validated >= target_candidates || after_validated == before_validated {
        return CoverageChange::Unchanged;
    }
    let missing = target_candidates.saturating_sub(after_validated);
    if missing <= refill_budget_remaining {
        CoverageChange::RefillWithinBudget
    } else {
        CoverageChange::ExplicitIncomplete
    }
}

#[cfg(test)]
mod spine_gate_tests {
    use super::*;
    use std::collections::BTreeSet;

    const RAW_UNIT: &[u8] = b"alpha\n";

    fn raw_v3_profile() -> (ProfileId, Blake3Digest32) {
        let profile = search_unitizer::validate_v3_unitizer_profile(
            &search_unitizer::V3UnitizerProfileDescriptor {
                profile_name: ProfileId::new("candidate-validator-test-v3").unwrap(),
                profile_revision: 1,
                representation_kind: search_unitizer::V3RepresentationKind::Text,
                unit_kind: search_contracts::UnitKind::File,
                boundary_revision: 1,
                overlap_policy: search_unitizer::V3OverlapPolicy::None,
                anchor_policy: search_unitizer::V3AnchorPolicy::ExactTextBytes,
                attachment_policy: search_unitizer::V3AttachmentPolicy::Absent,
                omission_policy: search_unitizer::V3OmissionPolicy::Forbidden,
                empty_policy: search_unitizer::V3EmptyPolicy::Reject,
                limits: search_unitizer::UnitizationLimits {
                    max_input_bytes: 128,
                    preferred_unit_bytes: 64,
                    max_unit_bytes: 128,
                    max_lines: 128,
                    max_units: 16,
                },
                min_unit_bytes: 1,
                max_unit_scalars: 128,
                max_unit_lines: 128,
                max_anchor_depth: 1,
                max_manifest_bytes: 4096,
                max_steps: 4096,
                fixture_digest: Blake3Digest32::from_bytes([0x71; 32]),
            },
        )
        .unwrap();
        let digest_hex = profile
            .id()
            .as_str()
            .strip_prefix("unitizer-v3-")
            .expect("owner v3 profile identity");
        (
            profile.id().clone(),
            Blake3Digest32::parse_hex(digest_hex).unwrap(),
        )
    }

    fn raw_nomination(expected_excerpt_digest: Option<Blake3Digest32>) -> CandidateNomination {
        let (profile_id, profile_digest) = raw_v3_profile();
        let exact_limit = search_contracts::Blake3ExactInputLimit::new(RAW_UNIT.len()).unwrap();
        CandidateNomination {
            request_id: RequestId::from_bytes([0x11; 16]),
            plan_digest: [0x12; 32],
            leg_id: 3,
            point_id: [0x13; 16],
            source_membership_id: SourceMembershipId::from_bytes([0x14; 16]),
            collection_generation_id: CollectionGenerationId::from_bytes([0x15; 16]),
            valid_from_epoch: Epoch::new(7).unwrap(),
            valid_until_epoch_exclusive: None,
            profile_id,
            profile_digest,
            source_revision_id: SourceRevisionId::from_bytes([0x16; 16]),
            representation_id: RepresentationId::from_bytes([0x17; 16]),
            unit_id: UnitId::from_bytes([0x18; 16]),
            unit_byte_start: 0,
            unit_byte_end: RAW_UNIT.len() as u64,
            expected_content_digest: search_contracts::blake3_exact_bytes(RAW_UNIT, exact_limit)
                .unwrap(),
            expected_unit_digest: raw_v3_unit_content_digest(RAW_UNIT, RAW_UNIT.len()).unwrap(),
            expected_excerpt_digest,
            expected_assurance: CandidateAssurance::Exact,
            expected_residency_digest: Blake3Digest32::from_bytes([0x19; 32]),
            raw_score: 1.0,
        }
    }

    fn raw_readback(nomination: &CandidateNomination, bytes: &[u8]) -> SourceReadback {
        let exact_limit = search_contracts::Blake3ExactInputLimit::new(bytes.len()).unwrap();
        SourceReadback {
            source_membership_id: nomination.source_membership_id,
            source_revision_id: nomination.source_revision_id,
            representation_id: nomination.representation_id,
            unit_id: nomination.unit_id,
            content_digest: nomination.expected_content_digest,
            unit_digest: nomination.expected_unit_digest,
            excerpt_digest: search_contracts::blake3_exact_bytes(bytes, exact_limit).unwrap(),
            profile_id: nomination.profile_id.clone(),
            profile_digest: nomination.profile_digest,
            residency_digest: nomination.expected_residency_digest,
            assurance: CandidateAssurance::Exact,
            source_byte_start: nomination.unit_byte_start,
            source_byte_end: nomination.unit_byte_end,
            coordinate_map_exact: true,
            residency_authorized: true,
            bytes: bytes.to_vec(),
        }
    }

    fn raw_context(nomination: &CandidateNomination) -> ValidationContext {
        let memberships = BTreeSet::from([nomination.source_membership_id]);
        ValidationContext {
            request_id: nomination.request_id,
            plan_digest: nomination.plan_digest,
            collection_generation_id: nomination.collection_generation_id,
            visible_epoch: nomination.valid_from_epoch,
            allowed_memberships: memberships.clone(),
            shadowed_units: BTreeSet::new(),
            max_candidates: 4,
            candidate_ordinal: 0,
            request_security: RequestSecurityFence {
                planned_generation: 2,
                memberships,
            },
            live_security: LiveSecurityState {
                generation: 2,
                denied_memberships: BTreeSet::new(),
                purged_memberships: BTreeSet::new(),
                fail_closed: false,
                snapshot_digest: Blake3Digest32::from_bytes([0x20; 32]),
            },
            contaminated_legs: BTreeSet::new(),
        }
    }

    struct FixedReadback {
        value: SourceReadback,
        reads: usize,
    }

    impl RevisionReadbackPort for FixedReadback {
        type Error = ();

        fn read_exact(
            &mut self,
            _request: &ExactRevisionReadbackRequest,
        ) -> Result<SourceReadback, Self::Error> {
            self.reads += 1;
            Ok(self.value.clone())
        }
    }

    #[test]
    fn gap_reason_codes_are_stable_closed_and_distinct() {
        let cases = [
            (
                ValidationError::RequestMismatch,
                "VALIDATION_REQUEST_MISMATCH",
            ),
            (ValidationError::PlanMismatch, "VALIDATION_PLAN_MISMATCH"),
            (
                ValidationError::LegContaminated,
                "VALIDATION_LEG_CONTAMINATED",
            ),
            (
                ValidationError::MembershipDenied,
                "VALIDATION_MEMBERSHIP_DENIED",
            ),
            (
                ValidationError::CollectionMismatch,
                "VALIDATION_COLLECTION_MISMATCH",
            ),
            (
                ValidationError::ProfileMismatch,
                "VALIDATION_PROFILE_MISMATCH",
            ),
            (ValidationError::EpochInvalid, "VALIDATION_EPOCH_INVALID"),
            (
                ValidationError::OverlayShadowed,
                "VALIDATION_OVERLAY_SHADOWED",
            ),
            (
                ValidationError::CandidateBudgetExceeded,
                "VALIDATION_BUDGET_EXCEEDED",
            ),
            (
                ValidationError::RevisionUnavailable,
                "VALIDATION_REVISION_UNAVAILABLE",
            ),
            (
                ValidationError::SourceUnreadable,
                "VALIDATION_SOURCE_UNREADABLE",
            ),
            (
                ValidationError::ReadbackTimeout,
                "VALIDATION_READBACK_TIMEOUT",
            ),
            (
                ValidationError::RevisionMismatch,
                "VALIDATION_REVISION_MISMATCH",
            ),
            (
                ValidationError::RepresentationMismatch,
                "VALIDATION_REPRESENTATION_MISMATCH",
            ),
            (ValidationError::UnitMismatch, "VALIDATION_UNIT_MISMATCH"),
            (
                ValidationError::ContentDigestMismatch,
                "VALIDATION_CONTENT_DIGEST_MISMATCH",
            ),
            (
                ValidationError::LengthMismatch,
                "VALIDATION_LENGTH_MISMATCH",
            ),
            (
                ValidationError::AnchorOutOfBounds,
                "VALIDATION_ANCHOR_OUT_OF_BOUNDS",
            ),
            (
                ValidationError::ResidencyDenied,
                "VALIDATION_RESIDENCY_DENIED",
            ),
            (
                ValidationError::CoordinateMapMismatch,
                "VALIDATION_COORDINATE_MAP_MISMATCH",
            ),
            (
                ValidationError::AssuranceInsufficient,
                "VALIDATION_ASSURANCE_INSUFFICIENT",
            ),
            (
                ValidationError::DisclosureExceeded,
                "VALIDATION_DISCLOSURE_EXCEEDED",
            ),
            (ValidationError::AccessRevoked, "VALIDATION_ACCESS_REVOKED"),
            (ValidationError::Purged, "VALIDATION_PURGED"),
            (
                ValidationError::EmissionFenceStale,
                "VALIDATION_EMISSION_FENCE_STALE",
            ),
            (
                ValidationError::OwnerMetadataRequired,
                "VALIDATION_OWNER_METADATA_REQUIRED",
            ),
            (
                ValidationError::OwnerBindingMismatch,
                "VALIDATION_OWNER_BINDING_MISMATCH",
            ),
            (ValidationError::InvalidScore, "VALIDATION_INVALID_SCORE"),
            (
                ValidationError::ReceiptComputationFailed,
                "VALIDATION_RECEIPT_COMPUTATION_FAILED",
            ),
        ];
        let mut seen = BTreeSet::new();
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), code);
            assert!(seen.insert(code), "duplicate gap code {code}");
        }
    }

    #[test]
    fn coverage_loss_never_relabels_remaining_topk_as_complete() {
        // Contamination always forces replan, even when counts look unchanged.
        assert_eq!(
            material_coverage_change(5, 5, 5, 10, true),
            CoverageChange::ReplanRequired
        );
        // No loss or target met: unchanged.
        assert_eq!(
            material_coverage_change(5, 5, 5, 0, false),
            CoverageChange::Unchanged
        );
        assert_eq!(
            material_coverage_change(5, 7, 5, 0, false),
            CoverageChange::Unchanged
        );
        // Loss within refill budget: bounded refill, never silent completion.
        assert_eq!(
            material_coverage_change(5, 3, 5, 2, false),
            CoverageChange::RefillWithinBudget
        );
        // Loss beyond refill budget: explicit incomplete, never success.
        assert_eq!(
            material_coverage_change(5, 3, 5, 1, false),
            CoverageChange::ExplicitIncomplete
        );
        assert_eq!(
            material_coverage_change(5, 0, 5, 4, false),
            CoverageChange::ExplicitIncomplete
        );
    }

    #[test]
    fn assurance_floor_is_ordered_so_exact_readback_cannot_weaken() {
        // `reopen_and_verify` rejects readback below the expected floor via
        // `Ord`; a derived snippet can never satisfy an exact demand.
        assert!(CandidateAssurance::Derived < CandidateAssurance::SourceBacked);
        assert!(CandidateAssurance::SourceBacked < CandidateAssurance::Exact);
        assert!(CandidateAssurance::Exact >= CandidateAssurance::Exact);
        assert!(CandidateAssurance::Derived < CandidateAssurance::Exact);
    }

    #[test]
    fn metadata_free_validation_refuses_before_source_readback() {
        let nomination = raw_nomination(Some(Blake3Digest32::from_bytes([0x22; 32])));
        let context = raw_context(&nomination);
        let mut port = FixedReadback {
            value: raw_readback(&nomination, RAW_UNIT),
            reads: 0,
        };

        let outcome = validate(
            &nomination,
            &context,
            RAW_UNIT.len() as u64,
            &mut port,
            &context.live_security,
        );
        assert!(matches!(
            outcome,
            ValidationOutcome::Gap(CandidateValidationGap {
                reason: ValidationError::OwnerMetadataRequired,
                ..
            })
        ));
        assert_eq!(port.reads, 0);
    }

    #[test]
    fn raw_v3_rejects_wrong_bytes_even_when_readback_echoes_expected_digests() {
        let nomination = raw_nomination(None);
        let mut readback = raw_readback(&nomination, b"wrong\n");
        readback.unit_digest = nomination.expected_unit_digest;
        readback.excerpt_digest = search_contracts::blake3_exact_bytes(
            RAW_UNIT,
            search_contracts::Blake3ExactInputLimit::new(RAW_UNIT.len()).unwrap(),
        )
        .unwrap();
        let request = build_readback_request(&nomination, RAW_UNIT.len() as u64).unwrap();

        assert_eq!(
            verify_source_slice_raw_v3_full_unit_v1(&request, readback),
            Err(ValidationError::ContentDigestMismatch)
        );
    }

    #[test]
    fn legacy_reopen_refuses_missing_excerpt_digest_before_readback() {
        let nomination = raw_nomination(None);
        let request = build_readback_request(&nomination, RAW_UNIT.len() as u64).unwrap();
        let mut port = FixedReadback {
            value: raw_readback(&nomination, RAW_UNIT),
            reads: 0,
        };

        assert_eq!(
            reopen_and_verify(&request, &mut port),
            Err(ValidationError::ContentDigestMismatch)
        );
        assert_eq!(port.reads, 0);
    }

    #[test]
    fn raw_v3_reopen_refuses_unrecognized_profile_before_readback() {
        let nomination = raw_nomination(None);
        let mut request = build_readback_request(&nomination, RAW_UNIT.len() as u64).unwrap();
        request.expected_profile_id = ProfileId::new("unaccepted-profile").unwrap();
        let mut port = FixedReadback {
            value: raw_readback(&nomination, RAW_UNIT),
            reads: 0,
        };

        assert_eq!(
            reopen_and_verify_raw_v3_full_unit_v1(&request, &mut port),
            Err(ValidationError::ProfileMismatch)
        );
        assert_eq!(port.reads, 0);
    }

    #[test]
    fn raw_v3_none_derives_standard_excerpt_digest_from_verified_bytes() {
        let nomination = raw_nomination(None);
        let request = build_readback_request(&nomination, RAW_UNIT.len() as u64).unwrap();
        let verified =
            verify_source_slice_raw_v3_full_unit_v1(&request, raw_readback(&nomination, RAW_UNIT))
                .unwrap();
        let expected_excerpt = search_contracts::blake3_exact_bytes(
            RAW_UNIT,
            search_contracts::Blake3ExactInputLimit::new(RAW_UNIT.len()).unwrap(),
        )
        .unwrap();
        assert_eq!(verified.excerpt_digest(), expected_excerpt);
        assert_ne!(verified.excerpt_digest(), verified.unit_digest());
        assert_eq!(verified.profile_id(), &nomination.profile_id);
        assert_eq!(verified.profile_digest(), nomination.profile_digest);
        assert_eq!(
            verified.residency_digest(),
            nomination.expected_residency_digest
        );
    }

    #[test]
    fn raw_v3_some_rejects_a_wrong_standard_excerpt_digest() {
        let wrong_excerpt = Blake3Digest32::from_bytes([0xD1; 32]);
        let nomination = raw_nomination(Some(wrong_excerpt));
        let request = build_readback_request(&nomination, RAW_UNIT.len() as u64).unwrap();

        assert_eq!(
            verify_source_slice_raw_v3_full_unit_v1(&request, raw_readback(&nomination, RAW_UNIT),),
            Err(ValidationError::ContentDigestMismatch)
        );
    }

    #[test]
    fn validation_receipt_binds_typed_candidate_and_final_permit() {
        let nomination = raw_nomination(None);
        let context = raw_context(&nomination);
        let namespace_id = search_contracts::SourceNamespaceId::from_bytes([0x30; 16]);
        let source_id = search_contracts::SourceId::from_bytes([0x31; 16]);
        let source_revision_ref = SourceRevisionRef {
            source_namespace_id: namespace_id,
            source_id,
            revision_id: nomination.source_revision_id,
            content_digest: nomination.expected_content_digest,
            byte_length: RAW_UNIT.len() as u64,
        };
        let residency = ObjectResidencyKeyDigest::from_bytes([0x19; 32]);
        let source_revision = SourceRevision {
            revision_id: nomination.source_revision_id,
            source_id,
            occurrence_sequence: 4,
            content_digest: nomination.expected_content_digest,
            byte_length: RAW_UNIT.len() as u64,
            observed_at: search_contracts::UtcTimestamp::parse("2026-10-16T12:00:00.000000Z")
                .unwrap(),
            acquisition_kind: search_contracts::AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("source-stability:test").unwrap(),
            object_residency_key_digest: residency,
        };
        let membership = SourceMembership {
            source_membership_id: nomination.source_membership_id,
            corpus_id: search_contracts::CorpusId::from_bytes([0x32; 16]),
            source_id,
            workspace_id: search_contracts::WorkspaceId::from_bytes([0x33; 16]),
            role: search_contracts::MembershipRole::Source,
            preparation_profile_id: ProfileId::new("raw-v2-test").unwrap(),
            access_policy_binding_id: search_contracts::AccessPolicyBindingId::from_bytes(
                [0x34; 16],
            ),
            retention_policy_id: ProfileId::new("retention-test-v1").unwrap(),
            residency_policy_binding_id: search_contracts::ResidencyPolicyBindingId::from_bytes(
                [0x35; 16],
            ),
            membership_revision: search_contracts::MembershipRevision::new(1),
        };
        let excerpt_digest = search_contracts::blake3_exact_bytes(
            RAW_UNIT,
            search_contracts::Blake3ExactInputLimit::new(RAW_UNIT.len()).unwrap(),
        )
        .unwrap();
        let source = VerifiedSourceSlice {
            source_membership_id: nomination.source_membership_id,
            source_revision_id: nomination.source_revision_id,
            representation_id: nomination.representation_id,
            unit_id: nomination.unit_id,
            byte_start: nomination.unit_byte_start,
            byte_end: nomination.unit_byte_end,
            content_digest: nomination.expected_content_digest,
            unit_digest: nomination.expected_unit_digest,
            excerpt_digest,
            profile_id: nomination.profile_id.clone(),
            profile_digest: nomination.profile_digest,
            residency_digest: nomination.expected_residency_digest,
            assurance: CandidateAssurance::Exact,
            bytes: RAW_UNIT.to_vec(),
        };
        let permit = EmissionPermit {
            access_permit: AccessPermit {
                checkpoint: AccessCheckpoint::BeforeResultEmission,
                live_generation: context.live_security.generation,
                live_snapshot_digest: context.live_security.snapshot_digest,
            },
            source_membership_id: nomination.source_membership_id,
            source_revision_id: nomination.source_revision_id,
        };
        let owner_fence = SourceOwnerFence {
            source_namespace_id: namespace_id,
            source_owner_generation: SourceOwnerGeneration::from_bytes([0x36; 32]),
        };
        let candidate_id = CandidateId::from_bytes([0x37; 16]);
        let baseline = candidate_validation_receipt_ref(
            candidate_id,
            &nomination,
            &context,
            &membership,
            &source_revision,
            &source_revision_ref,
            owner_fence,
            &source,
            permit,
        )
        .unwrap();
        let changed_candidate = candidate_validation_receipt_ref(
            CandidateId::from_bytes([0x38; 16]),
            &nomination,
            &context,
            &membership,
            &source_revision,
            &source_revision_ref,
            owner_fence,
            &source,
            permit,
        )
        .unwrap();
        let changed_permit = candidate_validation_receipt_ref(
            candidate_id,
            &nomination,
            &context,
            &membership,
            &source_revision,
            &source_revision_ref,
            owner_fence,
            &source,
            EmissionPermit {
                access_permit: AccessPermit {
                    live_snapshot_digest: Blake3Digest32::from_bytes([0x39; 32]),
                    ..permit.access_permit
                },
                ..permit
            },
        )
        .unwrap();

        assert!(baseline.as_str().starts_with("candidate-validation:v1:"));
        assert_ne!(baseline, changed_candidate);
        assert_ne!(baseline, changed_permit);
    }
}
