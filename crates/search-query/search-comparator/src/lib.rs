//! Lineage-aware descriptive comparison of validated implementations.
//!
//! The comparator collapses proven forks/mirrors/copies before counting
//! independent evidence, preserves evidence roles and configuration variants,
//! emits conflicts and unknowns explicitly, and never declares a correct,
//! preferred, best, or adoptable implementation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::cmp::Reverse;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    AssuranceClass, BehaviorComparison, BehaviorConflict, BehaviorObservation,
    Blake3Digest32, BoundedBehaviorSignature, BoundedList,
    BoundedNonContentMetadata, BoundedObservation, BoundedSet,
    ComparableImplementation, ComparisonAxis, CoverageUnknown,
    CrossRepositoryBehaviorSet, EvidenceRole, LocalComparisonSubject,
    MAX_LIST_ITEMS, MAX_REASON_CODES, MAX_SET_ITEMS, MatchBasis, OpaqueId,
    PortfolioRevision, ReceiptRef, RecipeResultHeader, ReferencePortfolioId,
    RepositoryLineageId, SearchReasonCodeV1, SearchSourceHandle,
};
use search_subject_resolver::SubjectResolution;

/// Closed comparison failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompareError {
    /// Local subject remains ambiguous or was not explicitly selected.
    AmbiguousSubject,
    /// Authorized reference scope contains no candidate lineage.
    ComparisonScopeEmpty,
    /// Request is malformed, contradictory, or attempts a normative verdict.
    ComparisonRequestInvalid,
    /// Source, subject, plan, security, or portfolio context is stale.
    ComparisonContextStale,
    /// Candidate lacks exact source-backed evidence or accepted analogue basis.
    ComparableEvidenceInvalid,
    /// No candidate has sufficient comparable evidence for a requested axis.
    InsufficientComparableEvidence,
    /// Lineage relation is ambiguous and cannot be collapsed or counted.
    LineageAmbiguous,
    /// Lineage/copy receipt is stale or mismatched.
    LineageReceiptStale,
    /// Configuration applicability is unknown or contradictory.
    ConfigurationAmbiguous,
    /// Requested axis is outside the closed profile.
    ComparisonAxisUnsupported,
    /// A finite candidate, role, axis, reading, or output limit was exhausted.
    ComparisonBudgetExhausted,
    /// Explicit cancellation was observed.
    ComparisonCancelled,
    /// Portfolio, axis, role, lineage, or local-absence coverage is incomplete.
    IncompleteCoverage,
    /// An evidence handle is missing, expired, or unauthorized.
    HandleUnavailable,
    /// A correctness, best, adoption, or recommendation verdict was requested.
    NormativeVerdictForbidden,
    /// Output accounting or a shared contract is contradictory.
    ComparisonReportInvalid,
}

impl CompareError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AmbiguousSubject => "AMBIGUOUS_SUBJECT",
            Self::ComparisonScopeEmpty => "COMPARISON_SCOPE_EMPTY",
            Self::ComparisonRequestInvalid => "COMPARISON_REQUEST_INVALID",
            Self::ComparisonContextStale => "COMPARISON_CONTEXT_STALE",
            Self::ComparableEvidenceInvalid => "COMPARABLE_EVIDENCE_INVALID",
            Self::InsufficientComparableEvidence => "INSUFFICIENT_COMPARABLE_EVIDENCE",
            Self::LineageAmbiguous => "LINEAGE_AMBIGUOUS",
            Self::LineageReceiptStale => "LINEAGE_RECEIPT_STALE",
            Self::ConfigurationAmbiguous => "CONFIGURATION_AMBIGUOUS",
            Self::ComparisonAxisUnsupported => "COMPARISON_AXIS_UNSUPPORTED",
            Self::ComparisonBudgetExhausted => "COMPARISON_BUDGET_EXHAUSTED",
            Self::ComparisonCancelled => "COMPARISON_CANCELLED",
            Self::IncompleteCoverage => "INCOMPLETE_COVERAGE",
            Self::HandleUnavailable => "HANDLE_UNAVAILABLE",
            Self::NormativeVerdictForbidden => "NORMATIVE_VERDICT_FORBIDDEN",
            Self::ComparisonReportInvalid => "COMPARISON_REPORT_INVALID",
        }
    }
}

impl fmt::Display for CompareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CompareError {}

/// Finite comparison resource policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonLimits {
    /// Maximum candidate implementations before lineage collapse.
    pub max_implementations: usize,
    /// Maximum independent lineage groups.
    pub max_lineages: usize,
    /// Maximum observations per implementation.
    pub max_observations_per_implementation: usize,
    /// Maximum total observations.
    pub max_observations: usize,
    /// Maximum evidence handles per normalized observation.
    pub max_handles_per_observation: usize,
    /// Maximum ambiguity/unknown rows.
    pub max_unknowns: usize,
    /// Maximum recommended reading handles.
    pub max_reading_handles: usize,
    /// Maximum reading handles from one lineage.
    pub max_reading_per_lineage: usize,
    /// Maximum reading handles from one source identity.
    pub max_reading_per_source: usize,
}

impl ComparisonLimits {
    /// Conservative baseline.
    pub const BASELINE: Self = Self {
        max_implementations: 1_024,
        max_lineages: 512,
        max_observations_per_implementation: 256,
        max_observations: MAX_LIST_ITEMS,
        max_handles_per_observation: 64,
        max_unknowns: 256,
        max_reading_handles: 256,
        max_reading_per_lineage: 8,
        max_reading_per_source: 4,
    };

    /// Validates every finite dimension.
    pub fn validate(self) -> Result<Self, CompareError> {
        let valid = self.max_implementations > 0
            && self.max_implementations <= MAX_LIST_ITEMS
            && self.max_lineages > 0
            && self.max_lineages <= self.max_implementations
            && self.max_observations_per_implementation > 0
            && self.max_observations_per_implementation <= MAX_LIST_ITEMS
            && self.max_observations > 0
            && self.max_observations <= MAX_LIST_ITEMS
            && self.max_handles_per_observation > 0
            && self.max_handles_per_observation <= MAX_LIST_ITEMS
            && self.max_unknowns > 0
            && self.max_unknowns <= MAX_LIST_ITEMS
            && self.max_reading_handles > 0
            && self.max_reading_handles <= MAX_LIST_ITEMS
            && self.max_reading_per_lineage > 0
            && self.max_reading_per_lineage <= self.max_reading_handles
            && self.max_reading_per_source > 0
            && self.max_reading_per_source <= self.max_reading_handles;
        if valid {
            Ok(self)
        } else {
            Err(CompareError::ComparisonBudgetExhausted)
        }
    }
}

/// Versioned descriptive comparison profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonProfile {
    /// Stable profile identifier.
    pub profile_id: search_contracts::ProfileId,
    /// Exact behavior-affecting profile digest.
    pub profile_digest: Blake3Digest32,
    /// Closed supported axes.
    pub supported_axes: BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
    /// Minimum assurance for a source-backed conflict.
    pub minimum_conflict_assurance: AssuranceClass,
    /// Minimum assurance for shared/corroborated classification.
    pub minimum_shared_assurance: AssuranceClass,
    /// Accepted profile qualification receipt.
    pub qualification_receipt_ref: ReceiptRef,
}

/// Normalized comparison request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonRequest {
    /// Digest of the selected local subject resolution.
    pub local_subject_resolution_digest: Blake3Digest32,
    /// Exact reference portfolio.
    pub portfolio_id: ReferencePortfolioId,
    /// Exact immutable portfolio revision.
    pub portfolio_revision: PortfolioRevision,
    /// Requested axes.
    pub axes: BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
    /// Exact source/reference view digest.
    pub source_view_digest: Blake3Digest32,
    /// Exact source-owner generation set digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Exact grant/access/live-deny/purge/shadow digest.
    pub security_fence_digest: Blake3Digest32,
    /// Reference inventory enumeration completed.
    pub reference_inventory_complete: bool,
    /// Number of explicitly omitted candidate memberships.
    pub omitted_memberships: u64,
    /// Number of unresolved candidate memberships.
    pub unknown_memberships: u64,
    /// Whether the request asks for a normative correctness/adoption verdict.
    pub normative_verdict_requested: bool,
    /// Whether cancellation was already observed.
    pub cancelled: bool,
}

