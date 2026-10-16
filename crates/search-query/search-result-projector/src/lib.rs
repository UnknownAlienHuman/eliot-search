//! Bounded projection of source-backed candidates into public contract types.
//!
//! Only candidates produced by `search-candidate-validator` are accepted.
//! Public output contains opaque handles and non-content ranking metadata; raw
//! source bytes, absolute paths, ACL subjects, and backend identifiers remain private.

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

use search_access::AccessCheckpoint;
use search_candidate_validator::ValidatedSearchCandidate as SourceBackedCandidate;
use search_contracts::{
    AssuranceClass, Blake3Digest32, BoundedBytes, BoundedList, BoundedMap,
    BoundedNonContentMetadata, BoundedNonContentRankingTrace, BoundedSet, CandidateId,
    CandidateValidationGap, CanonicalDigestDomain, CanonicalKey, CanonicalText, CanonicalValue,
    ContinuationHandle, Coverage, CoverageDenominatorKind, CoverageGap, CoverageGapKind,
    CoverageUnknown, DigestInputLimit, EntityKind, EvidenceRole, LegExecutionState,
    LegExecutionSummary, MAX_CANONICAL_BYTES, MAX_COLLECTION_ITEMS, MAX_FRAME_BYTES,
    MAX_LIST_ITEMS, MAX_MAP_ENTRIES, MAX_RAW_BYTES, MAX_REASON_CODES, MetadataKey, MetadataScalar,
    ObservationFreshnessState, OpaqueId, OpaqueRef, PlanFingerprint, PlanId, ReceiptRef,
    RepositoryLineageId, RequestId, ResultFence, Retryability, SearchCandidateSet,
    SearchReasonCodeV1, SearchSourceHandle, SourceMembershipId, SourceOwnerFence,
    SourceRevisionRef, ValidatedSearchCandidate, blake3_canonical,
};

/// Closed result-projection failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionError {
    RequestMismatch,
    PlanMismatch,
    EmissionPermitMissing,
    CandidateIdentityDuplicate,
    CandidateForbiddenReason,
    CandidateBudgetExceeded,
    ResultByteBudgetExceeded,
    SerializedOutputBudgetExceeded,
    OutputEncodingFailed,
    UnqualifiedRankingPopulation,
    CanonicalReceiptComputationFailed,
    InvalidRankingScore,
    InvalidCoverage,
    InvalidResultContract,
    HandleTargetMismatch,
}

impl ProjectionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RequestMismatch => "PROJECT_REQUEST_MISMATCH",
            Self::PlanMismatch => "PROJECT_PLAN_MISMATCH",
            Self::EmissionPermitMissing => "PROJECT_EMISSION_PERMIT_MISSING",
            Self::CandidateIdentityDuplicate => "PROJECT_CANDIDATE_DUPLICATE",
            Self::CandidateForbiddenReason => "PROJECT_CANDIDATE_REASON_FORBIDDEN",
            Self::CandidateBudgetExceeded => "PROJECT_CANDIDATE_BUDGET_EXCEEDED",
            Self::ResultByteBudgetExceeded => "PROJECT_RESULT_BYTE_BUDGET_EXCEEDED",
            Self::SerializedOutputBudgetExceeded => "PROJECT_SERIALIZED_OUTPUT_BUDGET_EXCEEDED",
            Self::OutputEncodingFailed => "PROJECT_OUTPUT_ENCODING_FAILED",
            Self::UnqualifiedRankingPopulation => "PROJECT_RANKING_POPULATION_UNQUALIFIED",
            Self::CanonicalReceiptComputationFailed => "PROJECT_CANONICAL_RECEIPT_FAILED",
            Self::InvalidRankingScore => "PROJECT_RANKING_SCORE_INVALID",
            Self::InvalidCoverage => "PROJECT_COVERAGE_INVALID",
            Self::InvalidResultContract => "PROJECT_RESULT_CONTRACT_INVALID",
            Self::HandleTargetMismatch => "PROJECT_HANDLE_TARGET_MISMATCH",
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionError {}

/// Public metadata assigned by a recipe-specific projector after validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidatePublicMetadata {
    pub candidate_id: CandidateId,
    pub source_handle: SearchSourceHandle,
    pub evidence_role: EvidenceRole,
    pub entity_kind: Option<EntityKind>,
    pub assurance: AssuranceClass,
    pub freshness: ObservationFreshnessState,
    pub ranking_trace: BoundedNonContentRankingTrace,
    pub reason_codes: BTreeSet<SearchReasonCodeV1>,
    pub candidate_validation_receipt_ref: ReceiptRef,
}

/// Source-backed candidate plus its already minted opaque public handle.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateProjectionInput {
    pub source_backed: SourceBackedCandidate,
    pub public: CandidatePublicMetadata,
}

/// Envelope shared by every candidate-set result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateSetEnvelope {
    pub request_id: RequestId,
    pub plan_id: PlanId,
    pub plan_fingerprint: PlanFingerprint,
    pub result_fence: ResultFence,
    pub coverage: Coverage,
    pub continuation_handle: Option<ContinuationHandle>,
}

/// Finite disclosure and output limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    pub max_candidates: usize,
    pub max_total_validated_source_bytes: usize,
    pub max_source_bytes_per_candidate: usize,
    pub max_serialized_output_bytes: usize,
}

impl ProjectionBudget {
    pub const BASELINE: Self = Self {
        max_candidates: 256,
        max_total_validated_source_bytes: 8 * 1_024 * 1_024,
        max_source_bytes_per_candidate: 512 * 1_024,
        max_serialized_output_bytes: MAX_FRAME_BYTES,
    };

