//! Deterministic subject resolution over bounded authorized observations.
//!
//! Resolution uses a strict ladder. A lower-priority candidate can never win
//! while a higher-priority applicable step is incomplete. Material ambiguity
//! is returned explicitly; score differences and iteration order never select
//! a definition by accident.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    AmbiguousSubjectCandidate, AssuranceClass, Blake3Digest32, BoundedList,
    BoundedNonContentMetadata, BoundedSet, EntityKind, MatchBasis, MAX_LIST_ITEMS,
    MAX_SET_ITEMS, OpaqueId, ReceiptRef, ResolvedSubject, SearchReasonCodeV1,
    SubjectAmbiguitySet,
};

/// Closed resolver failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SubjectError {
    /// Request contains no usable selector or contradictory selectors.
    SubjectRequestInvalid,
    /// Explicit authorized scope is empty.
    SubjectScopeEmpty,
    /// No subject was found in the completed resolution scope.
    SubjectNotFound,
    /// More than one material subject hypothesis remains.
    AmbiguousSubject,
    /// Context or source/workspace view is stale.
    SubjectContextStale,
    /// Source-owner generation changed.
    SubjectOwnerGenerationChanged,
    /// Current authorization denies subject disclosure.
    SubjectAccessRevoked,
    /// Observation continuity is incomplete.
    SubjectObservationGap,
    /// Candidate equivalence required for collapse is not proven.
    SubjectEquivalenceUnproven,
    /// A higher-priority applicable ladder step is incomplete.
    SubjectEvidenceIncomplete,
    /// Finite candidate or ambiguity budget was exhausted.
    SubjectBudgetExhausted,
    /// Explicit cancellation was observed.
    SubjectCancelled,
    /// Ambiguity cannot be represented completely inside the configured limit.
    SubjectAmbiguityTruncated,
    /// Candidate, step, fence, or receipt accounting is contradictory.
    SubjectReportInvalid,
}

impl SubjectError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SubjectRequestInvalid => "SUBJECT_REQUEST_INVALID",
            Self::SubjectScopeEmpty => "SUBJECT_SCOPE_EMPTY",
            Self::SubjectNotFound => "SUBJECT_NOT_FOUND",
            Self::AmbiguousSubject => "AMBIGUOUS_SUBJECT",
            Self::SubjectContextStale => "SUBJECT_CONTEXT_STALE",
            Self::SubjectOwnerGenerationChanged => "SUBJECT_OWNER_GENERATION_CHANGED",
            Self::SubjectAccessRevoked => "SUBJECT_ACCESS_REVOKED",
            Self::SubjectObservationGap => "SUBJECT_OBSERVATION_GAP",
            Self::SubjectEquivalenceUnproven => "SUBJECT_EQUIVALENCE_UNPROVEN",
            Self::SubjectEvidenceIncomplete => "SUBJECT_EVIDENCE_INCOMPLETE",
            Self::SubjectBudgetExhausted => "SUBJECT_BUDGET_EXHAUSTED",
            Self::SubjectCancelled => "SUBJECT_CANCELLED",
            Self::SubjectAmbiguityTruncated => "SUBJECT_AMBIGUITY_TRUNCATED",
            Self::SubjectReportInvalid => "SUBJECT_REPORT_INVALID",
        }
    }
}

impl fmt::Display for SubjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SubjectError {}

/// Strict resolution-ladder priority. Declaration order is strongest first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResolutionPriority {
    /// Validated explicit source handle.
    ExplicitHandle,
    /// Authenticated editor cursor or position.
    EditorPosition,
    /// Exact normalized qualified symbol/entity key.
    QualifiedKey,
    /// Exact normalized name in the requested scope.
    ExactName,
    /// Compatible signature and entity kind.
    SignatureAndKind,
    /// Validated structural candidate.
    Structural,
    /// Validated lexical candidate.
    Lexical,
}