/// Validated request carrying the selected local subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedComparisonRequest {
    /// Request.
    pub request: ComparisonRequest,
    /// Uniquely selected local subject.
    pub local_subject: LocalComparisonSubject,
}

/// Current context required before comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonLiveState {
    /// Current source/reference view digest.
    pub source_view_digest: Blake3Digest32,
    /// Current source-owner generation digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Current security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Current portfolio revision.
    pub portfolio_revision: PortfolioRevision,
    /// Current authorization permits comparison and handle disclosure.
    pub access_permitted: bool,
    /// No purge barrier covers the comparison scope.
    pub purge_clear: bool,
    /// Observation continuity remains current.
    pub observation_current: bool,
}

/// Validates request, local resolution, profile, and current fences.
pub fn validate_comparison_request(
    request: ComparisonRequest,
    local_resolution: &SubjectResolution,
    local_subject: LocalComparisonSubject,
    profile: &ComparisonProfile,
    live: ComparisonLiveState,
) -> Result<ValidatedComparisonRequest, CompareError> {
    if request.cancelled {
        return Err(CompareError::ComparisonCancelled);
    }
    if request.normative_verdict_requested {
        return Err(CompareError::NormativeVerdictForbidden);
    }
    if request.axes.is_empty() {
        return Err(CompareError::ComparisonRequestInvalid);
    }
    if request
        .axes
        .iter()
        .any(|axis| !profile.supported_axes.contains(axis))
    {
        return Err(CompareError::ComparisonAxisUnsupported);
    }
    match local_resolution {
        SubjectResolution::Resolved { subject, .. }
            if subject == &local_subject.resolved_subject => {}
        SubjectResolution::Ambiguous { .. } => return Err(CompareError::AmbiguousSubject),
        SubjectResolution::ScopeEmpty => return Err(CompareError::ComparisonScopeEmpty),
        SubjectResolution::NotFound { .. } | SubjectResolution::Incomplete { .. } => {
            return Err(CompareError::ComparableEvidenceInvalid);
        }
        SubjectResolution::Resolved { .. } => {
            return Err(CompareError::ComparableEvidenceInvalid);
        }
    }
    if !live.access_permitted || !live.purge_clear {
        return Err(CompareError::ComparisonContextStale);
    }
    if !live.observation_current
        || request.source_view_digest != live.source_view_digest
        || request.owner_generation_digest != live.owner_generation_digest
        || request.security_fence_digest != live.security_fence_digest
        || request.portfolio_revision != live.portfolio_revision
    {
        return Err(CompareError::ComparisonContextStale);
    }
    Ok(ValidatedComparisonRequest {
        request,
        local_subject,
    })
}

/// One source-backed behavior observation with stable comparison identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparableObservation {
    /// Contract observation.
    pub observation: BehaviorObservation,
    /// Digest of normalized descriptive observation semantics.
    pub observation_digest: Blake3Digest32,
    /// Digest of exact configuration applicability, when known.
    pub configuration_digest: Option<Blake3Digest32>,
    /// Evidence role that produced the observation.
    pub evidence_role: EvidenceRole,
    /// Exact source-backed validation receipt.
    pub validation_receipt_ref: ReceiptRef,
    /// Exact revision/handle readback succeeded.
    pub exact_readback_valid: bool,
    /// Current authorization permits disclosure.
    pub authorized: bool,
    /// Observation and handle remain current.
    pub current: bool,
}

/// Candidate implementation plus validation evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparableCandidate {
    /// Shared comparison result shape.
    pub implementation: ComparableImplementation,
    /// Digest of the resolved subject hypothesis.
    pub subject_hypothesis_digest: Blake3Digest32,
    /// Requested entity kind/signature constraints are compatible.
    pub entity_kind_and_signature_compatible: bool,
    /// Accepted analogue receipt for weak name/lexical bases.
    pub analogue_receipt_ref: Option<ReceiptRef>,
    /// Exact source revision and handle validation succeeded.
    pub exact_revision_valid: bool,
    /// Current authorization permits this implementation.
    pub authorized: bool,
    /// Source/view/owner generation remains current.
    pub current: bool,
    /// Source-backed observations.
    pub observations: BoundedList<ComparableObservation, MAX_LIST_ITEMS>,
    /// Candidate validation receipts.
    pub evidence_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
}

/// Validates one comparable implementation without treating name equality as proof.
pub fn validate_comparable_implementation(
    candidate: &ComparableCandidate,
    local_subject_digest: Blake3Digest32,
    requested_axes: &BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
    limits: ComparisonLimits,
) -> Result<(), CompareError> {
    let limits = limits.validate()?;
    if !candidate.authorized {
        return Err(CompareError::HandleUnavailable);
    }
    if !candidate.current || !candidate.exact_revision_valid {
        return Err(CompareError::ComparableEvidenceInvalid);
    }
    if candidate.observations.is_empty()
        || candidate.observations.len() > limits.max_observations_per_implementation
        || candidate.implementation.exact_handles.is_empty()
    {
        return Err(CompareError::InsufficientComparableEvidence);
    }
    let strong_basis = matches!(
        candidate.implementation.match_basis,
        MatchBasis::ExplicitHandle
            | MatchBasis::EditorPosition
            | MatchBasis::QualifiedName
            | MatchBasis::Signature
            | MatchBasis::Structural
    );
    let admitted_weak_basis = matches!(
        candidate.implementation.match_basis,
        MatchBasis::ExactName | MatchBasis::Lexical
    ) && candidate.analogue_receipt_ref.is_some();
    if !candidate.entity_kind_and_signature_compatible
        || (!strong_basis && !admitted_weak_basis)
        || matches!(candidate.implementation.match_basis, MatchBasis::Semantic)
    {
        return Err(CompareError::ComparableEvidenceInvalid);
    }
    if candidate.subject_hypothesis_digest == local_subject_digest
        && candidate.analogue_receipt_ref.is_none()
        && !strong_basis
    {
        return Err(CompareError::ComparableEvidenceInvalid);
    }
    for observation in &candidate.observations {
        if !requested_axes.contains(&observation.observation.axis)
            || !observation.exact_readback_valid
            || !observation.authorized
            || !observation.current
            || observation.observation.evidence_handles.is_empty()
            || observation.observation.evidence_handles.len()
                > limits.max_handles_per_observation
            || observation.observation.summary.as_str().is_empty()
        {
            return Err(CompareError::ComparableEvidenceInvalid);
        }
    }
    Ok(())
}

/// Qualified relation between two repository lineages.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LineageRelationKind {
    /// Same independent implementation lineage.
    SameLineage,
    /// Fork relation proven from immutable repository history.
    Fork,
    /// Mirror relation proven from immutable repository history.
    Mirror,
    /// Copy relation proven by accepted lineage evidence.
    ProvenCopy,
    /// Relation remains ambiguous and must not collapse or count as independent.
    Ambiguous,
    /// Distinct independent lineages.
    Independent,
}

/// Accepted lineage relation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageRelationReceipt {
    /// Left lineage.
    pub left: RepositoryLineageId,
    /// Right lineage.
    pub right: RepositoryLineageId,
    /// Qualified relation.
    pub relation: LineageRelationKind,
    /// Immutable relation-evidence digest.
    pub evidence_digest: Blake3Digest32,
    /// Exact portfolio revision whose graph was evaluated.
    pub portfolio_revision: PortfolioRevision,
    /// Relation receipt.
    pub receipt_ref: ReceiptRef,
    /// Receipt remains current.
    pub current: bool,
}

