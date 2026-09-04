use crate::bounds::{BoundedList, BoundedSet, MAX_LIST_ITEMS, MAX_REASON_CODES, MAX_SET_ITEMS};
use crate::canonical::{
    BoundedDisplayName, BoundedDisplayPath, BoundedExpression, OpaqueCanonicalBytes, OpaqueId,
    OpaqueRef, UtcTimestamp,
};
use crate::ids::{
    AccessDomainId, AccessPartitionId, AccessPolicyBindingId, ArtifactDigest, Blake3Digest32,
    CatalogRevision, CollectionGenerationId, CollectionRouteRevision, ConfidentialityDomainId,
    CorpusId, CutoverId, DataRootId, DigestRef, EncryptionKeyDomainId, Epoch, ErasureDomainId,
    GitObjectId, ImportedSnapshotId, InstallationId, InstallationIncarnationId, MaterializationId,
    MembershipRevision, NonZeroRevision, ObjectResidencyKeyDigest, OwnerEpoch, PathBindingId,
    PolicyRevision, PortfolioRevision, ProfileId, ProjectionMembershipId, ReceiptRef,
    ReferencePortfolioId, RepositoryLineageId, RepresentationId, ResidencyPolicyBindingId,
    RetentionDomainId, RootBindingId, RuleId, ScopeDomainId, ScoringPartitionId, SourceId,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration, SourceRevisionId, UnitId,
    VersionedContentDigest, WorkspaceId, WorkspaceOrCorpusRef, WorkspaceViewRevisionId,
};
use crate::query::NativeAnchor;
use crate::reasons::SearchReasonCodeV1;
use crate::schema::{AssuranceClass, SensitivityClass};
use crate::{ContractError, ContractErrorKind, SourceViewRef};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ActiveMode {
    Standalone,
    ManagedClient,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchInstallation {
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub data_root_id: DataRootId,
    pub owner_epoch: OwnerEpoch,
    pub active_mode: ActiveMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionRoute {
    pub collection_generation_id: CollectionGenerationId,
    pub physical_collection_name: OpaqueRef,
    pub schema_identity_digest: Blake3Digest32,
    pub qualified_qdrant_build: ArtifactDigest,
    pub committed_visible_epoch: Epoch,
    pub collection_route_revision: CollectionRouteRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VcsKind {
    Git,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryLineage {
    pub lineage_id: RepositoryLineageId,
    pub vcs_kind: VcsKind,
    pub canonical_remote_fingerprints: BoundedSet<DigestRef, MAX_SET_ITEMS>,
    pub fork_relations: BoundedSet<RepositoryLineageId, MAX_SET_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceInstance {
    pub workspace_id: WorkspaceId,
    pub lineage_id: RepositoryLineageId,
    pub root_binding_id: RootBindingId,
    pub worktree_or_checkout_identity: OpaqueId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NamespaceOwnershipStatus {
    Active,
    CutoverPrepared,
    Fenced,
    Retired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceNamespaceOwnership {
    pub source_namespace_id: SourceNamespaceId,
    pub owner_system_id: OpaqueId,
    pub owner_installation_incarnation_id: InstallationIncarnationId,
    pub owner_epoch: OwnerEpoch,
    pub ownership_record_revision: NonZeroRevision,
    pub source_owner_generation: SourceOwnerGeneration,
    pub source_admission_policy_revision: PolicyRevision,
    pub status: NamespaceOwnershipStatus,
    pub cutover_receipt_ref: Option<ReceiptRef>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceOwnerCutoverProtocolV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerCutover {
    pub cutover_id: CutoverId,
    pub source_namespace_id: SourceNamespaceId,
    pub identity_mapping_digest: Blake3Digest32,
    pub prepared_at: UtcTimestamp,
    pub effective_at: UtcTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OldSourceOwnerFence {
    pub owner_system_id: OpaqueId,
    pub source_owner_generation_before_fence: SourceOwnerGeneration,
    pub fence_revision: NonZeroRevision,
    pub final_source_view_ref: SourceViewRef,
    pub final_revision_set_digest: Blake3Digest32,
    pub terminal_status: NamespaceOwnershipStatus,
}

impl OldSourceOwnerFence {
    pub fn validate(&self) -> Result<(), ContractError> {
        if !matches!(
            self.terminal_status,
            NamespaceOwnershipStatus::Fenced | NamespaceOwnershipStatus::Retired
        ) {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "old_owner_terminal_status",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSourceOwnerActivation {
    pub owner_system_id: OpaqueId,
    pub source_owner_generation_after_activation: SourceOwnerGeneration,
    pub activation_revision: NonZeroRevision,
    pub admitted_revision_set_digest: Blake3Digest32,
    pub status: NamespaceOwnershipStatus,
}

impl NewSourceOwnerActivation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.status != NamespaceOwnershipStatus::Active {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "new_owner_status",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnresolvedSource {
    pub source_id: SourceId,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

impl UnresolvedSource {
    pub fn new(
        source_id: SourceId,
        reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    ) -> Result<Self, ContractError> {
        if reason_codes.is_empty() {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "unresolved_source.reason_codes",
            ));
        }
        Ok(Self {
            source_id,
            reason_codes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverValidation {
    pub compatibility_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
    pub integrity_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
    pub unresolved_sources_and_reasons: BoundedList<UnresolvedSource, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverAuthorization {
    pub old_owner_authorization_ref: OpaqueRef,
    pub new_owner_authorization_ref: OpaqueRef,
    pub issued_at: UtcTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerCutoverReceipt {
    pub protocol: SourceOwnerCutoverProtocolV1,
    pub cutover: SourceOwnerCutover,
    pub old_owner: OldSourceOwnerFence,
    pub new_owner: NewSourceOwnerActivation,
    pub validation: CutoverValidation,
    pub authorization: CutoverAuthorization,
}

impl SourceOwnerCutoverReceipt {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.old_owner.validate()?;
        self.new_owner.validate()?;
        if self.cutover.effective_at < self.cutover.prepared_at {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "cutover.effective_at",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceIdentityKind {
    NtfsFile,
    GitBlobLineage,
    ImportedObject,
    AdmittedVirtualSnapshot,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIdentity {
    pub source_namespace_id: SourceNamespaceId,
    pub source_id: SourceId,
    pub identity_kind: SourceIdentityKind,
    pub stable_identity_components: OpaqueCanonicalBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathBinding {
    pub binding_id: PathBindingId,
    pub source_id: SourceId,
    pub workspace_id: WorkspaceId,
    pub display_path: BoundedDisplayPath,
    pub canonical_path_key: OpaqueCanonicalBytes,
    pub first_seen_revision: SourceRevisionId,
    pub last_seen_revision: Option<SourceRevisionId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcquisitionKind {
    Filesystem,
    GitObject,
    Imported,
    AdmittedIdeSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevision {
    pub revision_id: SourceRevisionId,
    pub source_id: SourceId,
    pub occurrence_sequence: u64,
    pub content_digest: Blake3Digest32,
    pub byte_length: u64,
    pub observed_at: UtcTimestamp,
    pub acquisition_kind: AcquisitionKind,
    pub stability_receipt_ref: ReceiptRef,
    pub object_residency_key_digest: ObjectResidencyKeyDigest,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevisionRef {
    pub source_namespace_id: SourceNamespaceId,
    pub source_id: SourceId,
    pub revision_id: SourceRevisionId,
    pub content_digest: Blake3Digest32,
    pub byte_length: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MembershipRole {
    Source,
    Test,
    Documentation,
    Generated,
    Vendor,
    Reference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMembership {
    pub source_membership_id: SourceMembershipId,
    pub corpus_id: CorpusId,
    pub source_id: SourceId,
    pub workspace_id: WorkspaceId,
    pub role: MembershipRole,
    pub preparation_profile_id: ProfileId,
    pub access_policy_binding_id: AccessPolicyBindingId,
    pub retention_policy_id: ProfileId,
    pub residency_policy_binding_id: ResidencyPolicyBindingId,
    pub membership_revision: MembershipRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortfolioRoleFilter {
    Source,
    Test,
    Documentation,
    Reference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencePortfolioRevision {
    pub portfolio_id: ReferencePortfolioId,
    pub portfolio_revision: PortfolioRevision,
    pub display_name: BoundedDisplayName,
    pub included_scopes: BoundedList<WorkspaceOrCorpusRef, MAX_LIST_ITEMS>,
    pub membership_precedence: BoundedList<SourceMembershipId, MAX_LIST_ITEMS>,
    pub lineage_collapse_policy_id: ProfileId,
    pub role_filters: BoundedSet<PortfolioRoleFilter, MAX_SET_ITEMS>,
    pub access_policy_binding_id: AccessPolicyBindingId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchObjectResidencyKey {
    pub scope_domain_id: ScopeDomainId,
    pub access_domain_id: AccessDomainId,
    pub confidentiality_domain_id: ConfidentialityDomainId,
    pub encryption_key_domain_id: EncryptionKeyDomainId,
    pub retention_domain_id: RetentionDomainId,
    pub erasure_domain_id: ErasureDomainId,
    pub versioned_content_digest: VersionedContentDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceResidencyProfileRef {
    pub residency_policy_binding_id: ResidencyPolicyBindingId,
    pub policy_revision: PolicyRevision,
    pub profile_id: ProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Materialization {
    pub materialization_id: MaterializationId,
    pub source_revision_id: SourceRevisionId,
    pub materializer_profile_id: ProfileId,
    pub canonical_object_digest: Blake3Digest32,
    pub object_residency_key_digest: ObjectResidencyKeyDigest,
    pub native_coordinate_map_digest: Blake3Digest32,
    pub loss_map_digest: Blake3Digest32,
    pub assurance_ceiling: AssuranceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Representation {
    pub representation_id: RepresentationId,
    pub materialization_id: MaterializationId,
    pub unitizer_profile_id: ProfileId,
    pub enrichment_profile_ids: BoundedList<ProfileId, MAX_LIST_ITEMS>,
    pub unit_manifest_digest: Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembership {
    pub projection_membership_id: ProjectionMembershipId,
    pub source_membership_id: SourceMembershipId,
    pub representation_id: RepresentationId,
    pub access_partition_id: AccessPartitionId,
    pub scoring_partition_id: ScoringPartitionId,
    pub projection_schema_id: ProfileId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnitKind {
    File,
    Section,
    Symbol,
    Reference,
    Test,
    Doc,
    Table,
    ImageRegion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitOccurrence {
    pub unit_id: UnitId,
    pub representation_id: RepresentationId,
    pub unit_kind: UnitKind,
    pub ordinal: u64,
    pub native_anchor: NativeAnchor,
    pub structural_identity: Option<OpaqueId>,
    pub configuration_predicate: Option<BoundedExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceViewSource {
    pub workspace_instance_id: WorkspaceId,
    pub workspace_view_revision_ref: WorkspaceViewRevisionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCommitSourceView {
    pub workspace_instance_id: WorkspaceId,
    pub git_commit_oid: GitObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceView {
    WorkingTreeCurrent(WorkspaceViewSource),
    GitIndex(WorkspaceViewSource),
    GitCommit(GitCommitSourceView),
    ImportedSnapshot(ImportedSnapshotId),
    RetainedRevision(SourceRevisionId),
}

impl SourceView {
    /// The tagged Rust enum makes illegal simultaneous variants unrepresentable.
    pub const fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceViewRevision {
    pub workspace_view_revision_id: WorkspaceViewRevisionId,
    pub workspace_instance_id: WorkspaceId,
    pub root_filesystem_identity: OpaqueCanonicalBytes,
    pub repository_lineage_id: Option<RepositoryLineageId>,
    pub head_commit_and_branch: Option<OpaqueCanonicalBytes>,
    pub git_index_identity: Option<OpaqueCanonicalBytes>,
    pub inventory_revision: CatalogRevision,
    pub worktree_observation_cursor: crate::ObservationCursorRevision,
    pub authenticated_ide_overlay_revision: u64,
    pub ignore_and_source_admission_policy_revision: PolicyRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceAdmissionMaximumLimits {
    pub max_source_bytes: u64,
    pub max_archive_bytes: u64,
    pub max_archive_members: u32,
    pub max_materialized_bytes: u64,
    pub max_expansion_ratio: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAdmissionPolicy {
    pub policy_revision: PolicyRevision,
    pub denied_system_locations: BoundedList<RuleId, MAX_LIST_ITEMS>,
    pub denied_filename_and_format_classes: BoundedList<RuleId, MAX_LIST_ITEMS>,
    pub secret_and_private_key_detectors: BoundedList<ProfileId, MAX_LIST_ITEMS>,
    pub generated_vendor_and_binary_policy_ref: OpaqueRef,
    pub maximum_limits: SourceAdmissionMaximumLimits,
    pub sensitivity_classes: BoundedSet<SensitivityClass, MAX_SET_ITEMS>,
    pub explicit_override_authority_ref: OpaqueRef,
    pub disclosure_and_logging_policy_ref: OpaqueRef,
}

crate::impl_wire_enum!(ActiveMode {
    Standalone => "standalone",
    ManagedClient => "managed_client",
});
crate::impl_wire_enum!(VcsKind { Git => "git", None => "none" });
crate::impl_wire_enum!(NamespaceOwnershipStatus {
    Active => "ACTIVE",
    CutoverPrepared => "CUTOVER_PREPARED",
    Fenced => "FENCED",
    Retired => "RETIRED",
});
crate::impl_wire_enum!(SourceIdentityKind {
    NtfsFile => "ntfs_file",
    GitBlobLineage => "git_blob_lineage",
    ImportedObject => "imported_object",
    AdmittedVirtualSnapshot => "admitted_virtual_snapshot",
});
crate::impl_wire_enum!(AcquisitionKind {
    Filesystem => "filesystem",
    GitObject => "git_object",
    Imported => "imported",
    AdmittedIdeSnapshot => "admitted_ide_snapshot",
});
crate::impl_wire_enum!(MembershipRole {
    Source => "source",
    Test => "test",
    Documentation => "documentation",
    Generated => "generated",
    Vendor => "vendor",
    Reference => "reference",
});
crate::impl_wire_enum!(PortfolioRoleFilter {
    Source => "source",
    Test => "test",
    Documentation => "documentation",
    Reference => "reference",
});
crate::impl_wire_enum!(UnitKind {
    File => "file",
    Section => "section",
    Symbol => "symbol",
    Reference => "reference",
    Test => "test",
    Doc => "doc",
    Table => "table",
    ImageRegion => "image_region",
});
