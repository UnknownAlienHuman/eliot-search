use crate::bounds::{BoundedList, BoundedSet, MAX_LIST_ITEMS, MAX_SET_ITEMS};
use crate::canonical::{BoundedDisplayPath, BoundedName, BoundedSymbolKey};
use crate::ids::{
    BufferSnapshotId, CorpusId, ExactScanPlanRef, PlanId, PortfolioRevision, ProfileId,
    ReferencePortfolioId, RequestId, SourceMembershipId, WorkspaceId,
};
use crate::protocol::{ContinuationHandle, SearchSourceHandle};
use crate::query::{ExactCompletenessRequirements, ExactPredicate, NativeAnchor};
use crate::schema::{EntityKind, EvidenceRole};
use crate::source::SourceView;
use crate::{ContractError, ContractErrorKind};

/// Closed provider-wire registry with exactly the eleven P00 recipe values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RecipeIdV1 {
    Locate,
    FindText,
    InspectEntity,
    CompareImplementations,
    ExploreEntity,
    CorpusProfile,
    CorpusDelta,
    Provenance,
    CompileExactScan,
    ExecuteExactScan,
    ExpandHandle,
}

impl RecipeIdV1 {
    pub const ALL: [Self; 11] = [
        Self::Locate,
        Self::FindText,
        Self::InspectEntity,
        Self::CompareImplementations,
        Self::ExploreEntity,
        Self::CorpusProfile,
        Self::CorpusDelta,
        Self::Provenance,
        Self::CompileExactScan,
        Self::ExecuteExactScan,
        Self::ExpandHandle,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Locate => "locate@1",
            Self::FindText => "find_text@1",
            Self::InspectEntity => "inspect_entity@1",
            Self::CompareImplementations => "compare_implementations@1",
            Self::ExploreEntity => "explore_entity@1",
            Self::CorpusProfile => "corpus_profile@1",
            Self::CorpusDelta => "corpus_delta@1",
            Self::Provenance => "provenance@1",
            Self::CompileExactScan => "compile_exact_scan@1",
            Self::ExecuteExactScan => "execute_exact_scan@1",
            Self::ExpandHandle => "expand_handle@1",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        Self::ALL
            .into_iter()
            .find(|recipe| recipe.as_str() == value)
            .ok_or_else(|| ContractError::new(ContractErrorKind::InvalidCharacter, "recipe_id"))
    }