/// One independent lineage group after proven collapse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageGroup {
    /// Deterministically selected representative lineage.
    pub representative_lineage_id: RepositoryLineageId,
    /// Collapsed lineage members.
    pub lineage_ids: BoundedList<RepositoryLineageId, MAX_LIST_ITEMS>,
    /// Candidate implementations in stable order.
    pub implementations: BoundedList<ComparableCandidate, MAX_LIST_ITEMS>,
    /// Receipts that justified collapse.
    pub collapse_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
}

/// Complete lineage-collapse result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageGroupSet {
    /// Independent groups.
    pub groups: BoundedList<LineageGroup, MAX_LIST_ITEMS>,
    /// Ambiguous lineages excluded from confident independent counts.
    pub ambiguous_lineages: BoundedList<RepositoryLineageId, MAX_LIST_ITEMS>,
    /// Distinct relation receipts used.
    pub relation_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
}

/// Collapses only exact same/fork/mirror/copy relations with current receipts.
pub fn collapse_repository_lineages(
    candidates: Vec<ComparableCandidate>,
    receipts: &[LineageRelationReceipt],
    portfolio_revision: PortfolioRevision,
    limits: ComparisonLimits,
) -> Result<LineageGroupSet, CompareError> {
    let limits = limits.validate()?;
    if candidates.is_empty() {
        return Err(CompareError::ComparisonScopeEmpty);
    }
    if candidates.len() > limits.max_implementations {
        return Err(CompareError::ComparisonBudgetExhausted);
    }
    let lineages = candidates
        .iter()
        .map(|candidate| candidate.implementation.lineage_id)
        .collect::<BTreeSet<_>>();
    let mut parent = lineages
        .iter()
        .copied()
        .map(|lineage| (lineage, lineage))
        .collect::<BTreeMap<_, _>>();
    let mut ambiguous = BTreeSet::new();
    let mut used_receipts = BTreeSet::new();

    for receipt in receipts {
        if !lineages.contains(&receipt.left) || !lineages.contains(&receipt.right) {
            continue;
        }
        if !receipt.current || receipt.portfolio_revision != portfolio_revision {
            return Err(CompareError::LineageReceiptStale);
        }
        match receipt.relation {
            LineageRelationKind::SameLineage
            | LineageRelationKind::Fork
            | LineageRelationKind::Mirror
            | LineageRelationKind::ProvenCopy => {
                union(&mut parent, receipt.left, receipt.right);
                used_receipts.insert(receipt.receipt_ref.clone());
            }
            LineageRelationKind::Ambiguous => {
                ambiguous.insert(receipt.left);
                ambiguous.insert(receipt.right);
                used_receipts.insert(receipt.receipt_ref.clone());
            }
            LineageRelationKind::Independent => {}
        }
    }

    let mut grouped: BTreeMap<RepositoryLineageId, Vec<ComparableCandidate>> = BTreeMap::new();
    for candidate in candidates {
        let root = find_root(&mut parent, candidate.implementation.lineage_id);
        grouped.entry(root).or_default().push(candidate);
    }
    if grouped.len() > limits.max_lineages {
        return Err(CompareError::ComparisonBudgetExhausted);
    }

    let mut groups = Vec::new();
    for (root, mut implementations) in grouped {
        implementations.sort_by(candidate_order);
        let mut lineage_ids = implementations
            .iter()
            .map(|candidate| candidate.implementation.lineage_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        lineage_ids.sort_unstable();
        let representative = lineage_ids.first().copied().unwrap_or(root);
        let collapse_receipt_refs = receipts
            .iter()
            .filter(|receipt| {
                lineage_ids.contains(&receipt.left)
                    && lineage_ids.contains(&receipt.right)
                    && matches!(
                        receipt.relation,
                        LineageRelationKind::SameLineage
                            | LineageRelationKind::Fork
                            | LineageRelationKind::Mirror
                            | LineageRelationKind::ProvenCopy
                    )
            })
            .map(|receipt| receipt.receipt_ref.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        groups.push(LineageGroup {
            representative_lineage_id: representative,
            lineage_ids: bounded(lineage_ids)?,
            implementations: bounded(implementations)?,
            collapse_receipt_refs: bounded(collapse_receipt_refs)?,
        });
    }
    groups.sort_by_key(|group| group.representative_lineage_id);
    Ok(LineageGroupSet {
        groups: bounded(groups)?,
        ambiguous_lineages: bounded(ambiguous.into_iter().collect())?,
        relation_receipt_refs: bounded(used_receipts.into_iter().collect())?,
    })
}

fn find_root(
    parent: &mut BTreeMap<RepositoryLineageId, RepositoryLineageId>,
    value: RepositoryLineageId,
) -> RepositoryLineageId {
    let mut current = value;
    while parent.get(&current).copied().unwrap_or(current) != current {
        current = parent.get(&current).copied().unwrap_or(current);
    }
    let root = current;
    let mut current = value;
    while let Some(next) = parent.get(&current).copied() {
        parent.insert(current, root);
        if next == current {
            break;
        }
        current = next;
    }
    root
}

fn union(
    parent: &mut BTreeMap<RepositoryLineageId, RepositoryLineageId>,
    left: RepositoryLineageId,
    right: RepositoryLineageId,
) {
    let left_root = find_root(parent, left);
    let right_root = find_root(parent, right);
    let root = left_root.min(right_root);
    let other = left_root.max(right_root);
    parent.insert(other, root);
}

fn candidate_order(
    left: &ComparableCandidate,
    right: &ComparableCandidate,
) -> core::cmp::Ordering {
    match_basis_rank(left.implementation.match_basis)
        .cmp(&match_basis_rank(right.implementation.match_basis))
        .then_with(|| {
            right
                .implementation
                .exact_handles
                .len()
                .cmp(&left.implementation.exact_handles.len())
        })
        .then_with(|| {
            left.implementation
                .lineage_id
                .cmp(&right.implementation.lineage_id)
        })
        .then_with(|| left.subject_hypothesis_digest.cmp(&right.subject_hypothesis_digest))
}

/// One normalized axis component in a behavior signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorSignatureComponent {
    /// Comparison axis.
    pub axis: ComparisonAxis,
    /// Normalized observation digest.
    pub observation_digest: Blake3Digest32,
    /// Exact configuration predicate digest, when known.
    pub configuration_digest: Option<Blake3Digest32>,
    /// Evidence role.
    pub evidence_role: EvidenceRole,
    /// Assurance ceiling.
    pub assurance: AssuranceClass,
    /// Exact source-backed validation receipt.
    pub validation_receipt_ref: ReceiptRef,
}

/// Deterministic structured signature plus shared bounded serialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorSignature {
    /// Exact profile digest.
    pub profile_digest: Blake3Digest32,
    /// Structured components in canonical order.
    pub components: BoundedList<BehaviorSignatureComponent, MAX_LIST_ITEMS>,
    /// Closed axes missing from this implementation.
    pub unknown_axes: BoundedList<ComparisonAxis, MAX_LIST_ITEMS>,
    /// Shared contract signature.
    pub contract_signature: BoundedBehaviorSignature,
    /// Digest of exact canonical signature bytes.
    pub signature_digest: Blake3Digest32,
}

