use crate::bounds::{
    BoundedBytes, BoundedCanonicalBytes, BoundedList, BoundedMap, BoundedSet, MAX_LIST_ITEMS,
    MAX_MAP_ENTRIES, MAX_RAW_BYTES, MAX_REASON_CODES, MAX_SET_ITEMS,
};
use crate::canonical::{
    BoundedExpression, BoundedNonContentMetadata, BoundedObservation, CanonicalBytes, CanonicalKey,
    CanonicalText, CanonicalValue, OpaqueId, OpaqueRef, UtcTimestamp, domain_separated_preimage,
};
use crate::ids::{
    AccessPartitionId, AccessPolicyBindingId, AccessPolicyRevision, BindingId, Blake3Digest32,
    BufferSnapshotId, CatalogRevision, CollectionGenerationId, CollectionRouteRevision, CorpusId,
    CorpusOrPortfolioId, FusionProfileId, GitObjectId, GrantId, InstallationId,
    InstallationIncarnationId, MembershipRevision, ObservationCursorRevision, OverlayRevision,
    PlanFingerprint, PlanId, PortfolioRevision, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, PurgeFenceRevision, QuerySnapshotFingerprint, ReceiptRef,
    RepositoryLineageId, RequestId, ScopeDomainId, ScoringPartitionId, ShadowFenceRevision,
    SourceId, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration, SourceRevisionId,
    WorkspaceViewRevisionId,
};
use crate::protocol::{ContinuationHandle, ProtocolVersion, SearchSourceHandle};
use crate::reasons::SearchReasonCodeV1;
use crate::recipes::RecipeIdV1;
use crate::schema::{
    DisclosureCeiling, EntityKind, EvidenceRole, Modality, ObservationFreshnessState,
    SensitivityClass,
};
use crate::source::{SourceRevisionRef, SourceView};
use crate::{AuthorizedScopeRef, ContractError, ContractErrorKind, ExactScanPlanRef};
use core::{cmp::Ordering, hash::Hash};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegKind {
    Direct,
    Exact,
    Structural,
    Lexical,
    Semantic,
    Rerank,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegDescriptor {
    pub leg_ref: OpaqueId,
    pub leg_kind: LegKind,
    pub scoring_partition_ref: Option<OpaqueRef>,
    pub profile_id: ProfileId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoverageGapKind {
    UnavailableMembership,
    FailedLeg,
    OmittedBudget,
    ObservationGap,
    SourceUnreadable,
    ValidationGap,
    AccessRevoked,
    Purge,
    ProviderDegraded,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Retryability {
    Never,
    SameRequest,
    AfterRefresh,
    AfterReconcile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGap {
    pub gap_ref: OpaqueId,
    pub kind: CoverageGapKind,
    pub affected_scope_refs: BoundedList<OpaqueRef, MAX_LIST_ITEMS>,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    pub retryability: Retryability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageUnknown {
    pub unknown_ref: OpaqueId,
    pub description_template_id: OpaqueId,
    pub bounded_metadata: BoundedNonContentMetadata,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObservationFreshness {
    pub state: ObservationFreshnessState,
    pub observation_cursor_revision: ObservationCursorRevision,
    pub observed_age_ms: Option<u64>,
}

impl ObservationFreshness {
    pub fn validate(self) -> Result<(), ContractError> {
        match (self.state, self.observed_age_ms) {
            (ObservationFreshnessState::ObservedWithAge, Some(_))
            | (
                ObservationFreshnessState::CurrentConfirmed
                | ObservationFreshnessState::GapDetected
                | ObservationFreshnessState::Unknown,
                None,
            ) => Ok(()),
            _ => Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "observation_freshness",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchReadGrantClaims {
    pub grant_id: GrantId,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub binding_id: BindingId,
    pub principal_opaque_id: OpaqueId,
    pub client_scope_ref: OpaqueRef,
    pub scope_domain_id: ScopeDomainId,
    pub allowed_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    pub allowed_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    pub reference_portfolio_revision: Option<PortfolioRevision>,
    pub allowed_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    pub allowed_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    pub permitted_recipe_families: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    pub maximum_budget_class: ProfileId,
    pub sensitivity_ceiling: SensitivityClass,
    pub disclosure_ceiling: DisclosureCeiling,
    pub source_read_permission: bool,
    pub exact_scan_permission: bool,
    pub issued_boot_id: OpaqueId,
    pub issued_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub nonce: OpaqueId,
    pub revocation_generation: u64,
}

impl SearchReadGrantClaims {
    /// Validate only the closed grant shape; authorization remains a live server decision.
    pub fn validate_shape(&self) -> Result<(), ContractError> {
        self.validate()
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.expires_at <= self.issued_at {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "grant.expires_at",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PriorityClass {
    Interactive,
    Verification,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct QueryExecutionBudget {
    pub priority_class: PriorityClass,
    pub deadline_ms: u64,
    pub max_scoring_legs: u32,
    pub max_prefetch_candidates_per_leg: u32,
    pub max_validated_candidates: u32,
    pub max_source_read_bytes: u64,
    pub max_exact_scan_items: u64,
    pub max_exact_scan_bytes: u64,
    pub max_materialized_result_bytes: u64,
    pub max_cpu_ms: u64,
    pub max_memory_bytes: u64,
}

/// Planning-time snapshot. It is intentionally not interchangeable with
/// `ResultFence`, which records the later emission authorization state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySnapshotFence {
    pub installation_incarnation_id: InstallationIncarnationId,
    pub collection_generation_id: Option<CollectionGenerationId>,
    pub visible_epoch: Option<crate::Epoch>,
    pub collection_route_revision: CollectionRouteRevision,
    pub catalog_revision: CatalogRevision,
    pub membership_revision: MembershipRevision,
    pub reference_portfolio_revision: Option<PortfolioRevision>,
    pub access_policy_revision: AccessPolicyRevision,
    pub shadow_fence_revision: ShadowFenceRevision,
    pub purge_fence_revision: PurgeFenceRevision,
    pub overlay_revision: OverlayRevision,
    pub observation_cursor_revision: ObservationCursorRevision,
    pub observation_freshness: ObservationFreshness,
    pub source_view: SourceView,
    pub workspace_view_revision_ref: Option<WorkspaceViewRevisionId>,
    pub lexical_profile_ids: BoundedList<ProfileId, MAX_LIST_ITEMS>,
    pub snapshot_fingerprint: QuerySnapshotFingerprint,
}

impl QuerySnapshotFence {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.observation_freshness.validate()?;
        self.source_view.validate()?;
        if self.collection_generation_id.is_some() != self.visible_epoch.is_some() {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "query_snapshot.index_pair",
            ));
        }
        if !self.lexical_profile_ids.is_empty() && self.collection_generation_id.is_none() {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "query_snapshot.lexical_profiles",
            ));
        }
        match &self.source_view {
            SourceView::WorkingTreeCurrent(view) | SourceView::GitIndex(view) => {
                if self.workspace_view_revision_ref != Some(view.workspace_view_revision_ref) {
                    return Err(ContractError::new(
                        ContractErrorKind::ContradictoryState,
                        "query_snapshot.workspace_view_revision_ref",
                    ));
                }
            }
            SourceView::GitCommit(_)
            | SourceView::ImportedSnapshot(_)
            | SourceView::RetainedRevision(_) => {
                if self.workspace_view_revision_ref.is_some() {
                    return Err(ContractError::new(
                        ContractErrorKind::ContradictoryState,
                        "query_snapshot.workspace_view_revision_ref",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Canonical domain-separated bytes whose BLAKE3-256 digest is
    /// `snapshot_fingerprint`. The fingerprint field itself is excluded.
    pub fn canonical_fingerprint_input(&self) -> Result<CanonicalBytes, ContractError> {
        self.validate()?;
        domain_separated_preimage(
            "eliot-search/query-snapshot-fingerprint/v1",
            &self.canonical_fingerprint_value()?,
        )
    }

    fn canonical_fingerprint_value(&self) -> Result<CanonicalValue, ContractError> {
        canonical_object(vec![
            (
                "installation_incarnation_id",
                canonical_uuid(self.installation_incarnation_id.as_bytes())?,
            ),
            (
                "collection_generation_id",
                canonical_optional_uuid(
                    self.collection_generation_id
                        .as_ref()
                        .map(CollectionGenerationId::as_bytes),
                )?,
            ),
            (
                "visible_epoch",
                canonical_optional_u64(self.visible_epoch.map(crate::Epoch::get).map(i64_to_u64)),
            ),
            (
                "collection_route_revision",
                CanonicalValue::U64(self.collection_route_revision.get()),
            ),
            (
                "catalog_revision",
                CanonicalValue::U64(self.catalog_revision.get()),
            ),
            (
                "membership_revision",
                CanonicalValue::U64(self.membership_revision.get()),
            ),
            (
                "reference_portfolio_revision",
                self.reference_portfolio_revision
                    .map_or(Ok(CanonicalValue::Null), |value| {
                        Ok(CanonicalValue::U64(value.get()))
                    })?,
            ),
            (
                "access_policy_revision",
                CanonicalValue::U64(self.access_policy_revision.get()),
            ),
            (
                "shadow_fence_revision",
                CanonicalValue::U64(self.shadow_fence_revision.get()),
            ),
            (
                "purge_fence_revision",
                CanonicalValue::U64(self.purge_fence_revision.get()),
            ),
            (
                "overlay_revision",
                CanonicalValue::U64(self.overlay_revision.get()),
            ),
            (
                "observation_cursor_revision",
                CanonicalValue::U64(self.observation_cursor_revision.get()),
            ),
            (
                "observation_freshness",
                canonical_observation_freshness(self.observation_freshness)?,
            ),
            ("source_view", canonical_source_view(&self.source_view)?),
            (
                "workspace_view_revision_ref",
                canonical_optional_uuid(
                    self.workspace_view_revision_ref
                        .as_ref()
                        .map(WorkspaceViewRevisionId::as_bytes),
                )?,
            ),
            (
                "lexical_profile_ids",
                canonical_profile_list(&self.lexical_profile_ids)?,
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceOwnerFence {
    pub source_namespace_id: SourceNamespaceId,
    pub source_owner_generation: SourceOwnerGeneration,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateDependencyKind {
    MaterializerProfile,
    UnitizerProfile,
    EnricherProfile,
    ProviderCapability,
    OverlapRouteProof,
    RetentionLease,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateDependency {
    pub kind: StateDependencyKind,
    pub identity_digest: Blake3Digest32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RequiredDenominator {
    CandidateScope,
    CompleteScope,
    UnknownAllowed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExactnessRequirements {
    pub required_denominator: RequiredDenominator,
    pub require_current_observation: bool,
    pub allow_truthful_partial: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GrantFence {
    pub grant_id: GrantId,
    pub revocation_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientScopeFence {
    pub client_scope_ref: OpaqueRef,
    pub scope_domain_id: ScopeDomainId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileFence {
    pub fusion_profile_id: FusionProfileId,
    pub projection_profile_set_ids: BoundedList<ProjectionProfileSetId, MAX_LIST_ITEMS>,
    pub optional_provider_profile_ids: BoundedList<ProfileId, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchTaskPlan {
    pub plan_id: PlanId,
    pub provider_protocol_version: ProtocolVersion,
    pub request_id: RequestId,
    pub recipe_request_digest: Blake3Digest32,
    pub grant_fence: GrantFence,
    pub client_scope_fence: ClientScopeFence,
    pub query_snapshot_fence: QuerySnapshotFence,
    pub source_owner_fences: BoundedList<SourceOwnerFence, MAX_LIST_ITEMS>,
    pub selected_membership_ids: BoundedList<SourceMembershipId, MAX_LIST_ITEMS>,
    pub profile_fence: ProfileFence,
    pub overlay_snapshot_refs: BoundedList<OpaqueRef, MAX_LIST_ITEMS>,
    pub query_execution_budget: QueryExecutionBudget,
    pub exactness_requirements: ExactnessRequirements,
    pub additional_state_dependencies: BoundedList<StateDependency, MAX_LIST_ITEMS>,
    pub plan_fingerprint: PlanFingerprint,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
}

impl SearchTaskPlan {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.query_snapshot_fence.validate()?;
        if self.expires_at <= self.created_at {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "task_plan.expires_at",
            ));
        }
        Ok(())
    }

    /// Canonical domain-separated plan identity input. `plan_id`, timestamps,
    /// and `plan_fingerprint` are deliberately excluded from their own digest.
    pub fn canonical_fingerprint_input(&self) -> Result<CanonicalBytes, ContractError> {
        self.validate()?;
        let source_owner_fences = canonical_list(
            self.source_owner_fences
                .iter()
                .map(canonical_source_owner_fence)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let memberships = canonical_list(
            self.selected_membership_ids
                .iter()
                .map(|value| canonical_uuid(value.as_bytes()))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let overlay_refs = canonical_list(
            self.overlay_snapshot_refs
                .iter()
                .map(|value| canonical_text(value.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let state_dependencies = canonical_list(
            self.additional_state_dependencies
                .iter()
                .map(canonical_state_dependency)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let value = canonical_object(vec![
            (
                "provider_protocol_version",
                canonical_protocol_version(self.provider_protocol_version)?,
            ),
            ("request_id", canonical_uuid(self.request_id.as_bytes())?),
            (
                "recipe_request_digest",
                canonical_digest(self.recipe_request_digest.as_bytes())?,
            ),
            ("grant_fence", canonical_grant_fence(self.grant_fence)?),
            (
                "client_scope_fence",
                canonical_client_scope_fence(&self.client_scope_fence)?,
            ),
            (
                "query_snapshot_fence",
                canonical_query_snapshot_with_digest(&self.query_snapshot_fence)?,
            ),
            ("source_owner_fences", source_owner_fences),
            ("selected_membership_ids", memberships),
            (
                "profile_fence",
                canonical_profile_fence(&self.profile_fence)?,
            ),
            ("overlay_snapshot_refs", overlay_refs),
            (
                "query_execution_budget",
                canonical_query_budget(self.query_execution_budget)?,
            ),
            (
                "exactness_requirements",
                canonical_exactness(self.exactness_requirements)?,
            ),
            ("additional_state_dependencies", state_dependencies),
        ])?;
        domain_separated_preimage("eliot-search/plan-fingerprint/v1", &value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmissionSecurityFence {
    pub access_policy_revision: AccessPolicyRevision,
    pub live_deny_generation: u64,
    pub shadow_fence_revision: ShadowFenceRevision,
    pub purge_fence_revision: PurgeFenceRevision,
    pub checked_at: UtcTimestamp,
    pub receipt_ref: ReceiptRef,
}

/// Finite coordinate scalar. NaN and infinities cannot be constructed.
#[derive(Clone, Copy, Debug)]
pub struct FiniteF64(f64);

impl FiniteF64 {
    pub fn new(value: f64) -> Result<Self, ContractError> {
        if !value.is_finite() {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "finite_f64",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for FiniteF64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Eq for FiniteF64 {}
impl PartialOrd for FiniteF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FiniteF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl Hash for FiniteF64 {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PositionEncoding {
    Utf8Bytes,
    Utf16CodeUnits,
    Utf32Codepoints,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBytesAnchor {
    pub content_digest: Blake3Digest32,
    pub byte_start_0: u64,
    pub byte_end_exclusive_0: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitBlobBytesAnchor {
    pub repository_lineage_id: RepositoryLineageId,
    pub commit_oid: GitObjectId,
    pub path_bytes: BoundedBytes<MAX_RAW_BYTES>,
    pub byte_start_0: u64,
    pub byte_end_exclusive_0: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BufferRangeAnchor {
    pub buffer_snapshot_id: BufferSnapshotId,
    pub buffer_version: u64,
    pub position_encoding: PositionEncoding,
    pub start_line_0: u64,
    pub start_character_0: u64,
    pub end_line_0: u64,
    pub end_character_0: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PdfRegionAnchor {
    pub source_revision_id: SourceRevisionId,
    pub page_1: u64,
    pub x0: FiniteF64,
    pub y0: FiniteF64,
    pub x1: FiniteF64,
    pub y1: FiniteF64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveMemberAnchor {
    pub archive_revision_id: SourceRevisionId,
    pub member_path_bytes: BoundedBytes<MAX_RAW_BYTES>,
    pub nested_anchor: Box<NativeAnchor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeAnchor {
    TextBytes(TextBytesAnchor),
    GitBlobBytes(GitBlobBytesAnchor),
    BufferRange(BufferRangeAnchor),
    PdfRegion(PdfRegionAnchor),
    ArchiveMember(ArchiveMemberAnchor),
}

impl NativeAnchor {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.validate_at_depth(0)
    }

    fn validate_at_depth(&self, depth: usize) -> Result<(), ContractError> {
        if depth > crate::MAX_ANCHOR_DEPTH {
            return Err(ContractError::new(
                ContractErrorKind::DepthExceeded,
                "native_anchor",
            ));
        }
        match self {
            Self::TextBytes(anchor) => validate_range(
                anchor.byte_start_0,
                anchor.byte_end_exclusive_0,
                "text_bytes_anchor",
            ),
            Self::GitBlobBytes(anchor) => validate_range(
                anchor.byte_start_0,
                anchor.byte_end_exclusive_0,
                "git_blob_anchor",
            ),
            Self::BufferRange(anchor) => {
                if (anchor.start_line_0, anchor.start_character_0)
                    > (anchor.end_line_0, anchor.end_character_0)
                {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "buffer_range_anchor",
                    ));
                }
                Ok(())
            }
            Self::PdfRegion(anchor) => {
                if anchor.page_1 == 0 || anchor.x0 > anchor.x1 || anchor.y0 > anchor.y1 {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "pdf_region_anchor",
                    ));
                }
                Ok(())
            }
            Self::ArchiveMember(anchor) => anchor
                .nested_anchor
                .validate_at_depth(depth.saturating_add(1)),
        }
    }
}

fn validate_range(start: u64, end: u64, field: &'static str) -> Result<(), ContractError> {
    if start > end {
        return Err(ContractError::new(ContractErrorKind::InvalidRange, field));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactPredicateKind {
    Literal,
    Regex,
    QualifiedSymbol,
    StructuralPattern,
    RecordField,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactInputDomain {
    RawBytes,
    DecodedText,
    StructuralIr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPredicate {
    pub kind: ExactPredicateKind,
    pub engine_and_version: ProfileId,
    pub serialized_form: BoundedCanonicalBytes<MAX_RAW_BYTES>,
    pub input_domain: ExactInputDomain,
    pub worst_case_complexity_class: ProfileId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExactCompletenessRequirements {
    pub require_every_denominator_item: bool,
    pub require_stable_or_retained_revision: bool,
    pub require_current_observation: bool,
    pub include_authenticated_unsaved_buffers: bool,
    pub fail_on_timeout: bool,
    pub fail_on_cancellation: bool,
    pub fail_on_scope_drift: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactScanDenominator {
    pub source_revision_ids: BoundedList<SourceRevisionId, MAX_LIST_ITEMS>,
    pub inventory_revision: CatalogRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactScanPlan {
    pub plan_id: PlanId,
    pub predicate: ExactPredicate,
    pub denominator: ExactScanDenominator,
    pub inclusion_policy_digest: Blake3Digest32,
    pub unsaved_buffer_snapshot_ids: BoundedList<BufferSnapshotId, MAX_LIST_ITEMS>,
    pub completeness_requirements: ExactCompletenessRequirements,
    pub plan_fingerprint: PlanFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactMatch {
    pub source_revision_ref: SourceRevisionRef,
    pub native_anchor: NativeAnchor,
    pub match_digest: Blake3Digest32,
    pub matched_byte_length: u64,
    pub predicate_profile_id: ProfileId,
    pub assurance: crate::AssuranceClass,
    pub source_handle: SearchSourceHandle,
}

impl ExactMatch {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.native_anchor.validate()?;
        if !matches!(
            self.assurance,
            crate::AssuranceClass::ExactBytes | crate::AssuranceClass::MappedText
        ) {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "exact_match.assurance",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactItemFailureKind {
    Unreadable,
    RevisionUnavailable,
    ScopeChanged,
    Timeout,
    Cancelled,
    UnsupportedEncoding,
    PredicateError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactItemFailure {
    pub source_revision_id: SourceRevisionId,
    pub failure_kind: ExactItemFailureKind,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    pub bounded_metadata: BoundedNonContentMetadata,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoverageDenominatorKind {
    CandidateScope,
    CompleteScope,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactConclusion {
    MatchesFound,
    NoMatchInCompleteScope,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactExecutionReport {
    pub plan_ref: ExactScanPlanRef,
    pub matched_items: BoundedList<ExactMatch, MAX_LIST_ITEMS>,
    pub scanned_items: u64,
    pub scanned_bytes: u64,
    pub unreadable_items: BoundedList<ExactItemFailure, MAX_LIST_ITEMS>,
    pub changed_or_unavailable_items: BoundedList<ExactItemFailure, MAX_LIST_ITEMS>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub scope_drifted: bool,
    pub coverage: CoverageDenominatorKind,
    pub conclusion: ExactConclusion,
    pub receipt_ref: ReceiptRef,
}

impl ExactExecutionReport {
    pub fn validate(&self) -> Result<(), ContractError> {
        for exact_match in &self.matched_items {
            exact_match.validate()?;
        }
        let incomplete = !self.unreadable_items.is_empty()
            || !self.changed_or_unavailable_items.is_empty()
            || self.timed_out
            || self.cancelled
            || self.scope_drifted;
        if self.coverage == CoverageDenominatorKind::CompleteScope && incomplete {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "exact_report.complete_coverage",
            ));
        }
        match self.conclusion {
            ExactConclusion::MatchesFound if self.matched_items.is_empty() => {
                Err(ContractError::new(
                    ContractErrorKind::ContradictoryState,
                    "exact_report.matches_found",
                ))
            }
            ExactConclusion::NoMatchInCompleteScope
                if !self.matched_items.is_empty()
                    || incomplete
                    || self.coverage != CoverageDenominatorKind::CompleteScope =>
            {
                Err(ContractError::new(
                    ContractErrorKind::ContradictoryState,
                    "exact_report.complete_negative",
                ))
            }
            ExactConclusion::Incomplete
                if self.coverage == CoverageDenominatorKind::CompleteScope =>
            {
                Err(ContractError::new(
                    ContractErrorKind::ContradictoryState,
                    "exact_report.incomplete",
                ))
            }
            ExactConclusion::MatchesFound
            | ExactConclusion::NoMatchInCompleteScope
            | ExactConclusion::Incomplete => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeLeg {
    pub leg_ref: OpaqueId,
    pub leg_kind: LegKind,
    pub authorized_scope_ref: AuthorizedScopeRef,
    pub access_partition_id: Option<AccessPartitionId>,
    pub scoring_partition_id: Option<ScoringPartitionId>,
    pub projection_membership_ids: BoundedSet<ProjectionMembershipId, MAX_SET_ITEMS>,
    pub profile_id: ProfileId,
    pub budget: QueryExecutionBudget,
    pub eligibility_predicate_digest: Blake3Digest32,
    pub idf_predicate_digest: Option<Blake3Digest32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegExecutionState {
    Completed,
    Partial,
    Cancelled,
    Failed,
    DiscardedContaminated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegExecutionSummary {
    pub leg_ref: OpaqueId,
    pub state: LegExecutionState,
    pub nominated_count: u32,
    pub validated_count: u32,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    pub receipt_ref: ReceiptRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationObservation {
    pub predicate: BoundedExpression,
    pub observation: BoundedObservation,
    pub evidence_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub assurance: crate::AssuranceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorObservation {
    pub axis: crate::ComparisonAxis,
    pub summary: BoundedObservation,
    pub evidence_handles: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    pub configuration_predicate: Option<BoundedExpression>,
    pub independent_lineage_count: u32,
    pub assurance: crate::AssuranceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorConflict {
    pub axis: crate::ComparisonAxis,
    pub left: BehaviorObservation,
    pub right: BehaviorObservation,
    pub conflict_summary: BoundedObservation,
    pub unresolved_reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

fn canonical_object(
    entries: Vec<(&'static str, CanonicalValue)>,
) -> Result<CanonicalValue, ContractError> {
    BoundedMap::<CanonicalKey, CanonicalValue, MAX_MAP_ENTRIES>::from_entries(
        entries
            .into_iter()
            .map(|(key, value)| CanonicalKey::new_non_empty(key).map(|key| (key, value)))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map(CanonicalValue::Object)
}

fn canonical_list(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

fn canonical_text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

fn canonical_uuid(value: &[u8; 16]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.to_vec()).map(CanonicalValue::Bytes)
}

fn canonical_digest(value: &[u8; 32]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.to_vec()).map(CanonicalValue::Bytes)
}

fn canonical_optional_uuid(value: Option<&[u8; 16]>) -> Result<CanonicalValue, ContractError> {
    value.map_or(Ok(CanonicalValue::Null), canonical_uuid)
}

fn canonical_optional_u64(value: Option<u64>) -> CanonicalValue {
    value.map_or(CanonicalValue::Null, CanonicalValue::U64)
}

fn i64_to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn canonical_profile_list(
    values: &BoundedList<ProfileId, MAX_LIST_ITEMS>,
) -> Result<CanonicalValue, ContractError> {
    canonical_list(
        values
            .iter()
            .map(|value| canonical_text(value.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn canonical_observation_freshness(
    value: ObservationFreshness,
) -> Result<CanonicalValue, ContractError> {
    value.validate()?;
    canonical_object(vec![
        ("state", canonical_text(value.state.as_str())?),
        (
            "observation_cursor_revision",
            CanonicalValue::U64(value.observation_cursor_revision.get()),
        ),
        (
            "observed_age_ms",
            canonical_optional_u64(value.observed_age_ms),
        ),
    ])
}

fn canonical_source_view(value: &SourceView) -> Result<CanonicalValue, ContractError> {
    let (tag, body) = match value {
        SourceView::WorkingTreeCurrent(view) => (
            "working_tree_current",
            canonical_object(vec![
                (
                    "workspace_instance_id",
                    canonical_uuid(view.workspace_instance_id.as_bytes())?,
                ),
                (
                    "workspace_view_revision_ref",
                    canonical_uuid(view.workspace_view_revision_ref.as_bytes())?,
                ),
            ])?,
        ),
        SourceView::GitIndex(view) => (
            "git_index",
            canonical_object(vec![
                (
                    "workspace_instance_id",
                    canonical_uuid(view.workspace_instance_id.as_bytes())?,
                ),
                (
                    "workspace_view_revision_ref",
                    canonical_uuid(view.workspace_view_revision_ref.as_bytes())?,
                ),
            ])?,
        ),
        SourceView::GitCommit(view) => (
            "git_commit",
            canonical_object(vec![
                (
                    "workspace_instance_id",
                    canonical_uuid(view.workspace_instance_id.as_bytes())?,
                ),
                (
                    "git_commit_oid",
                    canonical_text(view.git_commit_oid.as_str())?,
                ),
            ])?,
        ),
        SourceView::ImportedSnapshot(snapshot_id) => (
            "imported_snapshot",
            canonical_object(vec![(
                "imported_snapshot_id",
                canonical_uuid(snapshot_id.as_bytes())?,
            )])?,
        ),
        SourceView::RetainedRevision(revision_id) => (
            "retained_revision",
            canonical_object(vec![(
                "retained_revision_id",
                canonical_uuid(revision_id.as_bytes())?,
            )])?,
        ),
    };
    canonical_object(vec![(tag, body)])
}

fn canonical_source_owner_fence(value: &SourceOwnerFence) -> Result<CanonicalValue, ContractError> {
    canonical_object(vec![
        (
            "source_namespace_id",
            canonical_uuid(value.source_namespace_id.as_bytes())?,
        ),
        (
            "source_owner_generation",
            canonical_digest(value.source_owner_generation.as_bytes())?,
        ),
    ])
}

fn canonical_protocol_version(value: ProtocolVersion) -> Result<CanonicalValue, ContractError> {
    canonical_object(vec![
        ("major", CanonicalValue::U64(u64::from(value.major))),
        ("minor", CanonicalValue::U64(u64::from(value.minor))),
    ])
}

fn canonical_grant_fence(value: GrantFence) -> Result<CanonicalValue, ContractError> {
    canonical_object(vec![
        ("grant_id", canonical_uuid(value.grant_id.as_bytes())?),
        (
            "revocation_generation",
            CanonicalValue::U64(value.revocation_generation),
        ),
    ])
}

fn canonical_client_scope_fence(value: &ClientScopeFence) -> Result<CanonicalValue, ContractError> {
    canonical_object(vec![
        (
            "client_scope_ref",
            canonical_text(value.client_scope_ref.as_str())?,
        ),
        (
            "scope_domain_id",
            canonical_uuid(value.scope_domain_id.as_bytes())?,
        ),
    ])
}

fn canonical_query_snapshot_with_digest(
    value: &QuerySnapshotFence,
) -> Result<CanonicalValue, ContractError> {
    let CanonicalValue::Object(mut entries) = value.canonical_fingerprint_value()? else {
        return Err(ContractError::new(
            ContractErrorKind::ContradictoryState,
            "query_snapshot.canonical_value",
        ));
    };
    entries.insert(
        CanonicalKey::new_non_empty("snapshot_fingerprint")?,
        canonical_digest(value.snapshot_fingerprint.as_bytes())?,
    )?;
    Ok(CanonicalValue::Object(entries))
}

fn canonical_profile_fence(value: &ProfileFence) -> Result<CanonicalValue, ContractError> {
    let projection_profiles = canonical_list(
        value
            .projection_profile_set_ids
            .iter()
            .map(|profile| canonical_text(profile.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let optional_profiles = canonical_list(
        value
            .optional_provider_profile_ids
            .iter()
            .map(|profile| canonical_text(profile.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    canonical_object(vec![
        (
            "fusion_profile_id",
            canonical_text(value.fusion_profile_id.as_str())?,
        ),
        ("projection_profile_set_ids", projection_profiles),
        ("optional_provider_profile_ids", optional_profiles),
    ])
}

fn canonical_query_budget(value: QueryExecutionBudget) -> Result<CanonicalValue, ContractError> {
    let priority = match value.priority_class {
        PriorityClass::Interactive => "interactive",
        PriorityClass::Verification => "verification",
        PriorityClass::Background => "background",
    };
    canonical_object(vec![
        ("priority_class", canonical_text(priority)?),
        ("deadline_ms", CanonicalValue::U64(value.deadline_ms)),
        (
            "max_scoring_legs",
            CanonicalValue::U64(u64::from(value.max_scoring_legs)),
        ),
        (
            "max_prefetch_candidates_per_leg",
            CanonicalValue::U64(u64::from(value.max_prefetch_candidates_per_leg)),
        ),
        (
            "max_validated_candidates",
            CanonicalValue::U64(u64::from(value.max_validated_candidates)),
        ),
        (
            "max_source_read_bytes",
            CanonicalValue::U64(value.max_source_read_bytes),
        ),
        (
            "max_exact_scan_items",
            CanonicalValue::U64(value.max_exact_scan_items),
        ),
        (
            "max_exact_scan_bytes",
            CanonicalValue::U64(value.max_exact_scan_bytes),
        ),
        (
            "max_materialized_result_bytes",
            CanonicalValue::U64(value.max_materialized_result_bytes),
        ),
        ("max_cpu_ms", CanonicalValue::U64(value.max_cpu_ms)),
        (
            "max_memory_bytes",
            CanonicalValue::U64(value.max_memory_bytes),
        ),
    ])
}

fn canonical_exactness(value: ExactnessRequirements) -> Result<CanonicalValue, ContractError> {
    let denominator = match value.required_denominator {
        RequiredDenominator::CandidateScope => "candidate_scope",
        RequiredDenominator::CompleteScope => "complete_scope",
        RequiredDenominator::UnknownAllowed => "unknown_allowed",
    };
    canonical_object(vec![
        ("required_denominator", canonical_text(denominator)?),
        (
            "require_current_observation",
            CanonicalValue::Bool(value.require_current_observation),
        ),
        (
            "allow_truthful_partial",
            CanonicalValue::Bool(value.allow_truthful_partial),
        ),
    ])
}

fn canonical_state_dependency(value: &StateDependency) -> Result<CanonicalValue, ContractError> {
    let kind = match value.kind {
        StateDependencyKind::MaterializerProfile => "materializer_profile",
        StateDependencyKind::UnitizerProfile => "unitizer_profile",
        StateDependencyKind::EnricherProfile => "enricher_profile",
        StateDependencyKind::ProviderCapability => "provider_capability",
        StateDependencyKind::OverlapRouteProof => "overlap_route_proof",
        StateDependencyKind::RetentionLease => "retention_lease",
    };
    canonical_object(vec![
        ("kind", canonical_text(kind)?),
        (
            "identity_digest",
            canonical_digest(value.identity_digest.as_bytes())?,
        ),
    ])
}

// Keep imports of semantically load-bearing IDs visible in rustdoc and avoid
// accidentally replacing them with open strings in downstream schemas.
crate::impl_wire_enum!(LegKind {
    Direct => "direct",
    Exact => "exact",
    Structural => "structural",
    Lexical => "lexical",
    Semantic => "semantic",
    Rerank => "rerank",
});
crate::impl_wire_enum!(CoverageGapKind {
    UnavailableMembership => "unavailable_membership",
    FailedLeg => "failed_leg",
    OmittedBudget => "omitted_budget",
    ObservationGap => "observation_gap",
    SourceUnreadable => "source_unreadable",
    ValidationGap => "validation_gap",
    AccessRevoked => "access_revoked",
    Purge => "purge",
    ProviderDegraded => "provider_degraded",
});
crate::impl_wire_enum!(Retryability {
    Never => "never",
    SameRequest => "same_request",
    AfterRefresh => "after_refresh",
    AfterReconcile => "after_reconcile",
});
crate::impl_wire_enum!(PriorityClass {
    Interactive => "interactive",
    Verification => "verification",
    Background => "background",
});
crate::impl_wire_enum!(StateDependencyKind {
    MaterializerProfile => "materializer_profile",
    UnitizerProfile => "unitizer_profile",
    EnricherProfile => "enricher_profile",
    ProviderCapability => "provider_capability",
    OverlapRouteProof => "overlap_route_proof",
    RetentionLease => "retention_lease",
});
crate::impl_wire_enum!(RequiredDenominator {
    CandidateScope => "candidate_scope",
    CompleteScope => "complete_scope",
    UnknownAllowed => "unknown_allowed",
});
crate::impl_wire_enum!(PositionEncoding {
    Utf8Bytes => "utf8_bytes",
    Utf16CodeUnits => "utf16_code_units",
    Utf32Codepoints => "utf32_codepoints",
});
crate::impl_wire_enum!(ExactPredicateKind {
    Literal => "literal",
    Regex => "regex",
    QualifiedSymbol => "qualified_symbol",
    StructuralPattern => "structural_pattern",
    RecordField => "record_field",
});
crate::impl_wire_enum!(ExactInputDomain {
    RawBytes => "raw_bytes",
    DecodedText => "decoded_text",
    StructuralIr => "structural_ir",
});
crate::impl_wire_enum!(ExactItemFailureKind {
    Unreadable => "unreadable",
    RevisionUnavailable => "revision_unavailable",
    ScopeChanged => "scope_changed",
    Timeout => "timeout",
    Cancelled => "cancelled",
    UnsupportedEncoding => "unsupported_encoding",
    PredicateError => "predicate_error",
});
crate::impl_wire_enum!(CoverageDenominatorKind {
    CandidateScope => "candidate_scope",
    CompleteScope => "complete_scope",
    Unknown => "unknown",
});
crate::impl_wire_enum!(ExactConclusion {
    MatchesFound => "matches_found",
    NoMatchInCompleteScope => "no_match_in_complete_scope",
    Incomplete => "incomplete",
});
crate::impl_wire_enum!(LegExecutionState {
    Completed => "completed",
    Partial => "partial",
    Cancelled => "cancelled",
    Failed => "failed",
    DiscardedContaminated => "discarded_contaminated",
});

const _: Option<(
    AccessPolicyBindingId,
    BindingId,
    CorpusId,
    SourceId,
    EntityKind,
    EvidenceRole,
)> = None;
const _: Option<ContinuationHandle> = None;