/// Maps a contract match basis into the deterministic ladder.
#[must_use]
pub const fn rank_resolution_basis(basis: MatchBasis) -> Option<ResolutionPriority> {
    match basis {
        MatchBasis::ExplicitHandle => Some(ResolutionPriority::ExplicitHandle),
        MatchBasis::EditorPosition => Some(ResolutionPriority::EditorPosition),
        MatchBasis::QualifiedName => Some(ResolutionPriority::QualifiedKey),
        MatchBasis::ExactName => Some(ResolutionPriority::ExactName),
        MatchBasis::Signature => Some(ResolutionPriority::SignatureAndKind),
        MatchBasis::Structural => Some(ResolutionPriority::Structural),
        MatchBasis::Lexical => Some(ResolutionPriority::Lexical),
        MatchBasis::Semantic => None,
    }
}

/// Normalized selector request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectRequest {
    /// Digest of canonical selector fields.
    pub selector_digest: Blake3Digest32,
    /// Digest of exact requested source/workspace/reference context.
    pub requested_context_digest: Blake3Digest32,
    /// Ladder steps applicable to the explicit selector.
    pub applicable_steps: BoundedSet<ResolutionPriority, MAX_SET_ITEMS>,
    /// Optional requested entity kind.
    pub required_entity_kind: Option<EntityKind>,
    /// Whether cancellation was observed before resolution.
    pub cancelled: bool,
}

impl SubjectRequest {
    /// Validates a non-empty bounded selector.
    pub fn validate(&self) -> Result<(), SubjectError> {
        if self.applicable_steps.is_empty() {
            return Err(SubjectError::SubjectRequestInvalid);
        }
        if self.cancelled {
            return Err(SubjectError::SubjectCancelled);
        }
        Ok(())
    }
}

/// Coherent authorization/currentness context for all candidate observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionContext {
    /// Digest of exact source/workspace/reference view and plan fence.
    pub context_digest: Blake3Digest32,
    /// Digest of exact source-owner generation set.
    pub owner_generation_digest: Blake3Digest32,
    /// Digest of current grant/access/live-deny/purge fence.
    pub security_fence_digest: Blake3Digest32,
    /// Explicit scope contains at least one authorized source.
    pub scope_non_empty: bool,
    /// Current authorization permits disclosure.
    pub access_permitted: bool,
    /// No purge barrier covers the request.
    pub purge_clear: bool,
    /// Source/workspace context remains current.
    pub view_current: bool,
    /// Owner generation remains current.
    pub owner_generation_current: bool,
    /// Observation continuity is sufficient for decisive fall-through.
    pub observation_complete: bool,
}

/// Validates one coherent resolution context.
pub fn validate_resolution_context(
    request: &SubjectRequest,
    context: &ResolutionContext,
) -> Result<(), SubjectError> {
    request.validate()?;
    if !context.scope_non_empty {
        return Err(SubjectError::SubjectScopeEmpty);
    }
    if !context.access_permitted || !context.purge_clear {
        return Err(SubjectError::SubjectAccessRevoked);
    }
    if request.requested_context_digest != context.context_digest || !context.view_current {
        return Err(SubjectError::SubjectContextStale);
    }
    if !context.owner_generation_current {
        return Err(SubjectError::SubjectOwnerGenerationChanged);
    }
    if !context.observation_complete {
        return Err(SubjectError::SubjectObservationGap);
    }
    Ok(())
}

/// Authorized source-backed candidate at one ladder rung.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectCandidate {
    /// Stable candidate identity digest.
    pub candidate_digest: Blake3Digest32,
    /// Hypothesis digest. Candidates may collapse under this digest only when
    /// an accepted equivalence receipt is present.
    pub hypothesis_digest: Blake3Digest32,
    /// Contract result shape for this candidate.
    pub subject: ResolvedSubject,
    /// Match basis supplied by the producing package.
    pub match_basis: MatchBasis,
    /// Assurance of the resolution evidence.
    pub assurance: AssuranceClass,
    /// Whether the requested entity-kind constraint is satisfied.
    pub entity_kind_compatible: bool,
    /// Current/reference portfolio precedence; lower values are preferred.
    pub portfolio_priority: u16,
    /// Stable source identity digest used for deterministic ordering.
    pub source_identity_digest: Blake3Digest32,
    /// Native coordinate digest used after source identity.
    pub coordinate_digest: Blake3Digest32,
    /// Exact context digest of the producing observation.
    pub context_digest: Blake3Digest32,
    /// Current grant permits disclosure of this candidate.
    pub authorized: bool,
    /// Candidate evidence is current.
    pub current: bool,
    /// Accepted receipt proving duplicate/alias/rename equivalence.
    pub equivalence_receipt_ref: Option<ReceiptRef>,
    /// Bounded authorized differentiation metadata.
    pub disambiguation_summary: BoundedNonContentMetadata,
    /// Source-backed evidence receipt.
    pub evidence_receipt_ref: ReceiptRef,
}