/// Normalizes one implementation without converting missing evidence to absence.
pub fn normalize_behavior_signature(
    candidate: &ComparableCandidate,
    request: &ValidatedComparisonRequest,
    profile: &ComparisonProfile,
    limits: ComparisonLimits,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<BehaviorSignature, CompareError> {
    validate_comparable_implementation(
        candidate,
        request.request.local_subject_resolution_digest,
        &request.request.axes,
        limits,
    )?;
    let mut components = candidate
        .observations
        .iter()
        .map(|value| BehaviorSignatureComponent {
            axis: value.observation.axis,
            observation_digest: value.observation_digest,
            configuration_digest: value.configuration_digest,
            evidence_role: value.evidence_role,
            assurance: value.observation.assurance,
            validation_receipt_ref: value.validation_receipt_ref.clone(),
        })
        .collect::<Vec<_>>();
    components.sort_by(|left, right| {
        left.axis
            .cmp(&right.axis)
            .then_with(|| left.configuration_digest.cmp(&right.configuration_digest))
            .then_with(|| left.evidence_role.cmp(&right.evidence_role))
            .then_with(|| left.observation_digest.cmp(&right.observation_digest))
    });
    components.dedup();
    let observed_axes = components
        .iter()
        .map(|component| component.axis)
        .collect::<BTreeSet<_>>();
    let unknown_axes = request
        .request
        .axes
        .iter()
        .copied()
        .filter(|axis| !observed_axes.contains(axis))
        .collect::<Vec<_>>();
    let bytes = signature_bytes(profile.profile_digest, &components, &unknown_axes)?;
    let text = signature_text(profile.profile_digest, &components, &unknown_axes)?;
    Ok(BehaviorSignature {
        profile_digest: profile.profile_digest,
        components: bounded(components)?,
        unknown_axes: bounded(unknown_axes)?,
        contract_signature: BoundedBehaviorSignature::new(text)
            .map_err(|_| CompareError::ComparisonBudgetExhausted)?,
        signature_digest: Blake3Digest32::from_bytes(blake3_256(&bytes)),
    })
}

fn signature_bytes(
    profile_digest: Blake3Digest32,
    components: &[BehaviorSignatureComponent],
    unknown_axes: &[ComparisonAxis],
) -> Result<Vec<u8>, CompareError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/behavior-signature/v1")?;
    bytes.extend_from_slice(profile_digest.as_bytes());
    for component in components {
        bytes.push(axis_tag(component.axis));
        bytes.extend_from_slice(component.observation_digest.as_bytes());
        match component.configuration_digest {
            Some(digest) => {
                bytes.push(1);
                bytes.extend_from_slice(digest.as_bytes());
            }
            None => bytes.push(0),
        }
        bytes.push(role_tag(component.evidence_role));
        bytes.push(assurance_rank(component.assurance));
        append(
            &mut bytes,
            component.validation_receipt_ref.as_str().as_bytes(),
        )?;
    }
    bytes.push(0xff);
    for axis in unknown_axes {
        bytes.push(axis_tag(*axis));
    }
    Ok(bytes)
}

fn signature_text(
    profile_digest: Blake3Digest32,
    components: &[BehaviorSignatureComponent],
    unknown_axes: &[ComparisonAxis],
) -> Result<String, CompareError> {
    let mut text = format!("profile={profile_digest};");
    for component in components {
        use fmt::Write as _;
        write!(
            text,
            "axis={};obs={};cfg={};role={};assurance={};",
            component.axis.as_str(),
            component.observation_digest,
            component
                .configuration_digest
                .map_or_else(|| "unknown".to_owned(), |digest| digest.to_string()),
            role_name(component.evidence_role),
            component.assurance.as_str(),
        )
        .map_err(|_| CompareError::ComparisonReportInvalid)?;
    }
    if !unknown_axes.is_empty() {
        text.push_str("unknown=");
        for (index, axis) in unknown_axes.iter().enumerate() {
            if index > 0 {
                text.push(',');
            }
            text.push_str(axis.as_str());
        }
        text.push(';');
    }
    Ok(text)
}

/// Evidence grouped by role inside one collapsed lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRoleAlignment {
    /// Independent lineage represented by this alignment.
    pub lineage_id: RepositoryLineageId,
    /// Axis/role groups in canonical order.
    pub groups: BoundedList<EvidenceRoleGroup, MAX_LIST_ITEMS>,
    /// Axes with no evidence after collapse.
    pub unknown_axes: BoundedList<ComparisonAxis, MAX_LIST_ITEMS>,
}

/// One axis/role evidence group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRoleGroup {
    /// Axis.
    pub axis: ComparisonAxis,
    /// Evidence role.
    pub role: EvidenceRole,
    /// Distinct normalized observation digests.
    pub observation_digests: BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    /// Exact evidence handles.
    pub handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    /// Whether observations in this role disagree materially.
    pub internally_conflicting: bool,
}

/// Aligns role evidence without treating tests or documentation as truth authority.
pub fn align_evidence_roles(
    group: &LineageGroup,
    requested_axes: &BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
    limits: ComparisonLimits,
) -> Result<EvidenceRoleAlignment, CompareError> {
    let limits = limits.validate()?;
    let mut buckets: BTreeMap<(ComparisonAxis, EvidenceRole), Vec<&ComparableObservation>> =
        BTreeMap::new();
    for implementation in &group.implementations {
        for observation in &implementation.observations {
            buckets
                .entry((observation.observation.axis, observation.evidence_role))
                .or_default()
                .push(observation);
        }
    }
    let mut groups = Vec::new();
    let mut observed_axes = BTreeSet::new();
    for ((axis, role), observations) in buckets {
        observed_axes.insert(axis);
        let digests = observations
            .iter()
            .map(|observation| observation.observation_digest)
            .collect::<BTreeSet<_>>();
        let handles = dedupe_handles(
            observations
                .iter()
                .flat_map(|observation| observation.observation.evidence_handles.iter().cloned()),
            limits.max_handles_per_observation,
        )?;
        groups.push(EvidenceRoleGroup {
            axis,
            role,
            internally_conflicting: digests.len() > 1,
            observation_digests: bounded(digests.into_iter().collect())?,
            handles,
        });
    }
    let unknown_axes = requested_axes
        .iter()
        .copied()
        .filter(|axis| !observed_axes.contains(axis))
        .collect::<Vec<_>>();
    Ok(EvidenceRoleAlignment {
        lineage_id: group.representative_lineage_id,
        groups: bounded(groups)?,
        unknown_axes: bounded(unknown_axes)?,
    })
}

/// Relationship between two exact configuration predicates.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PredicateRelation {
    /// Same exact applicability predicate.
    Equivalent,
    /// Predicates are proven mutually exclusive in the accepted context.
    MutuallyExclusive,
    /// Predicates overlap in the accepted context.
    Overlapping,
    /// At least one predicate is absent/unknown or relation evidence is incomplete.
    Unknown,
}

/// Closed decision for two observations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BehaviorComparisonDecision {
    /// Normalized observations are equivalent.
    EquivalentObservation,
    /// Difference is explained by mutually exclusive configuration.
    ConfigurationVariant,
    /// Difference is source-backed but not contradictory.
    MaterialVariant,
    /// Overlapping source-backed observations are incompatible.
    Conflict,
    /// Assurance cannot support a conflict/variant decision.
    InsufficientAssurance,
    /// Applicability or evidence is unknown.
    Unknown,
}

/// Classifies two source-backed observations without declaring a winner.
#[must_use]
pub fn classify_conflict(
    left: &ComparableObservation,
    right: &ComparableObservation,
    relation: PredicateRelation,
    minimum_conflict_assurance: AssuranceClass,
) -> BehaviorComparisonDecision {
    if left.observation.axis != right.observation.axis
        || !left.authorized
        || !right.authorized
        || !left.current
        || !right.current
        || !left.exact_readback_valid
        || !right.exact_readback_valid
    {
        return BehaviorComparisonDecision::Unknown;
    }
    if left.observation_digest == right.observation_digest
        && relation != PredicateRelation::Unknown
    {
        return BehaviorComparisonDecision::EquivalentObservation;
    }
    if relation == PredicateRelation::MutuallyExclusive {
        return BehaviorComparisonDecision::ConfigurationVariant;
    }
    if relation == PredicateRelation::Unknown {
        return BehaviorComparisonDecision::Unknown;
    }
    if assurance_rank(left.observation.assurance)
        < assurance_rank(minimum_conflict_assurance)
        || assurance_rank(right.observation.assurance)
            < assurance_rank(minimum_conflict_assurance)
    {
        return BehaviorComparisonDecision::InsufficientAssurance;
    }
    if relation == PredicateRelation::Overlapping {
        BehaviorComparisonDecision::Conflict
    } else {
        BehaviorComparisonDecision::MaterialVariant
    }
}