    pub const fn validate(self) -> Result<Self, ProjectionError> {
        if self.max_candidates == 0
            || self.max_total_validated_source_bytes == 0
            || self.max_source_bytes_per_candidate == 0
        {
            Err(ProjectionError::CandidateBudgetExceeded)
        } else if self.max_serialized_output_bytes == 0
            || self.max_serialized_output_bytes > MAX_FRAME_BYTES
        {
            Err(ProjectionError::SerializedOutputBudgetExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Candidate omitted at projection time with a closed reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionOmission {
    pub candidate_id: CandidateId,
    pub source_membership_id: SourceMembershipId,
    pub candidate_validation_receipt_ref: ReceiptRef,
    pub reason: ProjectionError,
}

/// Bounded projected candidate set and explicit omissions.
#[derive(Eq, PartialEq)]
pub struct ProjectedCandidateSet {
    result: SearchCandidateSet,
    omissions: BoundedList<ProjectionOmission, MAX_LIST_ITEMS>,
    /// Exact provider-encoded bytes that passed the configured output ceiling.
    /// The caller must emit these bytes unchanged instead of encoding `result` again.
    serialized_output_bytes: BoundedBytes<MAX_FRAME_BYTES>,
}

impl ProjectedCandidateSet {
    /// Read the prepared public result without taking ownership or permitting replacement.
    #[must_use]
    pub const fn result(&self) -> &SearchCandidateSet {
        &self.result
    }

    /// Read projection omissions without taking ownership or permitting replacement.
    #[must_use]
    pub fn omissions(&self) -> &[ProjectionOmission] {
        self.omissions.as_slice()
    }

    /// Borrow the exact bytes produced by the parent encoder for frame emission.
    #[must_use]
    pub fn serialized_output_bytes(&self) -> &[u8] {
        self.serialized_output_bytes.as_slice()
    }
}

#[derive(Clone)]
struct ResultReceiptIdentity {
    candidate_id: CandidateId,
    candidate_validation_receipt_ref: ReceiptRef,
    source_membership_id: SourceMembershipId,
    source_owner_fence: SourceOwnerFence,
    source_revision_ref: SourceRevisionRef,
    leg_id: usize,
    output_candidate_index: Option<usize>,
    omission_reason: Option<ProjectionError>,
}

/// Projects validated candidates into the canonical public candidate-set contract.
///
/// The input order is the validated owner order and is preserved verbatim.
/// Multi-leg candidate windows fail closed until an opaque ranking-window owner
/// supplies a cross-population order. `encode_output` must use the actual
/// provider JSON encoder and return the exact bytes that the caller will emit.
pub fn project_candidate_set<E>(
    envelope: CandidateSetEnvelope,
    inputs: Vec<CandidateProjectionInput>,
    budget: ProjectionBudget,
    encode_output: E,
) -> Result<ProjectedCandidateSet, ProjectionError>
where
    E: FnOnce(&SearchCandidateSet, usize) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProjectionError>,
{
    let budget = budget.validate()?;
    if inputs.len() > MAX_LIST_ITEMS {
        return Err(ProjectionError::CandidateBudgetExceeded);
    }
    envelope
        .coverage
        .validate()
        .map_err(|_| ProjectionError::InvalidCoverage)?;

    validate_rank_population(
        &envelope.coverage,
        inputs.iter().map(|input| input.source_backed.leg_id()),
    )?;

    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    let mut omissions = Vec::new();
    let mut receipt_identities = Vec::with_capacity(inputs.len());
    let mut validated_source_bytes = 0_usize;

    for input in inputs {
        if input.source_backed.request_id() != envelope.request_id {
            return Err(ProjectionError::RequestMismatch);
        }
        if input.source_backed.plan_digest() != envelope.plan_fingerprint.as_bytes() {
            return Err(ProjectionError::PlanMismatch);
        }
        if !input.source_backed.raw_score().is_finite() {
            return Err(ProjectionError::InvalidRankingScore);
        }
        if input
            .source_backed
            .emission_permit()
            .access_permit
            .checkpoint
            != AccessCheckpoint::BeforeResultEmission
            || input.source_backed.emission_permit().source_membership_id
                != input.source_backed.source().source_membership_id()
            || input.source_backed.emission_permit().source_revision_id
                != input.source_backed.source_revision_ref().revision_id
            || input.source_backed.source().source_revision_id()
                != input.source_backed.source_revision_ref().revision_id
            || input.source_backed.source().content_digest()
                != input.source_backed.source_revision_ref().content_digest
            || input.source_backed.source_owner_fence().source_namespace_id
                != input
                    .source_backed
                    .source_revision_ref()
                    .source_namespace_id
            || !envelope
                .result_fence
                .emission_source_owner_fences
                .iter()
                .any(|fence| *fence == input.source_backed.source_owner_fence())
        {
            return Err(ProjectionError::EmissionPermitMissing);
        }
        if !envelope
            .coverage
            .represented_memberships
            .contains(&input.source_backed.source().source_membership_id())
        {
            return Err(ProjectionError::InvalidCoverage);
        }
        if input.public.candidate_id != input.source_backed.candidate_id()
            || input.public.candidate_validation_receipt_ref
                != *input.source_backed.candidate_validation_receipt_ref()
        {
            return Err(ProjectionError::InvalidResultContract);
        }
        if !seen.insert(input.source_backed.candidate_id()) {
            return Err(ProjectionError::CandidateIdentityDuplicate);
        }
        if input
            .public
            .reason_codes
            .iter()
            .any(|reason| reason.is_candidate_forbidden())
        {
            return Err(ProjectionError::CandidateForbiddenReason);
        }

        let candidate_bytes = input.source_backed.source().bytes().len();
        let would_exceed_bytes = validated_source_bytes
            .checked_add(candidate_bytes)
            .is_none_or(|next| next > budget.max_total_validated_source_bytes);
        if candidates.len() >= budget.max_candidates
            || candidate_bytes > budget.max_source_bytes_per_candidate
            || would_exceed_bytes
        {
            let reason = if candidates.len() >= budget.max_candidates {
                ProjectionError::CandidateBudgetExceeded
            } else {
                ProjectionError::ResultByteBudgetExceeded
            };
            omissions.push(ProjectionOmission {
                candidate_id: input.public.candidate_id,
                source_membership_id: input.source_backed.source().source_membership_id(),
                candidate_validation_receipt_ref: input
                    .source_backed
                    .candidate_validation_receipt_ref()
                    .clone(),
                reason,
            });
            receipt_identities.push(ResultReceiptIdentity {
                candidate_id: input.source_backed.candidate_id(),
                candidate_validation_receipt_ref: input
                    .source_backed
                    .candidate_validation_receipt_ref()
                    .clone(),
                source_membership_id: input.source_backed.source().source_membership_id(),
                source_owner_fence: input.source_backed.source_owner_fence(),
                source_revision_ref: input.source_backed.source_revision_ref(),
                leg_id: input.source_backed.leg_id(),
                output_candidate_index: None,
                omission_reason: Some(reason),
            });
            continue;
        }

        let reason_codes = BoundedSet::<_, MAX_REASON_CODES>::from_items(input.public.reason_codes)
            .map_err(|_| ProjectionError::CandidateBudgetExceeded)?;
        let candidate = ValidatedSearchCandidate {
            candidate_id: input.source_backed.candidate_id(),
            source_handle: input.public.source_handle,
            evidence_role: input.public.evidence_role,
            entity_kind: input.public.entity_kind,
            assurance: input.public.assurance,
            freshness: input.public.freshness,
            ranking_trace: input.public.ranking_trace,
            reason_codes,
            candidate_validation_receipt_ref: input
                .source_backed
                .candidate_validation_receipt_ref()
                .clone(),
        };
        candidate
            .validate()
            .map_err(|_| ProjectionError::InvalidResultContract)?;
        validated_source_bytes = validated_source_bytes.saturating_add(candidate_bytes);
        receipt_identities.push(ResultReceiptIdentity {
            candidate_id: input.source_backed.candidate_id(),
            candidate_validation_receipt_ref: input
                .source_backed
                .candidate_validation_receipt_ref()
                .clone(),
            source_membership_id: input.source_backed.source().source_membership_id(),
            source_owner_fence: input.source_backed.source_owner_fence(),
            source_revision_ref: input.source_backed.source_revision_ref(),
            leg_id: input.source_backed.leg_id(),
            output_candidate_index: Some(candidates.len()),
            omission_reason: None,
        });
        candidates.push(candidate);
    }

    let candidates = BoundedList::<_, MAX_LIST_ITEMS>::new(candidates)
        .map_err(|_| ProjectionError::CandidateBudgetExceeded)?;
    let omissions = BoundedList::<_, MAX_LIST_ITEMS>::new(omissions)
        .map_err(|_| ProjectionError::CandidateBudgetExceeded)?;
    let mut coverage = envelope.coverage;
    if !omissions.is_empty() {
        let emitted_memberships = receipt_identities
            .iter()
            .filter(|identity| identity.output_candidate_index.is_some())
            .map(|identity| identity.source_membership_id)
            .collect::<Vec<_>>();
        coverage = coverage_with_budget_omissions(
            coverage,
            omissions.as_slice(),
            &emitted_memberships,
            envelope.continuation_handle.is_some(),
        )?;
    }
    let result_validation_receipt_ref = result_validation_receipt_ref(
        envelope.request_id,
        envelope.plan_id,
        envelope.plan_fingerprint,
        &envelope.result_fence,
        candidates.as_slice(),
        &receipt_identities,
        &coverage,
        envelope.continuation_handle.as_ref(),
        omissions.as_slice(),
    )?;
    let result = SearchCandidateSet {
        request_id: envelope.request_id,
        plan_id: envelope.plan_id,
        plan_fingerprint: envelope.plan_fingerprint,
        result_fence: envelope.result_fence,
        candidates,
        coverage,
        continuation_handle: envelope.continuation_handle,
        result_validation_receipt_ref,
    };
    result
        .validate()
        .map_err(|_| ProjectionError::InvalidResultContract)?;
    let serialized_output_bytes = encode_output(&result, budget.max_serialized_output_bytes)?;
    if serialized_output_bytes.len() > budget.max_serialized_output_bytes {
        return Err(ProjectionError::SerializedOutputBudgetExceeded);
    }
    Ok(ProjectedCandidateSet {
        result,
        omissions,
        serialized_output_bytes,
    })
}

fn coverage_with_budget_omissions(
    mut coverage: Coverage,
    omissions: &[ProjectionOmission],
    emitted_memberships: &[SourceMembershipId],
    continuation_available: bool,
) -> Result<Coverage, ProjectionError> {
    let mut affected_scope_refs = BTreeSet::new();
    for omission in omissions {
        if !coverage
            .represented_memberships
            .contains(&omission.source_membership_id)
        {
            return Err(ProjectionError::InvalidCoverage);
        }
        affected_scope_refs.insert(
            OpaqueRef::new(format!("membership:{}", omission.source_membership_id))
                .map_err(|_| ProjectionError::InvalidCoverage)?,
        );
    }

    let omission_material = canonical_array(omissions.iter().map(canonical_projection_omission))?;
    let omission_digest =
        canonical_digest("eliot/cbor/search-result-budget-gap/v1", &omission_material)?;
    let gap = CoverageGap {
        gap_ref: OpaqueId::new(format!("budget-gap:v1:{omission_digest}"))
            .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
        kind: CoverageGapKind::OmittedBudget,
        affected_scope_refs: BoundedList::new(affected_scope_refs.into_iter().collect())
            .map_err(|_| ProjectionError::InvalidCoverage)?,
        reason_codes: BoundedSet::from_items([SearchReasonCodeV1::ResourceExhausted])
            .map_err(|_| ProjectionError::InvalidCoverage)?,
        retryability: if continuation_available {
            Retryability::SameRequest
        } else {
            Retryability::Never
        },
    };
    let omitted_count =
        u64::try_from(omissions.len()).map_err(|_| ProjectionError::InvalidCoverage)?;
    let unknown = CoverageUnknown {
        unknown_ref: OpaqueId::new(format!("budget-unknown:v1:{omission_digest}"))
            .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
        description_template_id: OpaqueId::new("omitted_budget_v1")
            .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
        bounded_metadata: BoundedNonContentMetadata::new([(
            MetadataKey::parse("omitted_candidate_count")
                .map_err(|_| ProjectionError::InvalidCoverage)?,
            MetadataScalar::Unsigned(omitted_count),
        )])
        .map_err(|_| ProjectionError::InvalidCoverage)?,
    };

    let mut gaps = coverage.omitted_or_failed_legs.as_slice().to_vec();
    gaps.push(gap);
    coverage.omitted_or_failed_legs =
        BoundedList::new(gaps).map_err(|_| ProjectionError::InvalidCoverage)?;
    let mut unknowns = coverage.unknowns.as_slice().to_vec();
    unknowns.push(unknown);
    coverage.unknowns = BoundedList::new(unknowns).map_err(|_| ProjectionError::InvalidCoverage)?;
    coverage.represented_memberships = BoundedSet::from_items(emitted_memberships.iter().copied())
        .map_err(|_| ProjectionError::InvalidCoverage)?;
    coverage.represented_source_lineages = BoundedSet::empty();
    coverage.denominator_kind = CoverageDenominatorKind::Unknown;
    coverage
        .validate()
        .map_err(|_| ProjectionError::InvalidCoverage)?;
    Ok(coverage)
}

fn validate_rank_population(
    coverage: &Coverage,
    leg_ids: impl IntoIterator<Item = usize>,
) -> Result<(), ProjectionError> {
    let mut leg_ids = leg_ids.into_iter();
    let Some(sole_leg_id) = leg_ids.next() else {
        return Ok(());
    };
    if leg_ids.any(|leg_id| leg_id != sole_leg_id) {
        return Err(ProjectionError::UnqualifiedRankingPopulation);
    }
    if coverage.requested_legs.len() != 1 || coverage.executed_legs.len() != 1 {
        return Err(ProjectionError::UnqualifiedRankingPopulation);
    }
    let requested_leg = &coverage.requested_legs.as_slice()[0];
    let executed_leg = &coverage.executed_legs.as_slice()[0];
    if requested_leg.leg_ref != executed_leg.leg_ref
        || matches!(
            executed_leg.state,
            LegExecutionState::Cancelled
                | LegExecutionState::Failed
                | LegExecutionState::DiscardedContaminated
        )
    {
        return Err(ProjectionError::InvalidCoverage);
    }
    Ok(())
}

fn result_validation_receipt_ref(
    request_id: RequestId,
    plan_id: PlanId,
    plan_fingerprint: PlanFingerprint,
    result_fence: &ResultFence,
    candidates: &[ValidatedSearchCandidate],
    identities: &[ResultReceiptIdentity],
    coverage: &Coverage,
    continuation_handle: Option<&ContinuationHandle>,
    omissions: &[ProjectionOmission],
) -> Result<ReceiptRef, ProjectionError> {
    if identities.len() != candidates.len().saturating_add(omissions.len()) {
        return Err(ProjectionError::InvalidResultContract);
    }

    let mut candidate_index = 0_usize;
    let mut omission_index = 0_usize;
    let mut entries = Vec::with_capacity(identities.len());
    for identity in identities {
        let entry = match identity.output_candidate_index {
            Some(index) if identity.omission_reason.is_none() && index == candidate_index => {
                let candidate = candidates
                    .get(index)
                    .ok_or(ProjectionError::InvalidResultContract)?;
                candidate_index = candidate_index.saturating_add(1);
                if candidate.candidate_id != identity.candidate_id
                    || candidate.candidate_validation_receipt_ref
                        != identity.candidate_validation_receipt_ref
                {
                    return Err(ProjectionError::InvalidResultContract);
                }
                canonical_projected_candidate(candidate, identity)?
            }
            None if identity.omission_reason.is_some() => {
                let omission = omissions
                    .get(omission_index)
                    .ok_or(ProjectionError::InvalidResultContract)?;
                omission_index = omission_index.saturating_add(1);
                if omission.candidate_id != identity.candidate_id
                    || omission.source_membership_id != identity.source_membership_id
                    || omission.candidate_validation_receipt_ref
                        != identity.candidate_validation_receipt_ref
                    || Some(omission.reason) != identity.omission_reason
                {
                    return Err(ProjectionError::InvalidResultContract);
                }
                canonical_omission_identity(identity)?
            }
            _ => return Err(ProjectionError::InvalidResultContract),
        };
        entries.push(entry);
    }
    if candidate_index != candidates.len() || omission_index != omissions.len() {
        return Err(ProjectionError::InvalidResultContract);
    }

    let material = canonical_object(vec![
        ("schema", canonical_text("search-result-validation.v1")?),
        ("request_id", canonical_text(request_id.to_string())?),
        ("plan_id", canonical_text(plan_id.to_string())?),
        (
            "plan_fingerprint",
            canonical_bytes(plan_fingerprint.as_bytes())?,
        ),
        ("result_fence", canonical_result_fence(result_fence)?),
        ("ordered_validation_entries", canonical_array(entries)?),
        ("coverage", canonical_coverage(coverage)?),
        (
            "continuation_handle",
            canonical_optional_continuation_handle(continuation_handle)?,
        ),
    ])?;
    tagged_result_validation_receipt(&material)
}

fn tagged_result_validation_receipt(
    material: &CanonicalValue,
) -> Result<ReceiptRef, ProjectionError> {
    let digest = canonical_digest("eliot/cbor/search-result-validation/v1", material)?;
    ReceiptRef::new(format!("result-validation:v1:{digest}"))
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

fn canonical_projection_omission(
    omission: &ProjectionOmission,
) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        (
            "candidate_id",
            canonical_text(omission.candidate_id.to_string())?,
        ),
        (
            "source_membership_id",
            canonical_text(omission.source_membership_id.to_string())?,
        ),
        (
            "candidate_validation_receipt_ref",
            canonical_text(omission.candidate_validation_receipt_ref.as_str())?,
        ),
        ("reason", canonical_text(omission.reason.code())?),
    ])
}

fn canonical_omission_identity(
    identity: &ResultReceiptIdentity,
) -> Result<CanonicalValue, ProjectionError> {
    let reason = identity
        .omission_reason
        .ok_or(ProjectionError::InvalidResultContract)?;
    canonical_object(vec![
        ("kind", canonical_text("omitted")?),
        (
            "candidate_id",
            canonical_text(identity.candidate_id.to_string())?,
        ),
        (
            "candidate_validation_receipt_ref",
            canonical_text(identity.candidate_validation_receipt_ref.as_str())?,
        ),
        (
            "source_membership_id",
            canonical_text(identity.source_membership_id.to_string())?,
        ),
        (
            "leg_id",
            CanonicalValue::U64(
                u64::try_from(identity.leg_id)
                    .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
            ),
        ),
        (
            "source_owner_fence",
            canonical_owner_fence(identity.source_owner_fence)?,
        ),
        (
            "source_revision_ref",
            canonical_source_revision_ref(&identity.source_revision_ref)?,
        ),
        ("reason", canonical_text(reason.code())?),
    ])
}

fn canonical_projected_candidate(
    candidate: &ValidatedSearchCandidate,
    identity: &ResultReceiptIdentity,
) -> Result<CanonicalValue, ProjectionError> {
    let handle = &candidate.source_handle;
    let trace = &candidate.ranking_trace;
    canonical_object(vec![
        ("kind", canonical_text("candidate")?),
        (
            "candidate_id",
            canonical_text(candidate.candidate_id.to_string())?,
        ),
        (
            "candidate_validation_receipt_ref",
            canonical_text(candidate.candidate_validation_receipt_ref.as_str())?,
        ),
        (
            "source_membership_id",
            canonical_text(identity.source_membership_id.to_string())?,
        ),
        (
            "leg_id",
            CanonicalValue::U64(
                u64::try_from(identity.leg_id)
                    .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
            ),
        ),
        (
            "source_owner_fence",
            canonical_owner_fence(identity.source_owner_fence)?,
        ),
        (
            "source_revision_ref",
            canonical_source_revision_ref(&identity.source_revision_ref)?,
        ),
        (
            "source_handle",
            canonical_object(vec![
                ("handle_id", canonical_text(handle.handle_id.to_string())?),
                (
                    "handle_revision",
                    CanonicalValue::U64(handle.handle_revision.get()),
                ),
                (
                    "handle_class",
                    canonical_text(handle.handle_class.as_str())?,
                ),
                (
                    "expires_at",
                    canonical_optional_text(handle.expires_at.as_ref().map(ToString::to_string))?,
                ),
                (
                    "opaque_token",
                    canonical_bytes(handle.opaque_token.as_bytes())?,
                ),
            ])?,
        ),
        (
            "evidence_role",
            canonical_text(candidate.evidence_role.as_str())?,
        ),
        (
            "entity_kind",
            canonical_optional_text(candidate.entity_kind.map(|kind| kind.as_str().to_owned()))?,
        ),
        ("assurance", canonical_text(candidate.assurance.as_str())?),
        ("freshness", canonical_text(candidate.freshness.as_str())?),
        (
            "ranking_trace",
            canonical_object(vec![
                (
                    "fusion_profile_id",
                    canonical_text(trace.fusion_profile_id.as_str())?,
                ),
                (
                    "fused_rank",
                    CanonicalValue::U64(u64::from(trace.fused_rank)),
                ),
                (
                    "exact_or_entity_boost",
                    canonical_text(trace.exact_or_entity_boost.as_str())?,
                ),
                (
                    "evidence_role_priority",
                    CanonicalValue::U64(u64::from(trace.evidence_role_priority)),
                ),
                (
                    "portfolio_priority",
                    CanonicalValue::U64(u64::from(trace.portfolio_priority)),
                ),
                (
                    "lineage_diversity_action",
                    canonical_text(trace.lineage_diversity_action.as_str())?,
                ),
                (
                    "deterministic_tie_break_digest",
                    canonical_bytes(trace.deterministic_tie_break_digest.as_bytes())?,
                ),
            ])?,
        ),
        (
            "reason_codes",
            canonical_array(
                candidate
                    .reason_codes
                    .iter()
                    .map(|reason| canonical_text(reason.as_str())),
            )?,
        ),
    ])
}

fn canonical_result_fence(fence: &ResultFence) -> Result<CanonicalValue, ProjectionError> {
    let snapshot_preimage = fence
        .planned_snapshot
        .canonical_fingerprint_input()
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?;
    canonical_object(vec![
        (
            "planned_snapshot_preimage",
            canonical_bytes(snapshot_preimage.as_slice())?,
        ),
        (
            "planned_snapshot_fingerprint",
            canonical_bytes(fence.planned_snapshot.snapshot_fingerprint.as_bytes())?,
        ),
        (
            "emission_source_owner_fences",
            canonical_array(
                fence
                    .emission_source_owner_fences
                    .iter()
                    .map(|owner| canonical_owner_fence(*owner)),
            )?,
        ),
        (
            "emission_security_fence",
            canonical_object(vec![
                (
                    "access_policy_revision",
                    CanonicalValue::U64(fence.emission_security_fence.access_policy_revision.get()),
                ),
                (
                    "live_deny_generation",
                    CanonicalValue::U64(fence.emission_security_fence.live_deny_generation),
                ),
                (
                    "shadow_fence_revision",
                    CanonicalValue::U64(fence.emission_security_fence.shadow_fence_revision.get()),
                ),
                (
                    "purge_fence_revision",
                    CanonicalValue::U64(fence.emission_security_fence.purge_fence_revision.get()),
                ),
                (
                    "checked_at",
                    canonical_text(fence.emission_security_fence.checked_at.to_string())?,
                ),
                (
                    "receipt_ref",
                    canonical_text(fence.emission_security_fence.receipt_ref.as_str())?,
                ),
            ])?,
        ),
        (
            "result_fingerprint",
            canonical_bytes(fence.result_fingerprint.as_bytes())?,
        ),
    ])
}

fn canonical_owner_fence(fence: SourceOwnerFence) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        (
            "source_namespace_id",
            canonical_text(fence.source_namespace_id.to_string())?,
        ),
        (
            "source_owner_generation",
            canonical_bytes(fence.source_owner_generation.as_bytes())?,
        ),
    ])
}