impl SubjectCandidate {
    fn validate(
        &self,
        priority: ResolutionPriority,
        request: &SubjectRequest,
        context: &ResolutionContext,
    ) -> Result<(), SubjectError> {
        if rank_resolution_basis(self.match_basis) != Some(priority)
            || self.subject.match_basis != self.match_basis
            || self.subject.entity_kind
                != self.subject.entity_kind
            || self.context_digest != context.context_digest
        {
            return Err(SubjectError::SubjectReportInvalid);
        }
        if !self.authorized || !context.access_permitted || !context.purge_clear {
            return Err(SubjectError::SubjectAccessRevoked);
        }
        if !self.current || !context.view_current || !context.owner_generation_current {
            return Err(SubjectError::SubjectContextStale);
        }
        if !self.entity_kind_compatible
            || request
                .required_entity_kind
                .is_some_and(|kind| kind != self.subject.entity_kind)
        {
            return Err(SubjectError::SubjectEvidenceIncomplete);
        }
        Ok(())
    }
}

/// Why a ladder step did not complete.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StepIncompleteReason {
    /// Candidate retrieval was cancelled.
    Cancelled,
    /// Finite deadline expired.
    Timeout,
    /// Candidate budget was exhausted.
    BudgetExhausted,
    /// Candidate output was truncated.
    Truncated,
    /// Source evidence was unreadable.
    Unreadable,
    /// Observation continuity has a gap.
    ObservationGap,
    /// Context changed while the step executed.
    ContextStale,
    /// Access or purge state changed.
    AccessRevoked,
}

impl StepIncompleteReason {
    fn as_error(self) -> SubjectError {
        match self {
            Self::Cancelled => SubjectError::SubjectCancelled,
            Self::Timeout | Self::BudgetExhausted => SubjectError::SubjectBudgetExhausted,
            Self::Truncated => SubjectError::SubjectAmbiguityTruncated,
            Self::Unreadable => SubjectError::SubjectEvidenceIncomplete,
            Self::ObservationGap => SubjectError::SubjectObservationGap,
            Self::ContextStale => SubjectError::SubjectContextStale,
            Self::AccessRevoked => SubjectError::SubjectAccessRevoked,
        }
    }
}

/// Completion state for one ladder rung.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionStepState {
    /// Every relevant source for this rung was considered.
    Complete,
    /// Rung is applicable but could not complete decisively.
    Incomplete(StepIncompleteReason),
    /// Rung is not applicable to the normalized request.
    NotApplicable,
}

/// Bounded result of one resolution-ladder rung.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionStep {
    /// Strict ladder priority.
    pub priority: ResolutionPriority,
    /// Completion state.
    pub state: ResolutionStepState,
    /// Authorized source-backed candidates.
    pub candidates: BoundedList<SubjectCandidate, MAX_LIST_ITEMS>,
    /// Number of omitted candidate observations.
    pub omitted_candidates: u64,
}

impl ResolutionStep {
    fn validate_shape(&self) -> Result<(), SubjectError> {
        match self.state {
            ResolutionStepState::Complete if self.omitted_candidates == 0 => Ok(()),
            ResolutionStepState::Complete => Err(SubjectError::SubjectReportInvalid),
            ResolutionStepState::Incomplete(_) => Ok(()),
            ResolutionStepState::NotApplicable if self.candidates.is_empty() => Ok(()),
            ResolutionStepState::NotApplicable => Err(SubjectError::SubjectReportInvalid),
        }
    }
}

/// Finite resolution budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubjectResolutionLimits {
    /// Maximum observations considered across all rungs.
    pub max_candidates: usize,
    /// Maximum material hypotheses returned as ambiguity.
    pub max_ambiguity_candidates: usize,
    /// Maximum evidence receipts retained in a resolution receipt.
    pub max_evidence_receipts: usize,
}