/// Exact relation evidence for configuration predicates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateRelationReceipt {
    /// First normalized observation digest.
    pub left_observation_digest: Blake3Digest32,
    /// Second normalized observation digest.
    pub right_observation_digest: Blake3Digest32,
    /// Accepted predicate relationship.
    pub relation: PredicateRelation,
    /// Exact configuration-context digest.
    pub context_digest: Blake3Digest32,
    /// Receipt remains current.
    pub current: bool,
    /// Source-backed relation receipt.
    pub receipt_ref: ReceiptRef,
}

/// Local source-backed behavior observations and absence evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBehaviorEvidence {
    /// Local contract subject.
    pub subject: LocalComparisonSubject,
    /// Local observations.
    pub observations: BoundedList<ComparableObservation, MAX_LIST_ITEMS>,
    /// Axes proven complete by an exact frozen-denominator report.
    pub exact_complete_axes: BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
    /// Exact proof receipts for complete local axes.
    pub exact_absence_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
}

/// Explicit comparison coverage gap.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComparisonGap {
    /// Reference inventory omitted memberships.
    OmittedMemberships,
    /// Reference inventory contains unknown memberships.
    UnknownMemberships,
    /// One or more lineage relations are ambiguous.
    AmbiguousLineage,
    /// Requested axis has no independent comparable evidence.
    MissingAxisEvidence(ComparisonAxis),
    /// Local absence lacks an exact complete proof.
    LocalAbsenceUnproven(ComparisonAxis),
    /// Configuration relation is unknown.
    ConfigurationUnknown(ComparisonAxis),
    /// Evidence role contradicts another role inside one lineage.
    RoleConflict(ComparisonAxis),
}

/// Truthful comparison coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonCoverage {
    /// Requested axes.
    pub requested_axes: BoundedList<ComparisonAxis, MAX_LIST_ITEMS>,
    /// Axes with at least one classified observation.
    pub executed_axes: BoundedList<ComparisonAxis, MAX_LIST_ITEMS>,
    /// Independent lineage count after proven collapse.
    pub represented_lineages: usize,
    /// Ambiguous lineage count excluded from confident independence.
    pub ambiguous_lineages: usize,
    /// Explicit omitted memberships.
    pub omitted_memberships: u64,
    /// Explicit unknown memberships.
    pub unknown_memberships: u64,
    /// Whether reference inventory scope is complete.
    pub complete_reference_scope: bool,
    /// Axes with exact local absence proof.
    pub exact_local_absence_axes: BoundedList<ComparisonAxis, MAX_LIST_ITEMS>,
    /// Explicit gaps.
    pub gaps: BoundedList<ComparisonGap, MAX_LIST_ITEMS>,
    /// Digest of exact coverage fields.
    pub coverage_digest: Blake3Digest32,
}

/// Complete comparison matrix plus coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonMatrix {
    /// Shared contract behavior categories.
    pub comparison: BehaviorComparison,
    /// Truthful coverage.
    pub coverage: ComparisonCoverage,
}

#[derive(Clone)]
struct ObservationOccurrence {
    lineage: RepositoryLineageId,
    observation: ComparableObservation,
}

