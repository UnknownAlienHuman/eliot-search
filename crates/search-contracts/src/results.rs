use crate::bounds::{
    BoundedList, BoundedSet, BoundedTextOrBytes, MAX_LIST_ITEMS, MAX_REASON_CODES, MAX_SET_ITEMS,
};
use crate::canonical::{
    BoundedBehaviorSignature, BoundedDisplayPath, BoundedExpression, BoundedName,
    BoundedNonContentMetadata, BoundedNonContentRankingTrace, BoundedObservation,
    OpaqueAuthorizedFacetValue, OpaqueId, OpaqueRef,
};
use crate::ids::{
    Blake3Digest32, CandidateId, PlanFingerprint, PlanId, ProfileId, ReceiptRef,
    RepositoryLineageId, RequestId, SourceId, SourceMembershipId,
};
use crate::lifecycle::MembershipReadiness;
use crate::protocol::{ContinuationHandle, SearchSourceHandle};
use crate::query::{
    BehaviorConflict, BehaviorObservation, ConfigurationObservation, CoverageDenominatorKind,
    CoverageGap, CoverageUnknown, EmissionSecurityFence, ExactExecutionReport, ExactScanPlan,
    LegDescriptor, LegExecutionSummary, NativeAnchor, ObservationFreshness, QuerySnapshotFence,
    SourceOwnerFence,
};
use crate::reasons::SearchReasonCodeV1;
use crate::recipes::{
    ComparisonAxis, CorpusFacetDimension, ExpandHandleTarget, RecipeIdV1, RelationKind,
};
use crate::schema::{
    AssuranceClass, EntityKind, EvidenceRole, Modality, ObservationFreshnessState,
};
use crate::source::{SourceRevisionRef, SourceView};
use crate::{AuthorizedScopeRef, ContractError, ContractErrorKind};

/// Emission-time fence. The original planning snapshot is preserved unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultFence {
    pub planned_snapshot: QuerySnapshotFence,
    pub emission_source_owner_fences: BoundedList<SourceOwnerFence, MAX_LIST_ITEMS>,
    pub emission_security_fence: EmissionSecurityFence,
    pub result_fingerprint: Blake3Digest32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CandidateValidationGapReason {
    Stale,
    Unreadable,
    AccessRevoked,
    Purged,
    SourceRevisionUnavailable,
}

crate::impl_wire_enum!(CandidateValidationGapReason {
    Stale => "stale",
    Unreadable => "unreadable",
    AccessRevoked => "access_revoked",
    Purged => "purged",
    SourceRevisionUnavailable => "source_revision_unavailable",
});