impl SubjectResolutionLimits {
    /// Conservative baseline.
    pub const BASELINE: Self = Self {
        max_candidates: MAX_LIST_ITEMS,
        max_ambiguity_candidates: 64,
        max_evidence_receipts: 256,
    };

    /// Validates finite non-zero dimensions.
    pub fn validate(self) -> Result<Self, SubjectError> {
        if self.max_candidates == 0
            || self.max_candidates > MAX_LIST_ITEMS
            || self.max_ambiguity_candidates == 0
            || self.max_ambiguity_candidates > MAX_LIST_ITEMS
            || self.max_evidence_receipts == 0
            || self.max_evidence_receipts > MAX_LIST_ITEMS
        {
            Err(SubjectError::SubjectBudgetExhausted)
        } else {
            Ok(self)
        }
    }
}

/// Closed resolver output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubjectResolution {
    /// One materially unique subject at the strongest completed rung.
    Resolved {
        /// Resolved contract subject.
        subject: ResolvedSubject,
        /// Stable selected candidate digest.
        candidate_digest: Blake3Digest32,
        /// Winning ladder rung.
        priority: ResolutionPriority,
    },
    /// Multiple material hypotheses remain.
    Ambiguous {
        /// Contract ambiguity set.
        ambiguity: SubjectAmbiguitySet,
        /// Rung at which ambiguity became material.
        priority: ResolutionPriority,
    },
    /// Every applicable rung completed with no candidates.
    NotFound {
        /// Selector digest whose bounded scope was searched.
        selector_digest: Blake3Digest32,
    },
    /// Authorized scope is empty.
    ScopeEmpty,
    /// A higher-priority applicable rung did not complete.
    Incomplete {
        /// Blocking rung.
        priority: ResolutionPriority,
        /// Typed blocking reason.
        reason: SubjectError,
    },
}

/// Deterministically resolves a subject without score-based guessing.
pub fn resolve_subject(
    request: &SubjectRequest,
    context: &ResolutionContext,
    steps: Vec<ResolutionStep>,
    limits: SubjectResolutionLimits,
) -> Result<SubjectResolution, SubjectError> {
    request.validate()?;
    let limits = limits.validate()?;
    if !context.scope_non_empty {
        return Ok(SubjectResolution::ScopeEmpty);
    }
    validate_resolution_context(request, context)?;

    let mut by_priority = BTreeMap::new();
    let mut total_candidates = 0_usize;
    for step in steps {
        step.validate_shape()?;
        total_candidates = total_candidates
            .checked_add(step.candidates.len())
            .ok_or(SubjectError::SubjectBudgetExhausted)?;
        if total_candidates > limits.max_candidates
            || by_priority.insert(step.priority, step).is_some()
        {
            return Err(SubjectError::SubjectBudgetExhausted);
        }
    }

    for priority in request.applicable_steps.iter().copied() {
        let Some(step) = by_priority.get(&priority) else {
            return Ok(SubjectResolution::Incomplete {
                priority,
                reason: SubjectError::SubjectEvidenceIncomplete,
            });
        };
        match step.state {
            ResolutionStepState::NotApplicable => {
                return Err(SubjectError::SubjectReportInvalid);
            }
            ResolutionStepState::Incomplete(reason) => {
                return Ok(SubjectResolution::Incomplete {
                    priority,
                    reason: reason.as_error(),
                });
            }
            ResolutionStepState::Complete => {}
        }
        if step.candidates.is_empty() {
            continue;
        }

        let hypotheses = collapse_equivalent_occurrences(
            priority,
            request,
            context,
            step.candidates.iter().cloned().collect(),
        )?;
        if hypotheses.len() == 1 {
            let candidate = hypotheses
                .into_values()
                .next()
                .and_then(|mut values| {
                    stable_sort_candidates(&mut values);
                    values.into_iter().next()
                })
                .ok_or(SubjectError::SubjectReportInvalid)?;
            return Ok(SubjectResolution::Resolved {
                subject: candidate.subject,
                candidate_digest: candidate.candidate_digest,
                priority,
            });
        }

        if matches!(
            priority,
            ResolutionPriority::ExplicitHandle | ResolutionPriority::EditorPosition
        ) {
            return Ok(SubjectResolution::Incomplete {
                priority,
                reason: SubjectError::AmbiguousSubject,
            });
        }
        let representatives = hypotheses
            .into_values()
            .map(|mut values| {
                stable_sort_candidates(&mut values);
                values
                    .into_iter()
                    .next()
                    .expect("non-empty hypothesis")
            })
            .collect::<Vec<_>>();
        let ambiguity = build_ambiguity_set(
            request.selector_digest,
            representatives,
            limits.max_ambiguity_candidates,
        )?;
        return Ok(SubjectResolution::Ambiguous {
            ambiguity,
            priority,
        });
    }

    Ok(SubjectResolution::NotFound {
        selector_digest: request.selector_digest,
    })
}