/// Compares requested axes after lineage collapse and role alignment.
pub fn compare_axes(
    request: &ValidatedComparisonRequest,
    local: &LocalBehaviorEvidence,
    groups: &LineageGroupSet,
    alignments: &[EvidenceRoleAlignment],
    profile: &ComparisonProfile,
    predicate_relations: &[PredicateRelationReceipt],
    limits: ComparisonLimits,
    blake3_256: impl Fn(&[u8]) -> [u8; 32] + Copy,
) -> Result<ComparisonMatrix, CompareError> {
    let limits = limits.validate()?;
    if local.subject != request.local_subject {
        return Err(CompareError::ComparisonReportInvalid);
    }
    if groups.groups.is_empty() {
        return Err(CompareError::ComparisonScopeEmpty);
    }
    let expected_alignments = groups
        .groups
        .iter()
        .map(|group| group.representative_lineage_id)
        .collect::<BTreeSet<_>>();
    let actual_alignments = alignments
        .iter()
        .map(|alignment| alignment.lineage_id)
        .collect::<BTreeSet<_>>();
    if expected_alignments != actual_alignments {
        return Err(CompareError::ComparisonReportInvalid);
    }

    let mut local_by_axis: BTreeMap<ComparisonAxis, Vec<ComparableObservation>> = BTreeMap::new();
    for observation in &local.observations {
        if !request.request.axes.contains(&observation.observation.axis)
            || !observation.authorized
            || !observation.current
            || !observation.exact_readback_valid
        {
            return Err(CompareError::ComparableEvidenceInvalid);
        }
        local_by_axis
            .entry(observation.observation.axis)
            .or_default()
            .push(observation.clone());
    }

    let mut remote_by_axis: BTreeMap<ComparisonAxis, Vec<ObservationOccurrence>> = BTreeMap::new();
    let mut total_observations = local.observations.len();
    for group in &groups.groups {
        let mut seen_group = BTreeSet::new();
        for implementation in &group.implementations {
            for observation in &implementation.observations {
                total_observations = total_observations
                    .checked_add(1)
                    .ok_or(CompareError::ComparisonBudgetExhausted)?;
                if total_observations > limits.max_observations {
                    return Err(CompareError::ComparisonBudgetExhausted);
                }
                let key = (
                    observation.observation.axis,
                    observation.observation_digest,
                    observation.configuration_digest,
                    observation.evidence_role,
                );
                if seen_group.insert(key) {
                    remote_by_axis
                        .entry(observation.observation.axis)
                        .or_default()
                        .push(ObservationOccurrence {
                            lineage: group.representative_lineage_id,
                            observation: observation.clone(),
                        });
                }
            }
        }
    }

    let relation_map = build_relation_map(
        predicate_relations,
        request.request.source_view_digest,
    )?;
    let mut shared = Vec::new();
    let mut variants = Vec::new();
    let mut outliers = Vec::new();
    let mut locally_absent = Vec::new();
    let mut conflicts = Vec::new();
    let mut unknowns = Vec::new();
    let mut executed_axes = BTreeSet::new();
    let mut gaps = BTreeSet::new();

    if request.request.omitted_memberships > 0 {
        gaps.insert(ComparisonGap::OmittedMemberships);
    }
    if request.request.unknown_memberships > 0 {
        gaps.insert(ComparisonGap::UnknownMemberships);
    }
    if !groups.ambiguous_lineages.is_empty() {
        gaps.insert(ComparisonGap::AmbiguousLineage);
    }
    for alignment in alignments {
        for group in &alignment.groups {
            if group.internally_conflicting {
                gaps.insert(ComparisonGap::RoleConflict(group.axis));
            }
        }
    }

    for axis in request.request.axes.iter().copied() {
        let locals = local_by_axis.get(&axis).cloned().unwrap_or_default();
        let remotes = remote_by_axis.get(&axis).cloned().unwrap_or_default();
        if remotes.is_empty() {
            gaps.insert(ComparisonGap::MissingAxisEvidence(axis));
            push_unknown(
                &mut unknowns,
                axis,
                "comparison_axis_missing_reference_evidence",
                limits.max_unknowns,
            )?;
            continue;
        }
        executed_axes.insert(axis);

        let mut buckets: BTreeMap<Blake3Digest32, Vec<ObservationOccurrence>> = BTreeMap::new();
        for occurrence in remotes {
            buckets
                .entry(occurrence.observation.observation_digest)
                .or_default()
                .push(occurrence);
        }
        let remote_lineage_count = buckets
            .values()
            .flatten()
            .map(|value| value.lineage)
            .collect::<BTreeSet<_>>()
            .len();

        for (digest, occurrences) in &buckets {
            let lineages = occurrences
                .iter()
                .map(|value| value.lineage)
                .collect::<BTreeSet<_>>();
            let representative = &occurrences[0].observation;
            let matching_local = locals
                .iter()
                .find(|value| value.observation_digest == *digest);
            if matching_local.is_some() {
                shared.push(aggregate_observation(
                    representative,
                    occurrences,
                    u32::try_from(lineages.len().saturating_add(1)).unwrap_or(u32::MAX),
                    limits.max_handles_per_observation,
                )?);
                continue;
            }

            if locals.is_empty() {
                if local.exact_complete_axes.contains(&axis) {
                    locally_absent.push(aggregate_observation(
                        representative,
                        occurrences,
                        u32::try_from(lineages.len()).unwrap_or(u32::MAX),
                        limits.max_handles_per_observation,
                    )?);
                } else {
                    gaps.insert(ComparisonGap::LocalAbsenceUnproven(axis));
                    push_unknown(
                        &mut unknowns,
                        axis,
                        "comparison_local_absence_not_exactly_proven",
                        limits.max_unknowns,
                    )?;
                }
                continue;
            }

            let mut decision = BehaviorComparisonDecision::Unknown;
            let mut decisive_local = None;
            for local_observation in &locals {
                let relation = relation_for(
                    &relation_map,
                    local_observation.observation_digest,
                    representative.observation_digest,
                );
                let candidate_decision = classify_conflict(
                    local_observation,
                    representative,
                    relation,
                    profile.minimum_conflict_assurance,
                );
                if decision_rank(candidate_decision) > decision_rank(decision) {
                    decision = candidate_decision;
                    decisive_local = Some(local_observation);
                }
            }
            match decision {
                BehaviorComparisonDecision::EquivalentObservation => {
                    shared.push(aggregate_observation(
                        representative,
                        occurrences,
                        u32::try_from(lineages.len().saturating_add(1))
                            .unwrap_or(u32::MAX),
                        limits.max_handles_per_observation,
                    )?);
                }
                BehaviorComparisonDecision::ConfigurationVariant
                | BehaviorComparisonDecision::MaterialVariant => {
                    variants.push(aggregate_observation(
                        representative,
                        occurrences,
                        u32::try_from(lineages.len()).unwrap_or(u32::MAX),
                        limits.max_handles_per_observation,
                    )?);
                }
                BehaviorComparisonDecision::Conflict => {
                    let local_observation = decisive_local
                        .ok_or(CompareError::ComparisonReportInvalid)?;
                    conflicts.push(make_conflict(
                        axis,
                        local_observation,
                        representative,
                        u32::try_from(lineages.len()).unwrap_or(u32::MAX),
                        limits.max_handles_per_observation,
                    )?);
                }
                BehaviorComparisonDecision::InsufficientAssurance
                | BehaviorComparisonDecision::Unknown => {
                    gaps.insert(ComparisonGap::ConfigurationUnknown(axis));
                    push_unknown(
                        &mut unknowns,
                        axis,
                        "comparison_observation_applicability_or_assurance_unknown",
                        limits.max_unknowns,
                    )?;
                }
            }
        }

        if remote_lineage_count > 1 {
            for occurrences in buckets.values() {
                let count = occurrences
                    .iter()
                    .map(|value| value.lineage)
                    .collect::<BTreeSet<_>>()
                    .len();
                if count == 1
                    && !locals.iter().any(|value| {
                        value.observation_digest
                            == occurrences[0].observation.observation_digest
                    })
                {
                    outliers.push(aggregate_observation(
                        &occurrences[0].observation,
                        occurrences,
                        1,
                        limits.max_handles_per_observation,
                    )?);
                }
            }
        }
    }

    dedupe_behavior_observations(&mut shared);
    dedupe_behavior_observations(&mut variants);
    dedupe_behavior_observations(&mut outliers);
    dedupe_behavior_observations(&mut locally_absent);
    conflicts.sort_by(|left, right| {
        left.axis
            .cmp(&right.axis)
            .then_with(|| left.left.summary.as_str().cmp(right.left.summary.as_str()))
            .then_with(|| left.right.summary.as_str().cmp(right.right.summary.as_str()))
    });
    unknowns.sort_by(|left, right| left.unknown_ref.cmp(&right.unknown_ref));

    let comparison = BehaviorComparison {
        shared_observations: bounded(shared)?,
        variants: bounded(variants)?,
        outliers: bounded(outliers)?,
        locally_absent_observations: bounded(locally_absent)?,
        conflicts: bounded(conflicts)?,
        unknowns: bounded(unknowns)?,
    };
    let coverage = compute_comparison_coverage(
        request,
        groups,
        executed_axes,
        gaps,
        &comparison,
        blake3_256,
    )?;
    Ok(ComparisonMatrix {
        comparison,
        coverage,
    })
}

fn build_relation_map(
    receipts: &[PredicateRelationReceipt],
    context_digest: Blake3Digest32,
) -> Result<BTreeMap<(Blake3Digest32, Blake3Digest32), PredicateRelation>, CompareError> {
    let mut output = BTreeMap::new();
    for receipt in receipts {
        if !receipt.current || receipt.context_digest != context_digest {
            return Err(CompareError::ComparisonContextStale);
        }
        let key = ordered_pair(
            receipt.left_observation_digest,
            receipt.right_observation_digest,
        );
        if output.insert(key, receipt.relation).is_some() {
            return Err(CompareError::ComparisonReportInvalid);
        }
    }
    Ok(output)
}

fn relation_for(
    map: &BTreeMap<(Blake3Digest32, Blake3Digest32), PredicateRelation>,
    left: Blake3Digest32,
    right: Blake3Digest32,
) -> PredicateRelation {
    if left == right {
        PredicateRelation::Equivalent
    } else {
        map.get(&ordered_pair(left, right))
            .copied()
            .unwrap_or(PredicateRelation::Unknown)
    }
}

fn ordered_pair(
    left: Blake3Digest32,
    right: Blake3Digest32,
) -> (Blake3Digest32, Blake3Digest32) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn aggregate_observation(
    representative: &ComparableObservation,
    occurrences: &[ObservationOccurrence],
    independent_count: u32,
    max_handles: usize,
) -> Result<BehaviorObservation, CompareError> {
    let handles = dedupe_handles(
        occurrences
            .iter()
            .flat_map(|value| value.observation.observation.evidence_handles.iter().cloned())
            .chain(
                representative
                    .observation
                    .evidence_handles
                    .iter()
                    .cloned(),
            ),
        max_handles,
    )?;
    Ok(BehaviorObservation {
        axis: representative.observation.axis,
        summary: representative.observation.summary.clone(),
        evidence_handles: handles,
        configuration_predicate: representative
            .observation
            .configuration_predicate
            .clone(),
        independent_lineage_count: independent_count,
        assurance: representative.observation.assurance,
    })
}

