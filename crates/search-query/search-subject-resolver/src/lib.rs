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
    BoundedNonContentMetadata, BoundedSet, EntityKind, MAX_LIST_ITEMS, MAX_SET_ITEMS, MatchBasis,
    ReceiptRef, ResolvedSubject, SearchReasonCodeV1, SubjectAmbiguitySet,
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

/// Disclosure fence: access and purge authorization for resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisclosureFence {
    /// Current authorization permits disclosure.
    pub access_permitted: bool,
    /// No purge barrier covers the request.
    pub purge_clear: bool,
}

/// Currency fence: view and owner-generation currentness for resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrencyFence {
    /// Source/workspace context remains current.
    pub view_current: bool,
    /// Owner generation remains current.
    pub owner_generation_current: bool,
}

/// Scope and observation fence for resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeObservationFence {
    /// Explicit scope contains at least one authorized source.
    pub scope_non_empty: bool,
    /// Observation continuity is sufficient for decisive fall-through.
    pub observation_complete: bool,
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
    /// Scope and observation fence.
    pub scope_observation: ScopeObservationFence,
    /// Disclosure fence.
    pub disclosure: DisclosureFence,
    /// Currency fence.
    pub currency: CurrencyFence,
}

/// Validates one coherent resolution context.
pub fn validate_resolution_context(
    request: &SubjectRequest,
    context: &ResolutionContext,
) -> Result<(), SubjectError> {
    request.validate()?;
    if !context.scope_observation.scope_non_empty {
        return Err(SubjectError::SubjectScopeEmpty);
    }
    if !context.disclosure.access_permitted || !context.disclosure.purge_clear {
        return Err(SubjectError::SubjectAccessRevoked);
    }
    if request.requested_context_digest != context.context_digest
        || !context.currency.view_current
    {
        return Err(SubjectError::SubjectContextStale);
    }
    if !context.currency.owner_generation_current {
        return Err(SubjectError::SubjectOwnerGenerationChanged);
    }
    if !context.scope_observation.observation_complete {
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
            || self.context_digest != context.context_digest
        {
            return Err(SubjectError::SubjectReportInvalid);
        }
        if !self.authorized
            || !context.disclosure.access_permitted
            || !context.disclosure.purge_clear
        {
            return Err(SubjectError::SubjectAccessRevoked);
        }
        if !self.current
            || !context.currency.view_current
            || !context.currency.owner_generation_current
        {
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
    const fn as_error(self) -> SubjectError {
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
            ResolutionStepState::Incomplete(_) => Ok(()),
            ResolutionStepState::NotApplicable if self.candidates.is_empty() => Ok(()),
            ResolutionStepState::Complete | ResolutionStepState::NotApplicable => {
                Err(SubjectError::SubjectReportInvalid)
            }
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
    pub const fn validate(self) -> Result<Self, SubjectError> {
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
///
/// # Panics
///
/// Never panics on validated inputs; the internal `expect` on a non-empty
/// hypothesis is an internal invariant upheld via
/// `collapse_equivalent_occurrences`.
pub fn resolve_subject(
    request: &SubjectRequest,
    context: &ResolutionContext,
    steps: Vec<ResolutionStep>,
    limits: SubjectResolutionLimits,
) -> Result<SubjectResolution, SubjectError> {
    request.validate()?;
    let limits = limits.validate()?;
    if !context.scope_observation.scope_non_empty {
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
                values.into_iter().next().expect("non-empty hypothesis")
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
            .then_with(|| {
                right
                    .entity_kind_compatible
                    .cmp(&left.entity_kind_compatible)
            })
            .then_with(|| assurance_rank(right.assurance).cmp(&assurance_rank(left.assurance)))
            .then_with(|| left.portfolio_priority.cmp(&right.portfolio_priority))
            .then_with(|| {
                left.source_identity_digest
                    .cmp(&right.source_identity_digest)
            })
            .then_with(|| left.coordinate_digest.cmp(&right.coordinate_digest))
            .then_with(|| left.candidate_digest.cmp(&right.candidate_digest))
    });
}

const fn assurance_rank(value: AssuranceClass) -> u8 {
    match value {
        AssuranceClass::ExactBytes => 4,
        AssuranceClass::MappedText => 3,
        AssuranceClass::LossyText => 2,
        AssuranceClass::DescriptiveOnly => 1,
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
    let candidate_set_digest =
        Blake3Digest32::from_bytes(blake3_256(&candidate_set_digest_input(&candidate_digests)?));
    let (output_kind, selected_candidate_digest, ambiguity_candidate_digests) = match resolution {
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
        SubjectResolution::NotFound { .. } => (ResolutionOutputKind::NotFound, None, Vec::new()),
        SubjectResolution::ScopeEmpty => (ResolutionOutputKind::ScopeEmpty, None, Vec::new()),
        SubjectResolution::Incomplete { .. } => {
            (ResolutionOutputKind::Incomplete, None, Vec::new())
        }
    };
    let ambiguity_candidate_digests = BoundedList::new(ambiguity_candidate_digests)
        .map_err(|_| SubjectError::SubjectBudgetExhausted)?;
    let evidence_receipt_refs =
        BoundedList::new(evidence).map_err(|_| SubjectError::SubjectBudgetExhausted)?;
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
    let length = u64::try_from(value.len()).map_err(|_| SubjectError::SubjectBudgetExhausted)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > 8 * 1024 * 1024 {
        return Err(SubjectError::SubjectBudgetExhausted);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use search_contracts::{AssuranceClass, MatchBasis};

    use super::{ResolutionPriority, assurance_rank, rank_resolution_basis};

    #[test]
    fn assurance_order_matches_the_contract_loss_ladder() {
        assert!(
            assurance_rank(AssuranceClass::ExactBytes) > assurance_rank(AssuranceClass::MappedText)
        );
        assert!(
            assurance_rank(AssuranceClass::MappedText) > assurance_rank(AssuranceClass::LossyText)
        );
        assert!(
            assurance_rank(AssuranceClass::LossyText)
                > assurance_rank(AssuranceClass::DescriptiveOnly)
        );
    }

    #[test]
    fn semantic_similarity_is_not_subject_resolution_authority() {
        assert_eq!(rank_resolution_basis(MatchBasis::Semantic), None);
        assert_eq!(
            rank_resolution_basis(MatchBasis::Structural),
            Some(ResolutionPriority::Structural)
        );
    }
}

/// T36 baseline dictionary tests: every subject name below is a real
/// workspace package (or a filename shared by all of them), never invented
/// text. The fixtures prove the ladder, ambiguity and collapse rules that
/// the DIRECT-spine recipes rely on.
#[cfg(test)]
mod recipe_dictionary_tests {
    use search_contracts::{
        AssuranceClass, Blake3Digest32, BoundedList, BoundedName, BoundedNonContentMetadata,
        BoundedSet, EntityKind, HandleClass, HandleId, MatchBasis, NonZeroRevision,
        OpaqueHandleToken, ReceiptRef, ResolvedSubject, SearchReasonCodeV1, SearchSourceHandle,
    };

    use super::{
        CurrencyFence, DisclosureFence, ResolutionContext, ResolutionPriority, ResolutionStep,
        ResolutionStepState, ScopeObservationFence, StepIncompleteReason, SubjectCandidate,
        SubjectError, SubjectRequest, SubjectResolution, SubjectResolutionLimits,
        issue_resolution_receipt, resolve_subject,
    };

    /// Real workspace packages used as the subject dictionary.
    const CRATE_DICTIONARY: [&str; 6] = [
        "search-contracts",
        "search-access",
        "search-exact",
        "search-subject-resolver",
        "search-comparator",
        "search-query-planner",
    ];

    fn digest(first: u8, second: u8) -> Blake3Digest32 {
        let mut bytes = [0x5A; 32];
        bytes[0] = first;
        bytes[1] = second;
        Blake3Digest32::from_bytes(bytes)
    }

    /// Test-only non-crypto digest for receipt binding (determinism only).
    fn test_digest(bytes: &[u8]) -> [u8; 32] {
        let mut out = [0x33_u8; 32];
        let mut tweak = 0_u8;
        for (index, byte) in bytes.iter().enumerate() {
            out[index % 32] = out[index % 32].wrapping_add(*byte).wrapping_add(tweak);
            tweak = tweak.wrapping_add(1);
        }
        out
    }

    fn handle(seed: u8) -> SearchSourceHandle {
        SearchSourceHandle {
            handle_id: HandleId::from_bytes([seed; 16]),
            handle_revision: NonZeroRevision::new(1).expect("fixture revision"),
            handle_class: HandleClass::DurableSource,
            expires_at: None,
            opaque_token: OpaqueHandleToken::new(&[0xA5; 32]).expect("fixture token"),
        }
    }

    fn receipt(tag: &str) -> ReceiptRef {
        ReceiptRef::new(tag).expect("fixture receipt")
    }

    fn context() -> ResolutionContext {
        ResolutionContext {
            context_digest: digest(1, 1),
            owner_generation_digest: digest(2, 2),
            security_fence_digest: digest(3, 3),
            scope_observation: ScopeObservationFence {
                scope_non_empty: true,
                observation_complete: true,
            },
            disclosure: DisclosureFence {
                access_permitted: true,
                purge_clear: true,
            },
            currency: CurrencyFence {
                view_current: true,
                owner_generation_current: true,
            },
        }
    }

    fn request(steps: &[ResolutionPriority]) -> SubjectRequest {
        SubjectRequest {
            selector_digest: digest(9, 9),
            requested_context_digest: digest(1, 1),
            applicable_steps: BoundedSet::from_items(steps.iter().copied()).expect("fixture steps"),
            required_entity_kind: None,
            cancelled: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn candidate(
        seed: u8,
        name: &str,
        basis: MatchBasis,
        source_identity: (u8, u8),
        coordinate: (u8, u8),
        hypothesis: Option<(u8, u8)>,
        assurance: AssuranceClass,
        portfolio_priority: u16,
    ) -> SubjectCandidate {
        let hypothesis_digest =
            hypothesis.map_or_else(|| digest(seed, seed), |(a, b)| digest(a, b));
        SubjectCandidate {
            candidate_digest: digest(seed, seed),
            hypothesis_digest,
            subject: ResolvedSubject {
                canonical_handle: handle(seed),
                match_basis: basis,
                entity_kind: EntityKind::Function,
                normalized_name: BoundedName::new(name).expect("fixture name"),
                signature_observation: None,
                configuration_predicate: None,
            },
            match_basis: basis,
            assurance,
            entity_kind_compatible: true,
            portfolio_priority,
            source_identity_digest: digest(source_identity.0, source_identity.1),
            coordinate_digest: digest(coordinate.0, coordinate.1),
            context_digest: digest(1, 1),
            authorized: true,
            current: true,
            equivalence_receipt_ref: hypothesis.map(|_| receipt("t36-resolver-equivalence-01")),
            disambiguation_summary: BoundedNonContentMetadata::empty(),
            evidence_receipt_ref: receipt("t36-resolver-evidence-01"),
        }
    }

    fn step(
        priority: ResolutionPriority,
        state: ResolutionStepState,
        candidates: Vec<SubjectCandidate>,
    ) -> ResolutionStep {
        ResolutionStep {
            priority,
            state,
            candidates: BoundedList::new(candidates).expect("fixture candidates"),
            omitted_candidates: 0,
        }
    }

    fn limits() -> SubjectResolutionLimits {
        SubjectResolutionLimits::BASELINE
            .validate()
            .expect("baseline limits")
    }

    #[test]
    fn ambiguous_crate_filename_returns_bounded_set() {
        // `lib` exists in every workspace package: the same normalized name
        // observed in `search-comparator` and `search-subject-resolver` is
        // material ambiguity, never a first-rank win.
        let left = candidate(
            11,
            "lib",
            MatchBasis::ExactName,
            (21, 21),
            (31, 31),
            None,
            AssuranceClass::MappedText,
            1,
        );
        let right = candidate(
            12,
            "lib",
            MatchBasis::ExactName,
            (22, 22),
            (32, 32),
            None,
            AssuranceClass::MappedText,
            1,
        );
        let resolution = resolve_subject(
            &request(&[ResolutionPriority::ExactName]),
            &context(),
            vec![step(
                ResolutionPriority::ExactName,
                ResolutionStepState::Complete,
                vec![left, right],
            )],
            limits(),
        )
        .expect("ambiguity is a normal output");
        let SubjectResolution::Ambiguous {
            ambiguity,
            priority,
        } = resolution
        else {
            panic!("same-name distinct definitions must stay ambiguous");
        };
        assert_eq!(priority, ResolutionPriority::ExactName);
        assert_eq!(ambiguity.candidates.len(), 2);
        assert_eq!(ambiguity.reason_code, SearchReasonCodeV1::AmbiguousSubject);
        ambiguity.validate().expect("contract ambiguity set");
    }

    #[test]
    fn same_path_across_repos_stays_distinct_without_receipt() {
        // Paths are locators, not identity: one shared native coordinate in
        // two source identities never collapses without an equivalence
        // receipt, even when every other field agrees.
        let left = candidate(
            13,
            CRATE_DICTIONARY[4],
            MatchBasis::QualifiedName,
            (41, 41),
            (50, 50),
            None,
            AssuranceClass::ExactBytes,
            0,
        );
        let right = candidate(
            14,
            CRATE_DICTIONARY[4],
            MatchBasis::QualifiedName,
            (42, 42),
            (50, 50),
            None,
            AssuranceClass::ExactBytes,
            0,
        );
        let resolution = resolve_subject(
            &request(&[ResolutionPriority::QualifiedKey]),
            &context(),
            vec![step(
                ResolutionPriority::QualifiedKey,
                ResolutionStepState::Complete,
                vec![left, right],
            )],
            limits(),
        )
        .expect("ambiguity is a normal output");
        assert!(
            matches!(resolution, SubjectResolution::Ambiguous { .. }),
            "shared coordinate must not prove one subject: {resolution:?}"
        );
    }

    #[test]
    fn explicit_handle_outranks_lexical_candidates() {
        let explicit = candidate(
            15,
            CRATE_DICTIONARY[3],
            MatchBasis::ExplicitHandle,
            (51, 51),
            (61, 61),
            None,
            AssuranceClass::ExactBytes,
            5,
        );
        let lexical = candidate(
            16,
            CRATE_DICTIONARY[3],
            MatchBasis::Lexical,
            (52, 52),
            (62, 62),
            None,
            AssuranceClass::DescriptiveOnly,
            0,
        );
        let resolution = resolve_subject(
            &request(&[
                ResolutionPriority::ExplicitHandle,
                ResolutionPriority::Lexical,
            ]),
            &context(),
            vec![
                step(
                    ResolutionPriority::ExplicitHandle,
                    ResolutionStepState::Complete,
                    vec![explicit],
                ),
                step(
                    ResolutionPriority::Lexical,
                    ResolutionStepState::Complete,
                    vec![lexical],
                ),
            ],
            limits(),
        )
        .expect("explicit resolution");
        let SubjectResolution::Resolved {
            priority,
            candidate_digest,
            ..
        } = resolution
        else {
            panic!("explicit handle must win: {resolution:?}");
        };
        assert_eq!(priority, ResolutionPriority::ExplicitHandle);
        assert_eq!(candidate_digest, digest(15, 15));
    }

    #[test]
    fn stale_explicit_reference_never_falls_through_to_lexical() {
        let mut stale = candidate(
            17,
            CRATE_DICTIONARY[5],
            MatchBasis::ExplicitHandle,
            (53, 53),
            (63, 63),
            None,
            AssuranceClass::ExactBytes,
            0,
        );
        stale.current = false;
        let lexical = candidate(
            18,
            CRATE_DICTIONARY[5],
            MatchBasis::Lexical,
            (54, 54),
            (64, 64),
            None,
            AssuranceClass::MappedText,
            0,
        );
        let error = resolve_subject(
            &request(&[
                ResolutionPriority::ExplicitHandle,
                ResolutionPriority::Lexical,
            ]),
            &context(),
            vec![
                step(
                    ResolutionPriority::ExplicitHandle,
                    ResolutionStepState::Complete,
                    vec![stale],
                ),
                step(
                    ResolutionPriority::Lexical,
                    ResolutionStepState::Complete,
                    vec![lexical],
                ),
            ],
            limits(),
        )
        .expect_err("stale explicit evidence must fail typed");
        assert_eq!(error, SubjectError::SubjectContextStale);
    }

    #[test]
    fn qualified_key_precedes_name_and_empty_higher_falls_through() {
        let named = candidate(
            19,
            CRATE_DICTIONARY[0],
            MatchBasis::ExactName,
            (55, 55),
            (65, 65),
            None,
            AssuranceClass::MappedText,
            0,
        );
        let resolution = resolve_subject(
            &request(&[
                ResolutionPriority::QualifiedKey,
                ResolutionPriority::ExactName,
            ]),
            &context(),
            vec![
                step(
                    ResolutionPriority::QualifiedKey,
                    ResolutionStepState::Complete,
                    Vec::new(),
                ),
                step(
                    ResolutionPriority::ExactName,
                    ResolutionStepState::Complete,
                    vec![named],
                ),
            ],
            limits(),
        )
        .expect("fall-through resolution");
        assert!(
            matches!(
                resolution,
                SubjectResolution::Resolved {
                    priority: ResolutionPriority::ExactName,
                    ..
                }
            ),
            "{resolution:?}"
        );
    }

    #[test]
    fn higher_incomplete_evidence_blocks_lower_resolved_success() {
        let named = candidate(
            20,
            CRATE_DICTIONARY[1],
            MatchBasis::ExactName,
            (56, 56),
            (66, 66),
            None,
            AssuranceClass::ExactBytes,
            0,
        );
        let resolution = resolve_subject(
            &request(&[
                ResolutionPriority::QualifiedKey,
                ResolutionPriority::ExactName,
            ]),
            &context(),
            vec![
                step(
                    ResolutionPriority::QualifiedKey,
                    ResolutionStepState::Incomplete(StepIncompleteReason::Timeout),
                    Vec::new(),
                ),
                step(
                    ResolutionPriority::ExactName,
                    ResolutionStepState::Complete,
                    vec![named],
                ),
            ],
            limits(),
        )
        .expect("incomplete is a normal output");
        let SubjectResolution::Incomplete { priority, reason } = resolution else {
            panic!("higher incomplete rung must block: {resolution:?}");
        };
        assert_eq!(priority, ResolutionPriority::QualifiedKey);
        assert_eq!(reason, SubjectError::SubjectBudgetExhausted);
    }

    #[test]
    fn renamed_true_subject_collapses_only_with_accepted_receipt() {
        let renamed = |seed: u8, with_receipt: bool| {
            let hypothesis = with_receipt.then_some((77, 77));
            candidate(
                seed,
                CRATE_DICTIONARY[2],
                MatchBasis::QualifiedName,
                (57, seed),
                (67, seed),
                hypothesis,
                AssuranceClass::ExactBytes,
                0,
            )
        };
        let proven = resolve_subject(
            &request(&[ResolutionPriority::QualifiedKey]),
            &context(),
            vec![step(
                ResolutionPriority::QualifiedKey,
                ResolutionStepState::Complete,
                vec![renamed(21, true), renamed(22, true)],
            )],
            limits(),
        )
        .expect("proven collapse resolves");
        assert!(
            matches!(proven, SubjectResolution::Resolved { .. }),
            "{proven:?}"
        );
        let unproven = resolve_subject(
            &request(&[ResolutionPriority::QualifiedKey]),
            &context(),
            vec![step(
                ResolutionPriority::QualifiedKey,
                ResolutionStepState::Complete,
                vec![renamed(21, false), renamed(22, false)],
            )],
            limits(),
        )
        .expect("unproven rename stays ambiguous");
        assert!(
            matches!(unproven, SubjectResolution::Ambiguous { .. }),
            "{unproven:?}"
        );
    }

    #[test]
    fn overload_and_signature_variants_remain_distinct_when_material() {
        let first = candidate(
            23,
            CRATE_DICTIONARY[5],
            MatchBasis::Signature,
            (58, 58),
            (68, 68),
            None,
            AssuranceClass::MappedText,
            0,
        );
        let second = candidate(
            24,
            CRATE_DICTIONARY[5],
            MatchBasis::Signature,
            (59, 59),
            (69, 69),
            None,
            AssuranceClass::MappedText,
            0,
        );
        let resolution = resolve_subject(
            &request(&[ResolutionPriority::SignatureAndKind]),
            &context(),
            vec![step(
                ResolutionPriority::SignatureAndKind,
                ResolutionStepState::Complete,
                vec![first, second],
            )],
            limits(),
        )
        .expect("signature ambiguity is a normal output");
        assert!(
            matches!(
                resolution,
                SubjectResolution::Ambiguous {
                    priority: ResolutionPriority::SignatureAndKind,
                    ..
                }
            ),
            "{resolution:?}"
        );
    }

    #[test]
    fn structural_rank_gap_never_forces_resolution() {
        let top = candidate(
            25,
            CRATE_DICTIONARY[4],
            MatchBasis::Structural,
            (70, 70),
            (80, 80),
            None,
            AssuranceClass::MappedText,
            0,
        );
        let runner_up = candidate(
            26,
            CRATE_DICTIONARY[4],
            MatchBasis::Structural,
            (71, 71),
            (81, 81),
            None,
            AssuranceClass::DescriptiveOnly,
            9,
        );
        let resolution = resolve_subject(
            &request(&[ResolutionPriority::Structural]),
            &context(),
            vec![step(
                ResolutionPriority::Structural,
                ResolutionStepState::Complete,
                vec![top, runner_up],
            )],
            limits(),
        )
        .expect("structural ambiguity is a normal output");
        assert!(
            matches!(resolution, SubjectResolution::Ambiguous { .. }),
            "rank gap must not select: {resolution:?}"
        );
    }

    #[test]
    fn truncation_is_explicit_and_never_complete_ambiguity() {
        let pair = vec![
            candidate(
                27,
                "lib",
                MatchBasis::ExactName,
                (72, 72),
                (82, 82),
                None,
                AssuranceClass::MappedText,
                0,
            ),
            candidate(
                28,
                "lib",
                MatchBasis::ExactName,
                (73, 73),
                (83, 83),
                None,
                AssuranceClass::MappedText,
                0,
            ),
        ];
        let tight = SubjectResolutionLimits {
            max_ambiguity_candidates: 1,
            ..limits()
        };
        let error = resolve_subject(
            &request(&[ResolutionPriority::ExactName]),
            &context(),
            vec![step(
                ResolutionPriority::ExactName,
                ResolutionStepState::Complete,
                pair,
            )],
            tight,
        )
        .expect_err("truncation must fail typed");
        assert_eq!(error, SubjectError::SubjectAmbiguityTruncated);
    }

    #[test]
    fn fence_drift_fails_typed_before_any_resolution() {
        let named = candidate(
            29,
            CRATE_DICTIONARY[0],
            MatchBasis::ExactName,
            (74, 74),
            (84, 84),
            None,
            AssuranceClass::ExactBytes,
            0,
        );
        let steps = || {
            vec![step(
                ResolutionPriority::ExactName,
                ResolutionStepState::Complete,
                vec![named.clone()],
            )]
        };
        let mut stale_view = context();
        stale_view.context_digest = digest(8, 8);
        assert_eq!(
            resolve_subject(
                &request(&[ResolutionPriority::ExactName]),
                &stale_view,
                steps(),
                limits()
            )
            .expect_err("stale view"),
            SubjectError::SubjectContextStale
        );
        let mut revoked = context();
        revoked.disclosure.access_permitted = false;
        assert_eq!(
            resolve_subject(
                &request(&[ResolutionPriority::ExactName]),
                &revoked,
                steps(),
                limits()
            )
            .expect_err("revoked access"),
            SubjectError::SubjectAccessRevoked
        );
        let mut empty = context();
        empty.scope_observation.scope_non_empty = false;
        assert!(
            matches!(
                resolve_subject(
                    &request(&[ResolutionPriority::ExactName]),
                    &empty,
                    steps(),
                    limits()
                ),
                Ok(SubjectResolution::ScopeEmpty)
            ),
            "empty scope is explicit"
        );
    }

    #[test]
    fn equal_inputs_yield_equal_resolution_and_receipt_bytes() {
        let pair = || {
            vec![
                candidate(
                    30,
                    CRATE_DICTIONARY[3],
                    MatchBasis::ExactName,
                    (75, 75),
                    (85, 85),
                    None,
                    AssuranceClass::MappedText,
                    2,
                ),
                candidate(
                    31,
                    CRATE_DICTIONARY[3],
                    MatchBasis::ExactName,
                    (76, 76),
                    (86, 86),
                    None,
                    AssuranceClass::MappedText,
                    1,
                ),
            ]
        };
        let run = |candidates: Vec<SubjectCandidate>| {
            let resolution = resolve_subject(
                &request(&[ResolutionPriority::ExactName]),
                &context(),
                vec![step(
                    ResolutionPriority::ExactName,
                    ResolutionStepState::Complete,
                    candidates.clone(),
                )],
                limits(),
            )
            .expect("deterministic ambiguity");
            let receipt = issue_resolution_receipt(
                &request(&[ResolutionPriority::ExactName]),
                &context(),
                &resolution,
                candidates,
                limits(),
                test_digest,
            )
            .expect("deterministic receipt");
            (resolution, receipt.receipt_digest)
        };
        let (first, first_digest) = run(pair());
        let mut reversed = pair();
        reversed.reverse();
        let (second, second_digest) = run(reversed);
        assert_eq!(first, second);
        assert_eq!(first_digest, second_digest);
    }
}
