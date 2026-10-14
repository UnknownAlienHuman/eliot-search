//! Closed additive `retrieve_evidence@1` and `orient_scope@1` contracts (#213).
//!
//! Issue #213 freezes public evidence retrieval and scope orientation over one
//! immutable authorized scope. These two recipes are additive to the closed
//! eleven-recipe v1 registry: `RecipeIdV1`, `RecipeBodyV1`, `RecipeResultV1` and
//! every v1 wire byte are untouched, and a caller cannot smuggle heuristic
//! free-text semantics into `find_text@1` or issue raw backend queries.
//!
//! Every request names one `PublicScopeSelector` that resolves to an exact
//! authorized revision before planning. `PublicScopeSelector` values are
//! ordinary request references: they never mint a scope, a currentness
//! observation or an access decision here. Scope resolution, access, ranking
//! and exact readback remain owned by their respective packages.
//!
//! Assurance, currentness, coverage, handles, profiles and predicates reuse
//! the existing contract vocabulary. No parallel identity system, second
//! coverage type or vendor identifier is introduced.

use crate::bounds::{BoundedList, BoundedSet, MAX_LIST_ITEMS, MAX_SET_ITEMS};
use crate::ids::{
    CandidateId, CorpusId, MembershipRevision, OverlayRevision, PortfolioRevision, ProfileId,
    ReferencePortfolioId, WorkspaceViewRevisionId,
};
use crate::protocol::{ContinuationHandle, SearchSourceHandle};
use crate::query::{ExactPredicate, NativeAnchor, QueryExecutionBudget};
use crate::results::{Coverage, ResultFence};
use crate::schema::{AssuranceClass, EvidenceRole, ObservationFreshnessState};
use crate::source::{SourceRevisionRef, SourceView};
use crate::{ContractError, ContractErrorKind};

/// Maximum accepted question bytes.
pub const MAX_QUESTION_BYTES: usize = 4_096;
/// Maximum accepted bounded preview bytes of one card.
pub const MAX_PREVIEW_BYTES: usize = 1_024;
/// Maximum accepted suggested-check rationale bytes.
pub const MAX_RATIONALE_BYTES: usize = 512;

/// Declares one closed additive wire vocabulary with the same surface as the
/// shared `wire_enum!` vocabulary, without widening that macro's visibility.
macro_rules! declared_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Exact closed registry.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            /// Rejects unknown names and unversioned aliases.
            pub fn parse(value: &str) -> Result<Self, ContractError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ContractError::new(
                        ContractErrorKind::InvalidToken,
                        stringify!($name),
                    )),
                }
            }
        }
    };
}

/// Closed new-generation recipe registry, additive to `RecipeIdV1`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RecipeIdV2 {
    /// Bounded free-text evidence retrieval over one authorized scope.
    RetrieveEvidence,
    /// Bounded deterministic scope orientation over one authorized scope.
    OrientScope,
}

impl RecipeIdV2 {
    /// Exact closed set. Unknown names and unversioned aliases are rejected.
    pub const ALL: [Self; 2] = [Self::RetrieveEvidence, Self::OrientScope];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetrieveEvidence => "retrieve_evidence@1",
            Self::OrientScope => "orient_scope@1",
        }
    }

    /// Parses one exact versioned recipe identifier.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        Self::ALL
            .into_iter()
            .find(|recipe| recipe.as_str() == value)
            .ok_or_else(|| ContractError::new(ContractErrorKind::InvalidToken, "recipe_id"))
    }
}

declared_enum!(EvidenceIntent {
    Understand => "understand",
    LocateImplementation => "locate_implementation",
    FindSupportingMaterial => "find_supporting_material",
    Compare => "compare",
    TraceBehavior => "trace_behavior",
    FindTests => "find_tests",
    FindConfiguration => "find_configuration",
    ResearchQuestion => "research_question",
});

declared_enum!(OrientationDimension {
    EntryPoints => "entry_points",
    KeyDefinitions => "key_definitions",
    DependenciesAndImports => "dependencies_and_imports",
    Tests => "tests",
    Documentation => "documentation",
    Configuration => "configuration",
    ChangeHotspots => "change_hotspots",
    CitationNeighborhood => "citation_neighborhood",
    RecommendedReading => "recommended_reading",
});

declared_enum!(EvidenceRelationKind {
    Definition => "definition",
    Reference => "reference",
    Caller => "caller",
    Test => "test",
    Documentation => "documentation",
    Configuration => "configuration",
});

declared_enum!(PublicCurrentnessState {
    Current => "current",
    Stale => "stale",
    Unknown => "unknown",
    NotApplicable => "not_applicable",
});