fn make_conflict(
    axis: ComparisonAxis,
    local: &ComparableObservation,
    remote: &ComparableObservation,
    remote_lineages: u32,
    max_handles: usize,
) -> Result<BehaviorConflict, CompareError> {
    let left = aggregate_observation(local, &[], 1, max_handles)?;
    let right = aggregate_observation(remote, &[], remote_lineages, max_handles)?;
    let conflict_summary = BoundedObservation::new(format!(
        "overlapping source-backed observations differ on axis {}",
        axis.as_str()
    ))
    .map_err(|_| CompareError::ComparisonReportInvalid)?;
    Ok(BehaviorConflict {
        axis,
        left,
        right,
        conflict_summary,
        unresolved_reason_codes: BoundedSet::<SearchReasonCodeV1, MAX_REASON_CODES>::from_items([
            SearchReasonCodeV1::IncompleteCoverage,
        ])
        .map_err(|_| CompareError::ComparisonReportInvalid)?,
    })
}

fn push_unknown(
    output: &mut Vec<CoverageUnknown>,
    axis: ComparisonAxis,
    template: &str,
    maximum: usize,
) -> Result<(), CompareError> {
    if output.len() >= maximum {
        return Err(CompareError::ComparisonBudgetExhausted);
    }
    output.push(CoverageUnknown {
        unknown_ref: OpaqueId::new(format!(
            "comparison-unknown:{}:{}",
            axis.as_str(),
            output.len()
        ))
        .map_err(|_| CompareError::ComparisonReportInvalid)?,
        description_template_id: OpaqueId::new(template)
            .map_err(|_| CompareError::ComparisonReportInvalid)?,
        bounded_metadata: BoundedNonContentMetadata::empty(),
    });
    Ok(())
}

fn compute_comparison_coverage(
    request: &ValidatedComparisonRequest,
    groups: &LineageGroupSet,
    executed_axes: BTreeSet<ComparisonAxis>,
    gaps: BTreeSet<ComparisonGap>,
    comparison: &BehaviorComparison,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<ComparisonCoverage, CompareError> {
    let requested_axes = request.request.axes.iter().copied().collect::<Vec<_>>();
    let executed_axes = executed_axes.into_iter().collect::<Vec<_>>();
    let exact_local_absence_axes = comparison
        .locally_absent_observations
        .iter()
        .map(|observation| observation.axis)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let complete_reference_scope = request.request.reference_inventory_complete
        && request.request.omitted_memberships == 0
        && request.request.unknown_memberships == 0
        && groups.ambiguous_lineages.is_empty();
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/comparison-coverage/v1")?;
    for axis in &requested_axes {
        bytes.push(axis_tag(*axis));
    }
    bytes.push(0xff);
    for axis in &executed_axes {
        bytes.push(axis_tag(*axis));
    }
    bytes.extend_from_slice(
        &u64::try_from(groups.groups.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(groups.ambiguous_lineages.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&request.request.omitted_memberships.to_be_bytes());
    bytes.extend_from_slice(&request.request.unknown_memberships.to_be_bytes());
    bytes.push(u8::from(complete_reference_scope));
    for gap in &gaps {
        bytes.push(gap_tag(*gap));
    }
    Ok(ComparisonCoverage {
        requested_axes: bounded(requested_axes)?,
        executed_axes: bounded(executed_axes)?,
        represented_lineages: groups.groups.len(),
        ambiguous_lineages: groups.ambiguous_lineages.len(),
        omitted_memberships: request.request.omitted_memberships,
        unknown_memberships: request.request.unknown_memberships,
        complete_reference_scope,
        exact_local_absence_axes: bounded(exact_local_absence_axes)?,
        gaps: bounded(gaps.into_iter().collect())?,
        coverage_digest: Blake3Digest32::from_bytes(blake3_256(&bytes)),
    })
}

/// Candidate for deterministic recommended-reading ordering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingCandidate {
    /// Existing authorized source handle; the comparator never mints one.
    pub handle: SearchSourceHandle,
    /// Evidence role.
    pub role: EvidenceRole,
    /// Whether this is the local definition/contract.
    pub local: bool,
    /// Whether it directly supports a conflict or material variant.
    pub material: bool,
    /// Assurance ceiling.
    pub assurance: AssuranceClass,
    /// Portfolio priority; lower is earlier.
    pub portfolio_priority: u16,
    /// Collapsed independent lineage.
    pub lineage_id: RepositoryLineageId,
    /// Stable source identity digest.
    pub source_identity_digest: Blake3Digest32,
    /// Stable native-coordinate digest.
    pub coordinate_digest: Blake3Digest32,
    /// Current authorization permits disclosure.
    pub authorized: bool,
}

/// Orders existing handles with per-lineage and per-source diversity caps.
pub fn order_recommended_reading(
    mut candidates: Vec<ReadingCandidate>,
    limits: ComparisonLimits,
) -> Result<BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>, CompareError> {
    let limits = limits.validate()?;
    candidates.retain(|candidate| candidate.authorized);
    candidates.sort_by(|left, right| reading_key(left).cmp(&reading_key(right)));
    let mut selected = Vec::new();
    let mut lineages = BTreeMap::<RepositoryLineageId, usize>::new();
    let mut sources = BTreeMap::<Blake3Digest32, usize>::new();
    for candidate in candidates {
        if selected.len() >= limits.max_reading_handles {
            break;
        }
        if selected.contains(&candidate.handle) {
            continue;
        }
        let lineage_count = lineages.get(&candidate.lineage_id).copied().unwrap_or(0);
        let source_count = sources
            .get(&candidate.source_identity_digest)
            .copied()
            .unwrap_or(0);
        if lineage_count >= limits.max_reading_per_lineage
            || source_count >= limits.max_reading_per_source
        {
            continue;
        }
        *lineages.entry(candidate.lineage_id).or_default() += 1;
        *sources
            .entry(candidate.source_identity_digest)
            .or_default() += 1;
        selected.push(candidate.handle);
    }
    bounded(selected)
}

fn reading_key(
    value: &ReadingCandidate,
) -> (
    u8,
    Reverse<u8>,
    u8,
    u16,
    RepositoryLineageId,
    Blake3Digest32,
    Blake3Digest32,
) {
    (
        reading_class(value),
        Reverse(assurance_rank(value.assurance)),
        role_rank(value.role),
        value.portfolio_priority,
        value.lineage_id,
        value.source_identity_digest,
        value.coordinate_digest,
    )
}

fn reading_class(value: &ReadingCandidate) -> u8 {
    if value.local && value.role == EvidenceRole::Definition {
        0
    } else if value.material {
        1
    } else if value.role == EvidenceRole::Definition {
        2
    } else if matches!(value.role, EvidenceRole::Test | EvidenceRole::Caller) {
        3
    } else if value.role == EvidenceRole::Documentation {
        4
    } else {
        5
    }
}

/// Assembles the contract behavior set without adding a normative verdict.
pub fn assemble_behavior_set(
    header: RecipeResultHeader,
    local: LocalComparisonSubject,
    candidates: Vec<ComparableCandidate>,
    signatures: &BTreeMap<RepositoryLineageId, BehaviorSignature>,
    matrix: ComparisonMatrix,
    recommended_reading: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
) -> Result<CrossRepositoryBehaviorSet, CompareError> {
    let mut implementations = Vec::new();
    for mut candidate in candidates {
        let signature = signatures
            .get(&candidate.implementation.lineage_id)
            .ok_or(CompareError::ComparisonReportInvalid)?;
        candidate.implementation.behavior_signature = signature.contract_signature.clone();
        implementations.push(candidate.implementation);
    }
    implementations.sort_by(|left, right| {
        left.lineage_id
            .cmp(&right.lineage_id)
            .then_with(|| match_basis_rank(left.match_basis).cmp(&match_basis_rank(right.match_basis)))
    });
    if implementations.len() > MAX_LIST_ITEMS {
        return Err(CompareError::ComparisonBudgetExhausted);
    }
    Ok(CrossRepositoryBehaviorSet {
        header,
        local_subject: local,
        comparable_implementations: bounded(implementations)?,
        comparison: matrix.comparison,
        recommended_reading,
    })
}

/// Current fence for result revalidation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonResultFence {
    /// Current local subject resolution digest.
    pub local_subject_resolution_digest: Blake3Digest32,
    /// Current source/reference view digest.
    pub source_view_digest: Blake3Digest32,
    /// Current source-owner generation digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Current security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Current comparison profile digest.
    pub profile_digest: Blake3Digest32,
    /// Current lineage relation graph digest.
    pub lineage_graph_digest: Blake3Digest32,
    /// Current evidence manifest digest.
    pub evidence_manifest_digest: Blake3Digest32,
    /// Current portfolio revision.
    pub portfolio_revision: PortfolioRevision,
    /// Current access permits disclosure.
    pub access_permitted: bool,
    /// No purge barrier covers the result.
    pub purge_clear: bool,
    /// Observation continuity remains current.
    pub observation_current: bool,
    /// Every included handle remains valid.
    pub handles_valid: bool,
}

/// Immutable fence captured when a comparison was assembled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonResultBinding {
    /// Local subject resolution digest.
    pub local_subject_resolution_digest: Blake3Digest32,
    /// Source/reference view digest.
    pub source_view_digest: Blake3Digest32,
    /// Source-owner generation digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Comparison profile digest.
    pub profile_digest: Blake3Digest32,
    /// Lineage graph digest.
    pub lineage_graph_digest: Blake3Digest32,
    /// Evidence manifest digest.
    pub evidence_manifest_digest: Blake3Digest32,
    /// Portfolio revision.
    pub portfolio_revision: PortfolioRevision,
}