    /// Parse an exact versioned recipe identifier. Unversioned aliases are rejected.
    pub fn parse_versioned(value: &str) -> Result<Self, ContractError> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ComparisonAxis {
    Interface,
    Validation,
    Errors,
    SideEffects,
    Tests,
    Callers,
    Documentation,
}

impl ComparisonAxis {
    pub const ALL: [Self; 7] = [
        Self::Interface,
        Self::Validation,
        Self::Errors,
        Self::SideEffects,
        Self::Tests,
        Self::Callers,
        Self::Documentation,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::Validation => "validation",
            Self::Errors => "errors",
            Self::SideEffects => "side_effects",
            Self::Tests => "tests",
            Self::Callers => "callers",
            Self::Documentation => "documentation",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        Self::ALL
            .into_iter()
            .find(|axis| axis.as_str() == value)
            .ok_or_else(|| {
                ContractError::new(ContractErrorKind::InvalidCharacter, "comparison_axis")
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencePortfolioScope {
    pub portfolio_id: ReferencePortfolioId,
    pub portfolio_revision: PortfolioRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestedScope {
    ActiveWorkspace(WorkspaceId),
    ExplicitMemberships(BoundedList<SourceMembershipId, MAX_LIST_ITEMS>),
    Corpus(CorpusId),
    ReferencePortfolio(ReferencePortfolioScope),
    SourceHandle(SearchSourceHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPositionSelector {
    pub workspace_id: WorkspaceId,
    pub buffer_snapshot_id: Option<BufferSnapshotId>,
    pub anchor: NativeAnchor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedSymbolSelector {
    pub normalized_symbol_key: BoundedSymbolKey,
    pub entity_kinds: BoundedSet<EntityKind, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedNameSelector {
    pub name: BoundedName,
    pub entity_kinds: BoundedSet<EntityKind, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathSelector {
    pub workspace_id: WorkspaceId,
    pub display_path: BoundedDisplayPath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubjectSelector {
    SourceHandle(SearchSourceHandle),
    EditorPosition(EditorPositionSelector),
    QualifiedSymbol(QualifiedSymbolSelector),
    NormalizedName(NormalizedNameSelector),
    Path(PathSelector),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelationKind {
    Definition,
    Reference,
    Caller,
    Test,
    Documentation,
    Configuration,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CasePolicy {
    Exact,
    UnicodeCasefold,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CorpusFacetDimension {
    Role,
    LanguageOrFormat,
    EntityKind,
    Lineage,
    Readiness,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CorpusDeltaDimension {
    Source,
    Membership,
    Representation,
    Symbol,
    Readiness,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HandleExpansionKind {
    Excerpt,
    SourceMetadata,
    Provenance,
    Continuation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocateRecipe {
    pub subject: SubjectSelector,
    pub evidence_roles: BoundedSet<EvidenceRole, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindTextRecipe {
    pub predicate: ExactPredicate,
    pub case_policy: CasePolicy,
    pub context_bytes_before: u32,
    pub context_bytes_after: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectEntityRecipe {
    pub subject: SubjectSelector,
    pub evidence_roles: BoundedSet<EvidenceRole, MAX_SET_ITEMS>,
    pub include_relations: BoundedSet<RelationKind, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareImplementationsRecipe {
    pub subject: SubjectSelector,
    pub references: ReferencePortfolioScope,
    pub comparison_axes: BoundedSet<ComparisonAxis, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreEntityRecipe {
    pub subject: SubjectSelector,
    pub relation_kinds: BoundedSet<RelationKind, MAX_SET_ITEMS>,
    pub max_depth: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusProfileRecipe {
    pub facets: BoundedSet<CorpusFacetDimension, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusDeltaRecipe {
    pub from_view: SourceView,
    pub to_view: SourceView,
    pub dimensions: BoundedSet<CorpusDeltaDimension, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceRecipe {
    pub source_handle: SearchSourceHandle,
    pub max_lineage_depth: u8,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompileExactScanRecipe {
    pub completeness_requirements: ExactCompletenessRequirements,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExecuteExactScanRecipe {
    pub plan_ref: ExactScanPlanRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpandHandleTarget {
    Source(SearchSourceHandle),
    Continuation(ContinuationHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandHandleRecipe {
    pub handle: ExpandHandleTarget,
    pub expansion: HandleExpansionKind,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeBodyV1 {
    Locate(LocateRecipe),
    FindText(FindTextRecipe),
    InspectEntity(InspectEntityRecipe),
    CompareImplementations(CompareImplementationsRecipe),
    ExploreEntity(ExploreEntityRecipe),
    CorpusProfile(CorpusProfileRecipe),
    CorpusDelta(CorpusDeltaRecipe),
    Provenance(ProvenanceRecipe),
    CompileExactScan {
        predicate: ExactPredicate,
        body: CompileExactScanRecipe,
    },
    ExecuteExactScan(ExecuteExactScanRecipe),
    ExpandHandle(ExpandHandleRecipe),
}

impl RecipeBodyV1 {
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
            Self::CompileExactScan { .. } => RecipeIdV1::CompileExactScan,
            Self::ExecuteExactScan(_) => RecipeIdV1::ExecuteExactScan,
            Self::ExpandHandle(_) => RecipeIdV1::ExpandHandle,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRecipeRequest {
    pub request_id: RequestId,
    pub recipe: RecipeIdV1,
    pub source_view: SourceView,
    pub requested_scope: RequestedScope,
    pub requested_budget_class: ProfileId,
    pub body: RecipeBodyV1,
}

impl SearchRecipeRequest {
    pub fn new(
        request_id: RequestId,
        recipe: RecipeIdV1,
        source_view: SourceView,
        requested_scope: RequestedScope,
        requested_budget_class: ProfileId,
        body: RecipeBodyV1,
    ) -> Result<Self, ContractError> {
        if body.recipe_id() != recipe {
            return Err(ContractError::new(
                ContractErrorKind::FamilyMismatch,
                "recipe_body",
            ));
        }
        Ok(Self {
            request_id,
            recipe,
            source_view,
            requested_scope,
            requested_budget_class,
            body,
        })
    }
}

// Keep `PlanId` linked at the schema boundary; compile/execute records use the
// stronger `ExactScanPlanRef` rather than an open plan string.
crate::impl_wire_enum!(RelationKind {
    Definition => "definition",
    Reference => "reference",
    Caller => "caller",
    Test => "test",
    Documentation => "documentation",
    Configuration => "configuration",
});
crate::impl_wire_enum!(CasePolicy {
    Exact => "exact",
    UnicodeCasefold => "unicode_casefold",
});
crate::impl_wire_enum!(CorpusFacetDimension {
    Role => "role",
    LanguageOrFormat => "language_or_format",
    EntityKind => "entity_kind",
    Lineage => "lineage",
    Readiness => "readiness",
});
crate::impl_wire_enum!(CorpusDeltaDimension {
    Source => "source",
    Membership => "membership",
    Representation => "representation",
    Symbol => "symbol",
    Readiness => "readiness",
});
crate::impl_wire_enum!(HandleExpansionKind {
    Excerpt => "excerpt",
    SourceMetadata => "source_metadata",
    Provenance => "provenance",
    Continuation => "continuation",
});

const _: Option<PlanId> = None;