fn canonical_source_revision_ref(
    source: &SourceRevisionRef,
) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        (
            "source_namespace_id",
            canonical_text(source.source_namespace_id.to_string())?,
        ),
        ("source_id", canonical_text(source.source_id.to_string())?),
        (
            "revision_id",
            canonical_text(source.revision_id.to_string())?,
        ),
        (
            "content_digest",
            canonical_bytes(source.content_digest.as_bytes())?,
        ),
        ("byte_length", CanonicalValue::U64(source.byte_length)),
    ])
}

fn canonical_coverage(coverage: &Coverage) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        (
            "requested_legs",
            canonical_array(coverage.requested_legs.iter().map(|leg| {
                canonical_object(vec![
                    ("leg_ref", canonical_text(leg.leg_ref.as_str())?),
                    ("leg_kind", canonical_text(leg.leg_kind.as_str())?),
                    (
                        "scoring_partition_ref",
                        canonical_optional_text(
                            leg.scoring_partition_ref
                                .as_ref()
                                .map(|value| value.as_str().to_owned()),
                        )?,
                    ),
                    ("profile_id", canonical_text(leg.profile_id.as_str())?),
                ])
            }))?,
        ),
        (
            "executed_legs",
            canonical_array(coverage.executed_legs.iter().map(canonical_leg_execution))?,
        ),
        (
            "represented_memberships",
            canonical_array(
                coverage
                    .represented_memberships
                    .iter()
                    .map(|id| canonical_text(id.to_string())),
            )?,
        ),
        (
            "represented_source_lineages",
            canonical_array(
                coverage
                    .represented_source_lineages
                    .iter()
                    .map(|id: &RepositoryLineageId| canonical_text(id.to_string())),
            )?,
        ),
        (
            "omitted_or_failed_legs",
            canonical_array(
                coverage
                    .omitted_or_failed_legs
                    .iter()
                    .map(canonical_coverage_gap),
            )?,
        ),
        (
            "candidate_validation_gaps",
            canonical_array(
                coverage
                    .candidate_validation_gaps
                    .iter()
                    .map(canonical_candidate_validation_gap),
            )?,
        ),
        (
            "observation_freshness",
            canonical_object(vec![
                (
                    "state",
                    canonical_text(coverage.observation_freshness.state.as_str())?,
                ),
                (
                    "observation_cursor_revision",
                    CanonicalValue::U64(
                        coverage
                            .observation_freshness
                            .observation_cursor_revision
                            .get(),
                    ),
                ),
                (
                    "observed_age_ms",
                    canonical_optional_u64(coverage.observation_freshness.observed_age_ms),
                ),
            ])?,
        ),
        (
            "unknowns",
            canonical_array(coverage.unknowns.iter().map(canonical_coverage_unknown))?,
        ),
        (
            "denominator_kind",
            canonical_text(coverage.denominator_kind.as_str())?,
        ),
    ])
}