/// Comparison revalidation outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonRevalidation {
    /// Result remains current.
    Current,
    /// Restrictive access/purge state immediately invalidated disclosure.
    AccessRevoked,
    /// Local subject resolution changed.
    SubjectChanged,
    /// Portfolio or lineage graph changed.
    ReferenceScopeChanged,
    /// Evidence/profile/source view changed and comparison must be recomputed.
    Recompute,
    /// Observation continuity has a gap.
    ObservationGap,
    /// One or more reading/evidence handles are unavailable.
    HandleUnavailable,
}

/// Revalidates a comparison without patching old evidence in place.
#[must_use]
pub fn revalidate_comparison(
    binding: ComparisonResultBinding,
    current: ComparisonResultFence,
) -> ComparisonRevalidation {
    if !current.access_permitted
        || !current.purge_clear
        || current.security_fence_digest != binding.security_fence_digest
    {
        return ComparisonRevalidation::AccessRevoked;
    }
    if current.local_subject_resolution_digest != binding.local_subject_resolution_digest {
        return ComparisonRevalidation::SubjectChanged;
    }
    if current.portfolio_revision != binding.portfolio_revision
        || current.lineage_graph_digest != binding.lineage_graph_digest
    {
        return ComparisonRevalidation::ReferenceScopeChanged;
    }
    if !current.observation_current {
        return ComparisonRevalidation::ObservationGap;
    }
    if !current.handles_valid {
        return ComparisonRevalidation::HandleUnavailable;
    }
    if current.source_view_digest != binding.source_view_digest
        || current.owner_generation_digest != binding.owner_generation_digest
        || current.profile_digest != binding.profile_digest
        || current.evidence_manifest_digest != binding.evidence_manifest_digest
    {
        return ComparisonRevalidation::Recompute;
    }
    ComparisonRevalidation::Current
}

fn dedupe_handles(
    handles: impl IntoIterator<Item = SearchSourceHandle>,
    maximum: usize,
) -> Result<BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>, CompareError> {
    let mut output = Vec::new();
    for handle in handles {
        if output.contains(&handle) {
            continue;
        }
        if output.len() >= maximum {
            return Err(CompareError::ComparisonBudgetExhausted);
        }
        output.push(handle);
    }
    bounded(output)
}

fn dedupe_behavior_observations(values: &mut Vec<BehaviorObservation>) {
    values.sort_by(|left, right| {
        left.axis
            .cmp(&right.axis)
            .then_with(|| left.summary.as_str().cmp(right.summary.as_str()))
            .then_with(|| {
                left.independent_lineage_count
                    .cmp(&right.independent_lineage_count)
            })
    });
    values.dedup();
}

fn bounded<T>(values: Vec<T>) -> Result<BoundedList<T, MAX_LIST_ITEMS>, CompareError> {
    BoundedList::new(values).map_err(|_| CompareError::ComparisonBudgetExhausted)
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), CompareError> {
    let length = u64::try_from(value.len())
        .map_err(|_| CompareError::ComparisonBudgetExhausted)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > 8 * 1024 * 1024 {
        return Err(CompareError::ComparisonBudgetExhausted);
    }
    Ok(())
}

const fn match_basis_rank(value: MatchBasis) -> u8 {
    match value {
        MatchBasis::ExplicitHandle => 0,
        MatchBasis::EditorPosition => 1,
        MatchBasis::QualifiedName => 2,
        MatchBasis::Signature => 3,
        MatchBasis::Structural => 4,
        MatchBasis::ExactName => 5,
        MatchBasis::Lexical => 6,
        MatchBasis::Semantic => 7,
    }
}

const fn assurance_rank(value: AssuranceClass) -> u8 {
    match value {
        AssuranceClass::ExactBytes => 4,
        AssuranceClass::MappedText => 3,
        AssuranceClass::LossyText => 2,
        AssuranceClass::DescriptiveOnly => 1,
    }
}

const fn role_rank(value: EvidenceRole) -> u8 {
    match value {
        EvidenceRole::Definition => 0,
        EvidenceRole::Configuration => 1,
        EvidenceRole::Test => 2,
        EvidenceRole::Caller => 3,
        EvidenceRole::Reference => 4,
        EvidenceRole::Documentation => 5,
    }
}

const fn role_name(value: EvidenceRole) -> &'static str {
    match value {
        EvidenceRole::Definition => "definition",
        EvidenceRole::Reference => "reference",
        EvidenceRole::Test => "test",
        EvidenceRole::Documentation => "documentation",
        EvidenceRole::Caller => "caller",
        EvidenceRole::Configuration => "configuration",
    }
}

const fn axis_tag(value: ComparisonAxis) -> u8 {
    match value {
        ComparisonAxis::Interface => 1,
        ComparisonAxis::Validation => 2,
        ComparisonAxis::Errors => 3,
        ComparisonAxis::SideEffects => 4,
        ComparisonAxis::Tests => 5,
        ComparisonAxis::Callers => 6,
        ComparisonAxis::Documentation => 7,
    }
}

const fn role_tag(value: EvidenceRole) -> u8 {
    role_rank(value)
}

const fn gap_tag(value: ComparisonGap) -> u8 {
    match value {
        ComparisonGap::OmittedMemberships => 1,
        ComparisonGap::UnknownMemberships => 2,
        ComparisonGap::AmbiguousLineage => 3,
        ComparisonGap::MissingAxisEvidence(axis) => 10 + axis_tag(axis),
        ComparisonGap::LocalAbsenceUnproven(axis) => 30 + axis_tag(axis),
        ComparisonGap::ConfigurationUnknown(axis) => 50 + axis_tag(axis),
        ComparisonGap::RoleConflict(axis) => 70 + axis_tag(axis),
    }
}

const fn decision_rank(value: BehaviorComparisonDecision) -> u8 {
    match value {
        BehaviorComparisonDecision::Unknown => 0,
        BehaviorComparisonDecision::InsufficientAssurance => 1,
        BehaviorComparisonDecision::EquivalentObservation => 2,
        BehaviorComparisonDecision::ConfigurationVariant => 3,
        BehaviorComparisonDecision::MaterialVariant => 4,
        BehaviorComparisonDecision::Conflict => 5,
    }
}