declared_enum!(EvidenceGroupKind {
    Definitions => "definitions",
    Callers => "callers",
    Tests => "tests",
    Documentation => "documentation",
    Configuration => "configuration",
    SupportingSections => "supporting_sections",
    CitationNeighborhood => "citation_neighborhood",
    Ambiguity => "ambiguity",
});
/// One authorized immutable scope revision selected by a public request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicScopeSelector {
    /// One resolved immutable workspace view revision.
    WorkspaceRevision(WorkspaceViewRevisionId),
    /// One resolved immutable corpus revision.
    CorpusRevision {
        corpus_id: CorpusId,
        membership_revision: MembershipRevision,
    },
    /// One resolved immutable reference portfolio revision.
    PortfolioRevision {
        portfolio_id: ReferencePortfolioId,
        portfolio_revision: PortfolioRevision,
    },
    /// One resolved immutable authenticated overlay revision.
    OverlayRevision(OverlayRevision),
}

impl PublicScopeSelector {
    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

/// Exact source revision reference carried by evidence and orientation cards.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicCardSource {
    pub source_id: crate::SourceId,
    pub revision: SourceRevisionRef,
    pub representation_profile: ProfileId,
}

/// Request-side evidence hint set. Hints never grant access or force inclusion.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvidenceHints {
    pub authorized_source_ids: BoundedSet<crate::SourceId, MAX_SET_ITEMS>,
    pub suggested_exact_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
}

impl EvidenceHints {
    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

/// Request-side orientation hint set. Hints never grant access or force inclusion.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrientationHints {
    pub authorized_source_ids: BoundedSet<crate::SourceId, MAX_SET_ITEMS>,
    pub suggested_exact_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
}

impl OrientationHints {
    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

/// Shared bounded public query budgets.
///
/// The request carries the closed `QueryExecutionBudget` vocabulary unchanged.
/// Applying, subdividing and accounting for it stays with the planner and
/// executor owners; this contract only accepts a bounded well-formed value.
pub type PublicQueryBudgets = QueryExecutionBudget;

/// `retrieve_evidence@1` request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrieveEvidenceRequestV2 {
    pub scope: PublicScopeSelector,
    pub source_view: SourceView,
    pub question: crate::BoundedText<MAX_QUESTION_BYTES>,
    pub intent: EvidenceIntent,
    pub requested_roles: BoundedSet<EvidenceRole, MAX_SET_ITEMS>,
    pub optional_hints: EvidenceHints,
    pub profile_id: ProfileId,
    pub budgets: PublicQueryBudgets,
    pub progressive: bool,
}

impl RetrieveEvidenceRequestV2 {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.scope.validate()?;
        self.optional_hints.validate()?;
        validate_global_budget(&self.budgets)?;
        Ok(())
    }
}

/// `orient_scope@1` request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientScopeRequestV2 {
    pub scope: PublicScopeSelector,
    pub source_view: SourceView,
    pub question: crate::BoundedText<MAX_QUESTION_BYTES>,
    pub dimensions: BoundedSet<OrientationDimension, MAX_SET_ITEMS>,
    pub optional_hints: OrientationHints,
    pub profile_id: ProfileId,
    pub budgets: PublicQueryBudgets,
    pub progressive: bool,
}

impl OrientScopeRequestV2 {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.scope.validate()?;
        self.optional_hints.validate()?;
        validate_global_budget(&self.budgets)?;
        Ok(())
    }
}

/// One typed relation observation with its exact evidence handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRelation {
    pub kind: EvidenceRelationKind,
    pub evidence_handle: SearchSourceHandle,
}

/// One source-backed evidence card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCardV1 {
    pub card_id: CandidateId,
    pub evidence_role: EvidenceRole,
    pub source: PublicCardSource,
    pub unit: crate::UnitOccurrence,
    pub handle: SearchSourceHandle,
    pub bounded_preview: Option<crate::BoundedText<MAX_PREVIEW_BYTES>>,
    pub producer_assurance: AssuranceClass,
    pub freshness: ObservationFreshnessState,
    pub currentness: PublicCurrentnessState,
    pub lineage: Option<crate::RepositoryLineageId>,
    pub relations: BoundedList<EvidenceRelation, MAX_LIST_ITEMS>,
    pub ambiguity_group: Option<crate::Blake3Digest32>,
    pub score_profile: ProfileId,
    pub trace_ref: crate::BoundedNonContentRankingTrace,
}

impl EvidenceCardV1 {
    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

/// One grouped evidence cluster with an exact content-free digest key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGroup {
    pub group_id: crate::Blake3Digest32,
    pub kind: EvidenceGroupKind,
    pub card_ids: BoundedList<CandidateId, MAX_LIST_ITEMS>,
}