fn canonical_leg_execution(leg: &LegExecutionSummary) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        ("leg_ref", canonical_text(leg.leg_ref.as_str())?),
        ("state", canonical_text(leg.state.as_str())?),
        (
            "nominated_count",
            CanonicalValue::U64(u64::from(leg.nominated_count)),
        ),
        (
            "validated_count",
            CanonicalValue::U64(u64::from(leg.validated_count)),
        ),
        (
            "reason_codes",
            canonical_array(
                leg.reason_codes
                    .iter()
                    .map(|reason| canonical_text(reason.as_str())),
            )?,
        ),
        ("receipt_ref", canonical_text(leg.receipt_ref.as_str())?),
    ])
}

fn canonical_coverage_gap(gap: &CoverageGap) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        ("gap_ref", canonical_text(gap.gap_ref.as_str())?),
        ("kind", canonical_text(gap.kind.as_str())?),
        (
            "affected_scope_refs",
            canonical_array(
                gap.affected_scope_refs
                    .iter()
                    .map(|reference| canonical_text(reference.as_str())),
            )?,
        ),
        (
            "reason_codes",
            canonical_array(
                gap.reason_codes
                    .iter()
                    .map(|reason| canonical_text(reason.as_str())),
            )?,
        ),
        ("retryability", canonical_text(gap.retryability.as_str())?),
    ])
}