impl CandidateValidationGapReason {
    #[must_use]
    pub const fn public_reason(self) -> SearchReasonCodeV1 {
        match self {
            Self::Stale => SearchReasonCodeV1::Stale,
            Self::Unreadable => SearchReasonCodeV1::Unreadable,
            Self::AccessRevoked => SearchReasonCodeV1::AccessRevoked,
            Self::Purged => SearchReasonCodeV1::Purged,
            Self::SourceRevisionUnavailable => SearchReasonCodeV1::SourceRevisionUnavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CandidateGapDisposition {
    Dropped,
    ReplanRequested,
    GapReported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateValidationGap {
    pub nominated_candidate_ref: OpaqueId,
    pub source_revision_ref: Option<SourceRevisionRef>,
    pub reason: CandidateValidationGapReason,
    pub affected_leg_refs: BoundedList<OpaqueId, MAX_LIST_ITEMS>,
    pub contaminated_rank_leg: bool,
    pub disposition: CandidateGapDisposition,
}

impl CandidateValidationGap {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.affected_leg_refs.is_empty()
            || (self.contaminated_rank_leg
                && self.disposition != CandidateGapDisposition::ReplanRequested)
        {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "candidate_validation_gap",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    pub requested_legs: BoundedList<LegDescriptor, MAX_LIST_ITEMS>,
    pub executed_legs: BoundedList<LegExecutionSummary, MAX_LIST_ITEMS>,
    pub represented_memberships: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    pub represented_source_lineages: BoundedSet<RepositoryLineageId, MAX_SET_ITEMS>,
    pub omitted_or_failed_legs: BoundedList<CoverageGap, MAX_LIST_ITEMS>,
    pub candidate_validation_gaps: BoundedList<CandidateValidationGap, MAX_LIST_ITEMS>,
    pub observation_freshness: ObservationFreshness,
    pub unknowns: BoundedList<CoverageUnknown, MAX_LIST_ITEMS>,
    pub denominator_kind: CoverageDenominatorKind,
}

impl Coverage {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.observation_freshness.validate()?;
        for gap in &self.candidate_validation_gaps {
            gap.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSearchCandidate {
    pub candidate_id: CandidateId,
    pub source_handle: SearchSourceHandle,
    pub evidence_role: EvidenceRole,
    pub entity_kind: Option<EntityKind>,
    pub assurance: AssuranceClass,
    pub freshness: ObservationFreshnessState,
    pub ranking_trace: BoundedNonContentRankingTrace,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    pub candidate_validation_receipt_ref: ReceiptRef,
}

impl ValidatedSearchCandidate {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self
            .reason_codes
            .iter()
            .any(|reason| reason.is_candidate_forbidden())
        {
            return Err(ContractError::new(
                ContractErrorKind::ForbiddenCandidateReason,
                "validated_candidate.reason_codes",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCandidateSet {
    pub request_id: RequestId,
    pub plan_id: PlanId,
    pub plan_fingerprint: PlanFingerprint,
    pub result_fence: ResultFence,
    pub candidates: BoundedList<ValidatedSearchCandidate, MAX_LIST_ITEMS>,
    pub coverage: Coverage,
    pub continuation_handle: Option<ContinuationHandle>,
    pub result_validation_receipt_ref: ReceiptRef,
}

impl SearchCandidateSet {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.coverage.validate()?;
        for candidate in &self.candidates {
            candidate.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeResultHeader {
    pub request_id: RequestId,
    pub plan_id: PlanId,
    pub plan_fingerprint: PlanFingerprint,
    pub result_fence: ResultFence,
    pub coverage: Coverage,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatchBasis {
    ExplicitHandle,
    EditorPosition,
    QualifiedName,
    ExactName,
    Signature,
    Structural,
    Lexical,
    Semantic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSubject {
    pub canonical_handle: SearchSourceHandle,
    pub match_basis: MatchBasis,
    pub entity_kind: EntityKind,
    pub normalized_name: BoundedName,
    pub signature_observation: Option<BoundedObservation>,
    pub configuration_predicate: Option<BoundedExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousSubjectCandidate {
    pub source_handle: SearchSourceHandle,
    pub entity_kind: EntityKind,
    pub match_basis: MatchBasis,
    pub disambiguation_summary: BoundedNonContentMetadata,
}

impl AmbiguousSubjectCandidate {
    pub fn validate(&self) -> Result<(), ContractError> {
        if matches!(
            self.match_basis,
            MatchBasis::ExplicitHandle | MatchBasis::EditorPosition
        ) {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "ambiguous_subject.match_basis",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectAmbiguitySet {
    pub requested_selector_digest: Blake3Digest32,
    pub candidates: BoundedList<AmbiguousSubjectCandidate, MAX_LIST_ITEMS>,
    pub reason_code: SearchReasonCodeV1,
}

impl SubjectAmbiguitySet {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.reason_code != SearchReasonCodeV1::AmbiguousSubject || self.candidates.is_empty() {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "subject_ambiguity",
            ));
        }
        for candidate in &self.candidates {
            candidate.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectAmbiguityResult {
    pub header: RecipeResultHeader,
    pub ambiguity: SubjectAmbiguitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceObservation {
    pub role: EvidenceRole,
    pub source_handle: SearchSourceHandle,
    pub observation: BoundedObservation,
    pub assurance: AssuranceClass,
    pub configuration_predicate: Option<BoundedExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedEntityInspection {
    pub header: RecipeResultHeader,
    pub subject: ResolvedSubject,
    pub definitions: BoundedList<EvidenceObservation, MAX_LIST_ITEMS>,
    pub references: BoundedList<EvidenceObservation, MAX_LIST_ITEMS>,
    pub callers: BoundedList<EvidenceObservation, MAX_LIST_ITEMS>,
    pub tests: BoundedList<EvidenceObservation, MAX_LIST_ITEMS>,
    pub documentation: BoundedList<EvidenceObservation, MAX_LIST_ITEMS>,
    pub configuration_variants: BoundedList<ConfigurationObservation, MAX_LIST_ITEMS>,
    pub continuation_handle: Option<ContinuationHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityInspectionResult {
    Resolved(ResolvedEntityInspection),
    Ambiguous(SubjectAmbiguityResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityGraphNode {
    pub node_id: OpaqueId,
    pub source_handle: SearchSourceHandle,
    pub entity_kind: EntityKind,
    pub normalized_name: BoundedName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityGraphEdge {
    pub from_node_id: OpaqueId,
    pub to_node_id: OpaqueId,
    pub relation: RelationKind,
    pub assurance: AssuranceClass,
    pub evidence_handle: SearchSourceHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedEntityExploration {
    pub header: RecipeResultHeader,
    pub root_subject: ResolvedSubject,
    pub nodes: BoundedList<EntityGraphNode, MAX_LIST_ITEMS>,
    pub edges: BoundedList<EntityGraphEdge, MAX_LIST_ITEMS>,
    pub truncated_at_depth: Option<u8>,
    pub continuation_handle: Option<ContinuationHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityExplorationResult {
    Resolved(ResolvedEntityExploration),
    Ambiguous(SubjectAmbiguityResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalComparisonSubject {
    pub resolved_subject: ResolvedSubject,
    pub definition: SearchSourceHandle,
    pub signature: Option<BoundedObservation>,
    pub callers: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub tests: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub documentation: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparableImplementation {
    pub lineage_id: RepositoryLineageId,
    pub match_basis: MatchBasis,
    pub configuration_predicate: Option<BoundedExpression>,
    pub evidence_roles: BoundedSet<EvidenceRole, MAX_SET_ITEMS>,
    pub behavior_signature: BoundedBehaviorSignature,
    pub exact_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorComparison {
    pub shared_observations: BoundedList<BehaviorObservation, MAX_LIST_ITEMS>,
    pub variants: BoundedList<BehaviorObservation, MAX_LIST_ITEMS>,
    pub outliers: BoundedList<BehaviorObservation, MAX_LIST_ITEMS>,
    pub locally_absent_observations: BoundedList<BehaviorObservation, MAX_LIST_ITEMS>,
    pub conflicts: BoundedList<BehaviorConflict, MAX_LIST_ITEMS>,
    pub unknowns: BoundedList<CoverageUnknown, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossRepositoryBehaviorSet {
    pub header: RecipeResultHeader,
    pub local_subject: LocalComparisonSubject,
    pub comparable_implementations: BoundedList<ComparableImplementation, MAX_LIST_ITEMS>,
    pub comparison: BehaviorComparison,
    pub recommended_reading: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompareImplementationsResult {
    Compared(CrossRepositoryBehaviorSet),
    Ambiguous(SubjectAmbiguityResult),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CountAssurance {
    ExactInventory,
    FilteredIndex,
    Partial,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusFacet {
    pub dimension: CorpusFacetDimension,
    pub value: OpaqueAuthorizedFacetValue,
    pub count: u64,
    pub count_assurance: CountAssurance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusProfileResult {
    pub header: RecipeResultHeader,
    pub scope: AuthorizedScopeRef,
    pub facets: BoundedList<CorpusFacet, MAX_LIST_ITEMS>,
    pub readiness: BoundedList<MembershipReadiness, MAX_LIST_ITEMS>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CorpusChangeKind {
    SourceAdded,
    SourceRemoved,
    RevisionChanged,
    MembershipChanged,
    RepresentationChanged,
    SymbolChanged,
    ReadinessChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusChange {
    pub kind: CorpusChangeKind,
    pub source_id: Option<SourceId>,
    pub source_membership_id: Option<SourceMembershipId>,
    pub before_ref: Option<OpaqueRef>,
    pub after_ref: Option<OpaqueRef>,
    pub evidence_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub assurance: AssuranceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusDeltaResult {
    pub header: RecipeResultHeader,
    pub from_view: SourceView,
    pub to_view: SourceView,
    pub changes: BoundedList<CorpusChange, MAX_LIST_ITEMS>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProvenanceStepKind {
    SourceIdentity,
    RevisionOccurrence,
    Materialization,
    Representation,
    Unit,
    Projection,
    Export,
    OwnershipCutover,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceStep {
    pub sequence: u32,
    pub kind: ProvenanceStepKind,
    pub input_refs: BoundedList<OpaqueRef, MAX_LIST_ITEMS>,
    pub output_ref: OpaqueRef,
    pub profile_or_protocol_id: Option<ProfileId>,
    pub receipt_ref: ReceiptRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceResult {
    pub header: RecipeResultHeader,
    pub subject_handle: SearchSourceHandle,
    pub chain: BoundedList<ProvenanceStep, MAX_LIST_ITEMS>,
    pub unresolved_steps: BoundedList<CoverageGap, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExcerptExpansion {
    pub source_revision_ref: SourceRevisionRef,
    pub native_anchor: NativeAnchor,
    pub content: BoundedTextOrBytes,
    pub content_digest: Blake3Digest32,
    pub assurance: AssuranceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMetadataExpansion {
    pub source_revision_ref: SourceRevisionRef,
    pub authorized_display_path: Option<BoundedDisplayPath>,
    pub modality: Modality,
    pub language_or_format: ProfileId,
    pub provenance_ref: OpaqueRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationExpansion {
    pub candidates: BoundedList<ValidatedSearchCandidate, MAX_LIST_ITEMS>,
    pub coverage_delta: Coverage,
    pub next_continuation_handle: Option<ContinuationHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandleExpansionBody {
    Excerpt(ExcerptExpansion),
    SourceMetadata(SourceMetadataExpansion),
    Provenance(Box<ProvenanceResult>),
    Continuation(ContinuationExpansion),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleExpansionResult {
    pub header: RecipeResultHeader,
    pub handle: ExpandHandleTarget,
    pub authorization_receipt_ref: ReceiptRef,
    pub body: HandleExpansionBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeResultV1 {
    Locate(SearchCandidateSet),
    FindText(SearchCandidateSet),
    InspectEntity(EntityInspectionResult),
    CompareImplementations(CompareImplementationsResult),
    ExploreEntity(EntityExplorationResult),
    CorpusProfile(CorpusProfileResult),
    CorpusDelta(CorpusDeltaResult),
    Provenance(ProvenanceResult),
    CompileExactScan(ExactScanPlan),
    ExecuteExactScan(ExactExecutionReport),
    ExpandHandle(HandleExpansionResult),
}

impl RecipeResultV1 {
    #[must_use]
    pub const fn recipe_id(&self) -> RecipeIdV1 {
        match self {
            Self::Locate(_) => RecipeIdV1::Locate,
            Self::FindText(_) => RecipeIdV1::FindText,
            Self::InspectEntity(_) => RecipeIdV1::InspectEntity,
            Self::CompareImplementations(_) => RecipeIdV1::CompareImplementations,
            Self::ExploreEntity(_) => RecipeIdV1::ExploreEntity,
            Self::CorpusProfile(_) => RecipeIdV1::CorpusProfile,
            Self::CorpusDelta(_) => RecipeIdV1::CorpusDelta,
            Self::Provenance(_) => RecipeIdV1::Provenance,
            Self::CompileExactScan(_) => RecipeIdV1::CompileExactScan,
            Self::ExecuteExactScan(_) => RecipeIdV1::ExecuteExactScan,
            Self::ExpandHandle(_) => RecipeIdV1::ExpandHandle,
        }
    }

    pub fn validate_for(self, expected: RecipeIdV1) -> Result<Self, ContractError> {
        if self.recipe_id() != expected {
            return Err(ContractError::new(
                ContractErrorKind::FamilyMismatch,
                "recipe_result",
            ));
        }
        match &self {
            Self::Locate(result) | Self::FindText(result) => result.validate()?,
            Self::ExecuteExactScan(report) => report.validate()?,
            Self::InspectEntity(EntityInspectionResult::Ambiguous(result))
            | Self::ExploreEntity(EntityExplorationResult::Ambiguous(result))
            | Self::CompareImplementations(CompareImplementationsResult::Ambiguous(result)) => {
                result.ambiguity.validate()?;
            }
            _ => {}
        }
        Ok(self)
    }
}

crate::impl_wire_enum!(CandidateGapDisposition {
    Dropped => "dropped",
    ReplanRequested => "replan_requested",
    GapReported => "gap_reported",
});
crate::impl_wire_enum!(MatchBasis {
    ExplicitHandle => "explicit_handle",
    EditorPosition => "editor_position",
    QualifiedName => "qualified_name",
    ExactName => "exact_name",
    Signature => "signature",
    Structural => "structural",
    Lexical => "lexical",
    Semantic => "semantic",
});
crate::impl_wire_enum!(CountAssurance {
    ExactInventory => "exact_inventory",
    FilteredIndex => "filtered_index",
    Partial => "partial",
});
crate::impl_wire_enum!(CorpusChangeKind {
    SourceAdded => "source_added",
    SourceRemoved => "source_removed",
    RevisionChanged => "revision_changed",
    MembershipChanged => "membership_changed",
    RepresentationChanged => "representation_changed",
    SymbolChanged => "symbol_changed",
    ReadinessChanged => "readiness_changed",
});
crate::impl_wire_enum!(ProvenanceStepKind {
    SourceIdentity => "source_identity",
    RevisionOccurrence => "revision_occurrence",
    Materialization => "materialization",
    Representation => "representation",
    Unit => "unit",
    Projection => "projection",
    Export => "export",
    OwnershipCutover => "ownership_cutover",
});

const _: Option<ComparisonAxis> = None;