/// Collapses only candidates carrying accepted equivalence proof.
pub fn collapse_equivalent_occurrences(
    priority: ResolutionPriority,
    request: &SubjectRequest,
    context: &ResolutionContext,
    candidates: Vec<SubjectCandidate>,
) -> Result<BTreeMap<Blake3Digest32, Vec<SubjectCandidate>>, SubjectError> {
    let mut hypotheses = BTreeMap::new();
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        candidate.validate(priority, request, context)?;
        if !candidate_ids.insert(candidate.candidate_digest) {
            return Err(SubjectError::SubjectReportInvalid);
        }
        let key = if candidate.equivalence_receipt_ref.is_some() {
            candidate.hypothesis_digest
        } else {
            candidate.candidate_digest
        };
        hypotheses
            .entry(key)
            .or_insert_with(Vec::new)
            .push(candidate);
    }
    Ok(hypotheses)
}

/// Builds a complete bounded contract ambiguity set.
pub fn build_ambiguity_set(
    selector_digest: Blake3Digest32,
    mut candidates: Vec<SubjectCandidate>,
    limit: usize,
) -> Result<SubjectAmbiguitySet, SubjectError> {
    if candidates.len() < 2 {
        return Err(SubjectError::SubjectReportInvalid);
    }
    if limit == 0 || limit > MAX_LIST_ITEMS || candidates.len() > limit {
        return Err(SubjectError::SubjectAmbiguityTruncated);
    }
    if candidates.iter().any(|candidate| {
        matches!(
            candidate.match_basis,
            MatchBasis::ExplicitHandle | MatchBasis::EditorPosition | MatchBasis::Semantic
        )
    }) {
        return Err(SubjectError::SubjectReportInvalid);
    }
    stable_sort_candidates(&mut candidates);
    let ambiguity = SubjectAmbiguitySet {
        requested_selector_digest: selector_digest,
        candidates: BoundedList::new(
            candidates
                .into_iter()
                .map(|candidate| AmbiguousSubjectCandidate {
                    source_handle: candidate.subject.canonical_handle,
                    entity_kind: candidate.subject.entity_kind,
                    match_basis: candidate.match_basis,
                    disambiguation_summary: candidate.disambiguation_summary,
                })
                .collect(),
        )
        .map_err(|_| SubjectError::SubjectBudgetExhausted)?,
        reason_code: SearchReasonCodeV1::AmbiguousSubject,
    };
    ambiguity
        .validate()
        .map_err(|_| SubjectError::SubjectReportInvalid)?;
    Ok(ambiguity)
}

fn stable_sort_candidates(candidates: &mut [SubjectCandidate]) {
    candidates.sort_by(|left, right| {
        rank_resolution_basis(left.match_basis)
            .cmp(&rank_resolution_basis(right.match_basis))
            .then_with(|| right.entity_kind_compatible.cmp(&left.entity_kind_compatible))
            .then_with(|| assurance_rank(right.assurance).cmp(&assurance_rank(left.assurance)))
            .then_with(|| left.portfolio_priority.cmp(&right.portfolio_priority))
            .then_with(|| left.source_identity_digest.cmp(&right.source_identity_digest))
            .then_with(|| left.coordinate_digest.cmp(&right.coordinate_digest))
            .then_with(|| left.candidate_digest.cmp(&right.candidate_digest))
    });
}

const fn assurance_rank(value: AssuranceClass) -> u8 {
    match value {
        AssuranceClass::ExactBytes => 5,
        AssuranceClass::MappedText => 4,
        AssuranceClass::Structural => 3,
        AssuranceClass::IndexedLexical => 2,
        AssuranceClass::SemanticOnly => 1,
    }
}