fn canonical_candidate_validation_gap(
    gap: &CandidateValidationGap,
) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        (
            "nominated_candidate_ref",
            canonical_text(gap.nominated_candidate_ref.as_str())?,
        ),
        (
            "source_revision_ref",
            gap.source_revision_ref
                .map_or(Ok(CanonicalValue::Null), |source| {
                    canonical_source_revision_ref(&source)
                })?,
        ),
        ("reason", canonical_text(gap.reason.as_str())?),
        (
            "affected_leg_refs",
            canonical_array(
                gap.affected_leg_refs
                    .iter()
                    .map(|leg| canonical_text(leg.as_str())),
            )?,
        ),
        (
            "contaminated_rank_leg",
            CanonicalValue::Bool(gap.contaminated_rank_leg),
        ),
        ("disposition", canonical_text(gap.disposition.as_str())?),
    ])
}

fn canonical_coverage_unknown(
    unknown: &CoverageUnknown,
) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(vec![
        ("unknown_ref", canonical_text(unknown.unknown_ref.as_str())?),
        (
            "description_template_id",
            canonical_text(unknown.description_template_id.as_str())?,
        ),
        (
            "bounded_metadata",
            canonical_metadata(&unknown.bounded_metadata)?,
        ),
    ])
}

fn canonical_metadata(
    metadata: &BoundedNonContentMetadata,
) -> Result<CanonicalValue, ProjectionError> {
    canonical_object(
        metadata
            .entries
            .iter()
            .map(|(key, value)| Ok((key.as_str(), canonical_metadata_scalar(value)?)))
            .collect::<Result<Vec<_>, ProjectionError>>()?,
    )
}

