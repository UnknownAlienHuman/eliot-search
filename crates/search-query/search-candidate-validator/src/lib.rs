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
    AccessCheckpoint, AccessError, AccessPermit, LiveSecurityState,
    RequestSecurityFence, recheck_live_access,
};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, ProfileId,
    RepresentationId, RequestId, SourceMembershipId, SourceRevisionId, UnitId,
};

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
    pub expected_excerpt_digest: Blake3Digest32,
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
    pub expected_excerpt_digest: Blake3Digest32,
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

    #[must_use]
    pub const fn assurance(&self) -> CandidateAssurance {
        self.assurance
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Reopens and verifies one exact immutable source slice.
pub fn reopen_and_verify<P: RevisionReadbackPort>(
    request: &ExactRevisionReadbackRequest,
    port: &mut P,
) -> Result<VerifiedSourceSlice, ValidationError> {
    let readback = port
        .read_exact(request)
        .map_err(|_| ValidationError::SourceUnreadable)?;
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
    if readback.content_digest != request.expected_content_digest
        || readback.unit_digest != request.expected_unit_digest
        || readback.excerpt_digest != request.expected_excerpt_digest
    {
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
        assurance: readback.assurance,
        bytes: readback.bytes,
    })
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

/// Validated evidence candidate with exact score provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSearchCandidate {
    pub candidate_id: OpaqueId,
    pub leg_id: usize,
    pub raw_score: f32,
    pub source: VerifiedSourceSlice,
    pub emission_permit: EmissionPermit,
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

/// Complete validation pipeline over one nomination.
pub fn validate<P: RevisionReadbackPort>(
    nomination: CandidateNomination,
    context: &ValidationContext,
    max_read_bytes: u64,
    port: &mut P,
    final_live_security: &LiveSecurityState,
) -> ValidationOutcome {
    match precheck(&nomination, context) {
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
    let result = build_readback_request(&nomination, max_read_bytes)
        .and_then(|request| reopen_and_verify(&request, port))
        .and_then(|verified| {
            let permit = recheck_before_emission(
                &verified,
                &context.request_security,
                final_live_security,
            )?;
            Ok((verified, permit))
        });
    match result {
        Ok((source, emission_permit)) => {
            ValidationOutcome::Validated(ValidatedSearchCandidate {
                candidate_id: OpaqueId::new(format!(
                    "candidate:{}:{}",
                    nomination.leg_id,
                    hex_point(nomination.point_id)
                ))
                .unwrap_or_else(|_| nomination.profile_id_to_opaque()),
                leg_id: nomination.leg_id,
                raw_score: nomination.raw_score,
                source,
                emission_permit,
            })
        }
        Err(reason) => ValidationOutcome::Gap(CandidateValidationGap {
            leg_id: nomination.leg_id,
            source_membership_id: nomination.source_membership_id,
            reason,
        }),
    }
}

impl CandidateNomination {
    fn profile_id_to_opaque(&self) -> OpaqueId {
        OpaqueId::new(format!("candidate-fallback:{}", self.profile_id.as_str()))
            .expect("bounded profile identifier produces a bounded opaque identifier")
    }
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
pub fn material_coverage_change(
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

fn hex_point(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(32);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