/// One bounded suggested exact check compiled from accepted exact predicates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCheckTemplate {
    pub template_id: crate::Blake3Digest32,
    pub rationale: crate::BoundedText<MAX_RATIONALE_BYTES>,
    pub predicate: ExactPredicate,
    pub native_anchor: Option<NativeAnchor>,
}

/// `retrieve_evidence@1` result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrieveEvidenceResultV2 {
    pub request_id: crate::RequestId,
    pub plan_id: crate::PlanId,
    pub plan_fingerprint: crate::PlanFingerprint,
    pub result_fence: ResultFence,
    pub resolved_scope: PublicScopeSelector,
    pub cards: BoundedList<EvidenceCardV1, MAX_LIST_ITEMS>,
    pub groups: BoundedList<EvidenceGroup, MAX_LIST_ITEMS>,
    pub suggested_exact_checks: BoundedList<ExactCheckTemplate, MAX_LIST_ITEMS>,
    pub coverage: Coverage,
    pub continuation: Option<ContinuationHandle>,
}

impl RetrieveEvidenceResultV2 {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.resolved_scope.validate()?;
        self.coverage.validate()?;
        for card in &self.cards {
            card.validate()?;
        }
        Ok(())
    }
}

/// One bounded orientation card naming exact source-backed handles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientationCard {
    pub card_id: CandidateId,
    pub dimension: OrientationDimension,
    pub source: PublicCardSource,
    pub unit: crate::UnitOccurrence,
    pub handle: SearchSourceHandle,
    pub producer_assurance: AssuranceClass,
    pub freshness: ObservationFreshnessState,
    pub currentness: PublicCurrentnessState,
    pub rank_key: crate::BoundedNonContentRankingTrace,
}

/// One grouped orientation cluster with an exact content-free digest key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientationGroup {
    pub group_id: crate::Blake3Digest32,
    pub dimension: OrientationDimension,
    pub card_ids: BoundedList<CandidateId, MAX_LIST_ITEMS>,
}

/// `orient_scope@1` result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientScopeResultV2 {
    pub request_id: crate::RequestId,
    pub plan_id: crate::PlanId,
    pub plan_fingerprint: crate::PlanFingerprint,
    pub result_fence: ResultFence,
    pub resolved_scope: PublicScopeSelector,
    pub entry_points: BoundedList<OrientationCard, MAX_LIST_ITEMS>,
    pub groups: BoundedList<OrientationGroup, MAX_LIST_ITEMS>,
    pub recommended_reading: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub suggested_exact_checks: BoundedList<ExactCheckTemplate, MAX_LIST_ITEMS>,
    pub ambiguities: BoundedList<EvidenceGroup, MAX_LIST_ITEMS>,
    pub coverage: Coverage,
    pub continuation: Option<ContinuationHandle>,
}

impl OrientScopeResultV2 {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.resolved_scope.validate()?;
        self.coverage.validate()?;
        Ok(())
    }
}

/// Additive union of the two closed new-generation results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeResultV2 {
    RetrieveEvidence(RetrieveEvidenceResultV2),
    OrientScope(OrientScopeResultV2),
}

impl RecipeResultV2 {
    #[must_use]
    pub const fn recipe_id(&self) -> RecipeIdV2 {
        match self {
            Self::RetrieveEvidence(_) => RecipeIdV2::RetrieveEvidence,
            Self::OrientScope(_) => RecipeIdV2::OrientScope,
        }
    }

    pub fn validate_for(self, expected: RecipeIdV2) -> Result<Self, ContractError> {
        if self.recipe_id() != expected {
            return Err(ContractError::new(
                ContractErrorKind::FamilyMismatch,
                "recipe_result",
            ));
        }
        match self {
            Self::RetrieveEvidence(ref result) => result.validate()?,
            Self::OrientScope(ref result) => result.validate()?,
        }
        Ok(self)
    }
}

fn validate_global_budget(budgets: &PublicQueryBudgets) -> Result<(), ContractError> {
    if budgets.deadline_ms == 0
        || budgets.max_scoring_legs == 0
        || budgets.max_prefetch_candidates_per_leg == 0
        || budgets.max_validated_candidates == 0
        || budgets.max_source_read_bytes == 0
        || budgets.max_exact_scan_items == 0
        || budgets.max_exact_scan_bytes == 0
        || budgets.max_materialized_result_bytes == 0
        || budgets.max_cpu_ms == 0
        || budgets.max_memory_bytes == 0
    {
        return Err(ContractError::new(ContractErrorKind::InvalidRange, "public_query_budgets"));
    }
    Ok(())
}