fn canonical_metadata_scalar(value: &MetadataScalar) -> Result<CanonicalValue, ProjectionError> {
    match value {
        MetadataScalar::Boolean(value) => Ok(CanonicalValue::Bool(*value)),
        MetadataScalar::Unsigned(value) | MetadataScalar::DurationMs(value) => {
            Ok(CanonicalValue::U64(*value))
        }
        MetadataScalar::Signed(value) if *value < 0 => Ok(CanonicalValue::I64(*value)),
        MetadataScalar::Signed(value) => Ok(CanonicalValue::U64(
            u64::try_from(*value)
                .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
        )),
        MetadataScalar::Digest(value) => canonical_bytes(value.as_bytes()),
        MetadataScalar::ProfileId(value) => canonical_text(value.as_str()),
        MetadataScalar::TemplateId(value) => canonical_text(value.as_str()),
    }
}

fn canonical_optional_continuation_handle(
    handle: Option<&ContinuationHandle>,
) -> Result<CanonicalValue, ProjectionError> {
    let Some(handle) = handle else {
        return Ok(CanonicalValue::Null);
    };
    canonical_object(vec![
        (
            "continuation_id",
            canonical_text(handle.continuation_id.to_string())?,
        ),
        ("expires_at", canonical_text(handle.expires_at.to_string())?),
        (
            "opaque_token",
            canonical_bytes(handle.opaque_token.as_bytes())?,
        ),
    ])
}