/// Output class bound into a resolution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionOutputKind {
    /// Unique resolution.
    Resolved,
    /// Material ambiguity.
    Ambiguous,
    /// Completed bounded non-resolution.
    NotFound,
    /// Empty scope.
    ScopeEmpty,
    /// Incomplete higher-priority evidence.
    Incomplete,
}

/// Immutable content-free resolution receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionReceipt {
    /// Canonical selector digest.
    pub selector_digest: Blake3Digest32,
    /// Exact context digest.
    pub context_digest: Blake3Digest32,
    /// Exact owner-generation digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Exact security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Digest of every candidate identity considered.
    pub candidate_set_digest: Blake3Digest32,
    /// Closed output class.
    pub output_kind: ResolutionOutputKind,
    /// Selected candidate digest, if uniquely resolved.
    pub selected_candidate_digest: Option<Blake3Digest32>,
    /// Material ambiguity candidate digests.
    pub ambiguity_candidate_digests: BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    /// Source-backed evidence receipt references.
    pub evidence_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
    /// Digest of exact receipt contents.
    pub receipt_digest: Blake3Digest32,
}

/// Issues a deterministic resolution receipt from the exact executed inputs.
pub fn issue_resolution_receipt(
    request: &SubjectRequest,
    context: &ResolutionContext,
    resolution: &SubjectResolution,
    mut candidates: Vec<SubjectCandidate>,
    limits: SubjectResolutionLimits,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<ResolutionReceipt, SubjectError> {
    let limits = limits.validate()?;
    if candidates.len() > limits.max_candidates {
        return Err(SubjectError::SubjectBudgetExhausted);
    }
    stable_sort_candidates(&mut candidates);
    let mut seen = BTreeSet::new();
    for candidate in &candidates {
        if candidate.context_digest != context.context_digest
            || !seen.insert(candidate.candidate_digest)
        {
            return Err(SubjectError::SubjectReportInvalid);
        }
    }
    let candidate_digests = candidates
        .iter()
        .map(|candidate| candidate.candidate_digest)
        .collect::<Vec<_>>();
    let evidence = candidates
        .iter()
        .map(|candidate| candidate.evidence_receipt_ref.clone())
        .collect::<Vec<_>>();
    if evidence.len() > limits.max_evidence_receipts {
        return Err(SubjectError::SubjectBudgetExhausted);
    }
    let candidate_set_digest = Blake3Digest32::from_bytes(blake3_256(
        &candidate_set_digest_input(&candidate_digests)?,
    ));
    let (output_kind, selected_candidate_digest, ambiguity_candidate_digests) =
        match resolution {
            SubjectResolution::Resolved {
                candidate_digest, ..
            } => (
                ResolutionOutputKind::Resolved,
                Some(*candidate_digest),
                Vec::new(),
            ),
            SubjectResolution::Ambiguous { ambiguity, .. } => {
                let digests = ambiguity
                    .candidates
                    .iter()
                    .filter_map(|candidate| {
                        candidates
                            .iter()
                            .find(|source| {
                                source.subject.canonical_handle == candidate.source_handle
                                    && source.match_basis == candidate.match_basis
                            })
                            .map(|source| source.candidate_digest)
                    })
                    .collect::<Vec<_>>();
                if digests.len() != ambiguity.candidates.len() {
                    return Err(SubjectError::SubjectReportInvalid);
                }
                (ResolutionOutputKind::Ambiguous, None, digests)
            }
            SubjectResolution::NotFound { .. } => {
                (ResolutionOutputKind::NotFound, None, Vec::new())
            }
            SubjectResolution::ScopeEmpty => {
                (ResolutionOutputKind::ScopeEmpty, None, Vec::new())
            }
            SubjectResolution::Incomplete { .. } => {
                (ResolutionOutputKind::Incomplete, None, Vec::new())
            }
        };
    let ambiguity_candidate_digests = BoundedList::new(ambiguity_candidate_digests)
        .map_err(|_| SubjectError::SubjectBudgetExhausted)?;
    let evidence_receipt_refs = BoundedList::new(evidence)
        .map_err(|_| SubjectError::SubjectBudgetExhausted)?;
    let receipt_input = resolution_receipt_digest_input(
        request,
        context,
        candidate_set_digest,
        output_kind,
        selected_candidate_digest,
        &ambiguity_candidate_digests,
        &evidence_receipt_refs,
    )?;
    Ok(ResolutionReceipt {
        selector_digest: request.selector_digest,
        context_digest: context.context_digest,
        owner_generation_digest: context.owner_generation_digest,
        security_fence_digest: context.security_fence_digest,
        candidate_set_digest,
        output_kind,
        selected_candidate_digest,
        ambiguity_candidate_digests,
        evidence_receipt_refs,
        receipt_digest: Blake3Digest32::from_bytes(blake3_256(&receipt_input)),
    })
}

/// Current state used to revalidate a resolution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionLiveState {
    /// Current context digest.
    pub context_digest: Blake3Digest32,
    /// Current owner-generation digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Current security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Current access permits disclosure.
    pub access_permitted: bool,
    /// No purge barrier covers the result.
    pub purge_clear: bool,
    /// Observation continuity is current.
    pub observation_complete: bool,
}

