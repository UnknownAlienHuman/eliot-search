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

use core::cmp::Ordering;
use core::fmt;
use std::collections::BTreeSet;

use search_access::AccessCheckpoint;
use search_candidate_validator::ValidatedSearchCandidate as SourceBackedCandidate;
use search_contracts::{
    AssuranceClass, BoundedList, BoundedNonContentRankingTrace, BoundedSet,
    CandidateId, ContinuationHandle, Coverage, EntityKind, EvidenceRole,
    ObservationFreshnessState, PlanFingerprint, PlanId, ReceiptRef, RequestId,
    ResultFence, SearchCandidateSet, SearchReasonCodeV1, SearchSourceHandle,
    ValidatedSearchCandidate, MAX_LIST_ITEMS, MAX_REASON_CODES,
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
    pub result_validation_receipt_ref: ReceiptRef,
}

/// Finite disclosure and output limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    pub max_candidates: usize,
    pub max_total_validated_source_bytes: usize,
    pub max_source_bytes_per_candidate: usize,
}

impl ProjectionBudget {
    pub const BASELINE: Self = Self {
        max_candidates: 256,
        max_total_validated_source_bytes: 8 * 1_024 * 1_024,
        max_source_bytes_per_candidate: 512 * 1_024,
    };

    pub const fn validate(self) -> Result<Self, ProjectionError> {
        if self.max_candidates == 0
            || self.max_total_validated_source_bytes == 0
            || self.max_source_bytes_per_candidate == 0
        {
            Err(ProjectionError::CandidateBudgetExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Candidate omitted at projection time with a closed reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionOmission {
    pub candidate_id: CandidateId,
    pub reason: ProjectionError,
}

/// Bounded projected candidate set and explicit omissions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedCandidateSet {
    pub result: SearchCandidateSet,
    pub omissions: Vec<ProjectionOmission>,
}

/// Projects validated candidates into the canonical public candidate-set contract.
///
/// Candidate order is deterministic: descending validated score, then
/// `CandidateId`. Over-budget candidates are omitted explicitly; no raw bytes
/// or path text are copied to public output.
pub fn project_candidate_set(
    envelope: CandidateSetEnvelope,
    mut inputs: Vec<CandidateProjectionInput>,
    budget: ProjectionBudget,
) -> Result<ProjectedCandidateSet, ProjectionError> {
    let budget = budget.validate()?;
    envelope
        .coverage
        .validate()
        .map_err(|_| ProjectionError::InvalidCoverage)?;

    inputs.sort_by(|left, right| {
        right
            .source_backed
            .raw_score
            .partial_cmp(&left.source_backed.raw_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.public.candidate_id.cmp(&right.public.candidate_id))
    });

    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    let mut omissions = Vec::new();
    let mut validated_source_bytes = 0_usize;

    for input in inputs {
        if !input.source_backed.raw_score.is_finite() {
            return Err(ProjectionError::InvalidRankingScore);
        }
        if input.source_backed.emission_permit.access_permit.checkpoint
            != AccessCheckpoint::BeforeResultEmission
            || input.source_backed.emission_permit.source_membership_id
                != input.source_backed.source.source_membership_id()
        {
            return Err(ProjectionError::EmissionPermitMissing);
        }
        if !seen.insert(input.public.candidate_id) {
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

        let candidate_bytes = input.source_backed.source.bytes().len();
        let would_exceed_bytes = validated_source_bytes
            .checked_add(candidate_bytes)
            .is_none_or(|next| next > budget.max_total_validated_source_bytes);
        if candidates.len() >= budget.max_candidates
            || candidate_bytes > budget.max_source_bytes_per_candidate
            || would_exceed_bytes
        {
            omissions.push(ProjectionOmission {
                candidate_id: input.public.candidate_id,
                reason: if candidates.len() >= budget.max_candidates {
                    ProjectionError::CandidateBudgetExceeded
                } else {
                    ProjectionError::ResultByteBudgetExceeded
                },
            });
            continue;
        }

        let reason_codes = BoundedSet::<_, MAX_REASON_CODES>::from_items(
            input.public.reason_codes,
        )
        .map_err(|_| ProjectionError::CandidateBudgetExceeded)?;
        let candidate = ValidatedSearchCandidate {
            candidate_id: input.public.candidate_id,
            source_handle: input.public.source_handle,
            evidence_role: input.public.evidence_role,
            entity_kind: input.public.entity_kind,
            assurance: input.public.assurance,
            freshness: input.public.freshness,
            ranking_trace: input.public.ranking_trace,
            reason_codes,
            candidate_validation_receipt_ref: input
                .public
                .candidate_validation_receipt_ref,
        };
        candidate
            .validate()
            .map_err(|_| ProjectionError::InvalidResultContract)?;
        validated_source_bytes = validated_source_bytes.saturating_add(candidate_bytes);
        candidates.push(candidate);
    }

    let candidates = BoundedList::<_, MAX_LIST_ITEMS>::new(candidates)
        .map_err(|_| ProjectionError::CandidateBudgetExceeded)?;
    let result = SearchCandidateSet {
        request_id: envelope.request_id,
        plan_id: envelope.plan_id,
        plan_fingerprint: envelope.plan_fingerprint,
        result_fence: envelope.result_fence,
        candidates,
        coverage: envelope.coverage,
        continuation_handle: envelope.continuation_handle,
        result_validation_receipt_ref: envelope.result_validation_receipt_ref,
    };
    result
        .validate()
        .map_err(|_| ProjectionError::InvalidResultContract)?;
    Ok(ProjectedCandidateSet { result, omissions })
}

/// Verifies that a public handle points at the same exact validated revision
/// and unit represented by a private handle-target digest.
pub fn verify_handle_target_binding(
    candidate: &SourceBackedCandidate,
    expected_source_membership: search_contracts::SourceMembershipId,
    expected_source_revision: search_contracts::SourceRevisionId,
    expected_unit: search_contracts::UnitId,
) -> Result<(), ProjectionError> {
    if candidate.source.source_membership_id() != expected_source_membership
        || candidate.source.source_revision_id() != expected_source_revision
        || candidate.source.unit_id() != expected_unit
    {
        Err(ProjectionError::HandleTargetMismatch)
    } else {
        Ok(())
    }
}