fn canonical_optional_text(value: Option<String>) -> Result<CanonicalValue, ProjectionError> {
    value.map_or(Ok(CanonicalValue::Null), canonical_text)
}

fn canonical_optional_u64(value: Option<u64>) -> CanonicalValue {
    value.map_or(CanonicalValue::Null, CanonicalValue::U64)
}

fn canonical_text(value: impl Into<String>) -> Result<CanonicalValue, ProjectionError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

fn canonical_bytes(value: impl AsRef<[u8]>) -> Result<CanonicalValue, ProjectionError> {
    BoundedBytes::<MAX_RAW_BYTES>::new(value.as_ref().to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

trait IntoCanonicalValue {
    fn into_canonical_value(self) -> Result<CanonicalValue, ProjectionError>;
}

impl IntoCanonicalValue for CanonicalValue {
    fn into_canonical_value(self) -> Result<CanonicalValue, ProjectionError> {
        Ok(self)
    }
}

impl IntoCanonicalValue for Result<CanonicalValue, ProjectionError> {
    fn into_canonical_value(self) -> Result<CanonicalValue, ProjectionError> {
        self
    }
}

fn canonical_array<T: IntoCanonicalValue>(
    values: impl IntoIterator<Item = T>,
) -> Result<CanonicalValue, ProjectionError> {
    let values = values
        .into_iter()
        .map(IntoCanonicalValue::into_canonical_value)
        .collect::<Result<Vec<_>, _>>()?;
    BoundedList::<_, MAX_COLLECTION_ITEMS>::new(values)
        .map(CanonicalValue::Array)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

fn canonical_object<K: Into<String>>(
    fields: impl IntoIterator<Item = (K, CanonicalValue)>,
) -> Result<CanonicalValue, ProjectionError> {
    let fields = fields
        .into_iter()
        .map(|(key, value)| {
            Ok((
                CanonicalKey::new_non_empty(key.into())
                    .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?,
                value,
            ))
        })
        .collect::<Result<Vec<_>, ProjectionError>>()?;
    BoundedMap::<_, _, MAX_MAP_ENTRIES>::from_entries(fields)
        .map(CanonicalValue::Object)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

fn canonical_digest(
    domain_name: &str,
    material: &CanonicalValue,
) -> Result<Blake3Digest32, ProjectionError> {
    let domain = CanonicalDigestDomain::parse(domain_name)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?;
    let limit = DigestInputLimit::new(MAX_CANONICAL_BYTES)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)?;
    blake3_canonical(&domain, material, limit)
        .map_err(|_| ProjectionError::CanonicalReceiptComputationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{ObservationCursorRevision, ObservationFreshness, RepositoryLineageId};

    #[test]
    fn budget_omissions_make_coverage_unknown_and_receipts_bind_order_and_handles() {
        let _: for<'a> fn(&'a ProjectedCandidateSet) -> &'a SearchCandidateSet =
            ProjectedCandidateSet::result;
        let _: for<'a> fn(&'a ProjectedCandidateSet) -> &'a [ProjectionOmission] =
            ProjectedCandidateSet::omissions;
        let _: for<'a> fn(&'a ProjectedCandidateSet) -> &'a [u8] =
            ProjectedCandidateSet::serialized_output_bytes;
        let membership = SourceMembershipId::from_bytes([0x11; 16]);
        let leg_ref = OpaqueId::new("leg:3").expect("leg reference");
        let coverage = Coverage {
            requested_legs: BoundedList::new(vec![search_contracts::LegDescriptor {
                leg_ref: leg_ref.clone(),
                leg_kind: search_contracts::LegKind::Lexical,
                scoring_partition_ref: None,
                profile_id: search_contracts::ProfileId::new("lexical-v1").expect("profile"),
            }])
            .expect("single requested leg"),
            executed_legs: BoundedList::new(vec![LegExecutionSummary {
                leg_ref,
                state: LegExecutionState::Completed,
                nominated_count: 2,
                validated_count: 2,
                reason_codes: BoundedSet::empty(),
                receipt_ref: ReceiptRef::new("leg:receipt").expect("leg receipt"),
            }])
            .expect("single executed leg"),
            represented_memberships: BoundedSet::from_items([membership])
                .expect("membership is unique"),
            represented_source_lineages: BoundedSet::from_items([RepositoryLineageId::from_bytes(
                [0x22; 16],
            )])
            .expect("lineage is unique"),
            omitted_or_failed_legs: BoundedList::empty(),
            candidate_validation_gaps: BoundedList::empty(),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::Unknown,
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observed_age_ms: None,
            },
            unknowns: BoundedList::empty(),
            denominator_kind: CoverageDenominatorKind::CompleteScope,
        };
        assert_eq!(validate_rank_population(&coverage, [3]), Ok(()));
        assert_eq!(
            validate_rank_population(&coverage, [3, 4]),
            Err(ProjectionError::UnqualifiedRankingPopulation)
        );
        assert_eq!(
            validate_rank_population(&coverage, core::iter::empty()),
            Ok(())
        );
        let omissions = [ProjectionOmission {
            candidate_id: CandidateId::from_bytes([0x33; 16]),
            source_membership_id: membership,
            candidate_validation_receipt_ref: ReceiptRef::new("candidate-validation:v1:actual")
                .expect("typed receipt ref"),
            reason: ProjectionError::CandidateBudgetExceeded,
        }];
        let reduced = coverage_with_budget_omissions(coverage, &omissions, &[], false)
            .expect("budget omission becomes explicit coverage");
        assert_eq!(reduced.denominator_kind, CoverageDenominatorKind::Unknown);
        assert_eq!(reduced.omitted_or_failed_legs.len(), 1);
        assert_eq!(
            reduced.omitted_or_failed_legs.as_slice()[0].kind,
            CoverageGapKind::OmittedBudget
        );
        assert_eq!(reduced.unknowns.len(), 1);
        assert!(reduced.represented_memberships.is_empty());
        assert!(reduced.represented_source_lineages.is_empty());

        let card = |id: &str, handle: &str| {
            canonical_object(vec![
                ("candidate_id", canonical_text(id).expect("canonical id")),
                ("handle", canonical_text(handle).expect("canonical handle")),
            ])
            .expect("canonical candidate card")
        };
        let material = |cards: Vec<CanonicalValue>| {
            canonical_object(vec![(
                "ordered_cards",
                canonical_array(cards.into_iter().map(Ok)).expect("ordered cards"),
            )])
            .expect("canonical receipt material")
        };
        let forward = tagged_result_validation_receipt(&material(vec![
            card("candidate-1", "handle-1"),
            card("candidate-2", "handle-2"),
        ]))
        .expect("computed receipt");
        let reordered = tagged_result_validation_receipt(&material(vec![
            card("candidate-2", "handle-2"),
            card("candidate-1", "handle-1"),
        ]))
        .expect("computed receipt");
        let tampered_handle = tagged_result_validation_receipt(&material(vec![
            card("candidate-1", "handle-tampered"),
            card("candidate-2", "handle-2"),
        ]))
        .expect("computed receipt");
        assert_ne!(forward, reordered);
        assert_ne!(forward, tampered_handle);
        assert!(forward.as_str().starts_with("result-validation:v1:"));
    }
}

/// Verifies that a public handle points at the same exact validated revision
/// and unit represented by a private handle-target digest.
pub fn verify_handle_target_binding(
    candidate: &SourceBackedCandidate,
    expected_source_membership: search_contracts::SourceMembershipId,
    expected_source_revision: search_contracts::SourceRevisionId,
    expected_unit: search_contracts::UnitId,
) -> Result<(), ProjectionError> {
    if candidate.source().source_membership_id() != expected_source_membership
        || candidate.source().source_revision_id() != expected_source_revision
        || candidate.source().unit_id() != expected_unit
    {
        Err(ProjectionError::HandleTargetMismatch)
    } else {
        Ok(())
    }
}