/// Receipt revalidation outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionRevalidation {
    /// Receipt remains current.
    Current,
    /// Source/workspace or candidate context changed.
    ReResolve,
    /// Owner generation changed.
    OwnerGenerationChanged,
    /// Access or purge state revoked disclosure.
    AccessRevoked,
    /// Observation continuity is incomplete.
    ObservationGap,
}

/// Revalidates a receipt without patching it in place.
#[must_use]
pub fn revalidate_resolution(
    receipt: &ResolutionReceipt,
    current: ResolutionLiveState,
) -> ResolutionRevalidation {
    if !current.access_permitted
        || !current.purge_clear
        || current.security_fence_digest != receipt.security_fence_digest
    {
        return ResolutionRevalidation::AccessRevoked;
    }
    if !current.observation_complete {
        return ResolutionRevalidation::ObservationGap;
    }
    if current.owner_generation_digest != receipt.owner_generation_digest {
        return ResolutionRevalidation::OwnerGenerationChanged;
    }
    if current.context_digest != receipt.context_digest {
        return ResolutionRevalidation::ReResolve;
    }
    ResolutionRevalidation::Current
}

fn candidate_set_digest_input(
    candidate_digests: &[Blake3Digest32],
) -> Result<Vec<u8>, SubjectError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/subject-candidates/v1")?;
    for digest in candidate_digests {
        append(&mut bytes, digest.as_bytes())?;
    }
    Ok(bytes)
}

fn resolution_receipt_digest_input(
    request: &SubjectRequest,
    context: &ResolutionContext,
    candidate_set_digest: Blake3Digest32,
    output_kind: ResolutionOutputKind,
    selected_candidate_digest: Option<Blake3Digest32>,
    ambiguity_candidate_digests: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    evidence_receipt_refs: &BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
) -> Result<Vec<u8>, SubjectError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/subject-resolution-receipt/v1")?;
    append(&mut bytes, request.selector_digest.as_bytes())?;
    append(&mut bytes, context.context_digest.as_bytes())?;
    append(&mut bytes, context.owner_generation_digest.as_bytes())?;
    append(&mut bytes, context.security_fence_digest.as_bytes())?;
    append(&mut bytes, candidate_set_digest.as_bytes())?;
    bytes.push(match output_kind {
        ResolutionOutputKind::Resolved => 1,
        ResolutionOutputKind::Ambiguous => 2,
        ResolutionOutputKind::NotFound => 3,
        ResolutionOutputKind::ScopeEmpty => 4,
        ResolutionOutputKind::Incomplete => 5,
    });
    match selected_candidate_digest {
        Some(digest) => {
            bytes.push(1);
            append(&mut bytes, digest.as_bytes())?;
        }
        None => bytes.push(0),
    }
    for digest in ambiguity_candidate_digests {
        append(&mut bytes, digest.as_bytes())?;
    }
    for receipt in evidence_receipt_refs {
        append(&mut bytes, receipt.as_str().as_bytes())?;
    }
    Ok(bytes)
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), SubjectError> {
    let length = u64::try_from(value.len())
        .map_err(|_| SubjectError::SubjectBudgetExhausted)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > 8 * 1024 * 1024 {
        return Err(SubjectError::SubjectBudgetExhausted);
    }
    Ok(())
}
