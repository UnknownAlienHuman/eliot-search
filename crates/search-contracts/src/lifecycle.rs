use crate::bounds::{BoundedList, BoundedSet, MAX_LIST_ITEMS, MAX_REASON_CODES, MAX_SET_ITEMS};
use crate::canonical::{BoundedExpression, OpaqueId, OpaqueRef, UtcTimestamp};
use crate::ids::{
    AccessPolicyRevision, BindingId, Blake3Digest32, BufferSnapshotId, CollectionGenerationId,
    ContinuationId, Epoch, GrantId, HandleId, HandleTokenDigest, InstallationIncarnationId,
    NonZeroRevision, ObjectResidencyKeyDigest, PlanFingerprint, ProfileId, ProjectionMembershipId,
    PublicationIntentId, PublicationReceiptId, PurgeFenceRevision, ReceiptRef, SourceNamespaceId,
    SourceOwnerGeneration, WorkspaceId, WorkspaceViewRevisionId,
};
use crate::query::{NativeAnchor, ObservationFreshness, StateDependency};
use crate::reasons::SearchReasonCodeV1;
use crate::results::ResultFence;
use crate::schema::AssuranceClass;
use crate::source::{SourceRevisionRef, SourceView};
use crate::{ContractError, ContractErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LifecycleRecordStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedSourceTarget {
    pub source_namespace_id: SourceNamespaceId,
    pub source_owner_generation: SourceOwnerGeneration,
    pub source_revision_ref: SourceRevisionRef,
    pub source_view: SourceView,
    pub workspace_view_revision_ref: Option<WorkspaceViewRevisionId>,
    pub native_anchor: NativeAnchor,
    pub excerpt_digest: Blake3Digest32,
    pub materialization_profile_id: ProfileId,
    pub assurance_ceiling: AssuranceClass,
    pub object_residency_key_digest: ObjectResidencyKeyDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsavedBufferTarget {
    pub workspace_id: WorkspaceId,
    pub workspace_view_revision_ref: WorkspaceViewRevisionId,
    pub buffer_snapshot_id: BufferSnapshotId,
    pub buffer_version: u64,
    pub native_anchor: NativeAnchor,
    pub excerpt_digest: Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EphemeralHandleTarget {
    RetainedSource(RetainedSourceTarget),
    UnsavedBuffer(UnsavedBufferTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EphemeralSourceHandleRecord {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub token_digest: HandleTokenDigest,
    pub binding_id: BindingId,
    pub grant_id: GrantId,
    pub target: EphemeralHandleTarget,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub invalidation_refs: BoundedList<OpaqueRef, MAX_LIST_ITEMS>,
    pub status: LifecycleRecordStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableSourceHandleRecord {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub token_digest: HandleTokenDigest,
    pub binding_id: BindingId,
    pub grant_id: GrantId,
    pub source_namespace_id: SourceNamespaceId,
    pub source_owner_generation: SourceOwnerGeneration,
    pub source_revision_ref: SourceRevisionRef,
    pub source_view: SourceView,
    pub workspace_view_revision_ref: Option<WorkspaceViewRevisionId>,
    pub native_anchor: NativeAnchor,
    pub excerpt_digest: Blake3Digest32,
    pub materialization_profile_id: ProfileId,
    pub assurance_ceiling: AssuranceClass,
    pub object_residency_key_digest: ObjectResidencyKeyDigest,
    pub retention_lease_ref: OpaqueRef,
    pub created_at: UtcTimestamp,
    pub retention_expiry: Option<UtcTimestamp>,
    pub invalidation_refs: BoundedList<OpaqueRef, MAX_LIST_ITEMS>,
    pub status: LifecycleRecordStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchSourceHandleRecord {
    Ephemeral(EphemeralSourceHandleRecord),
    DurableSource(DurableSourceHandleRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EphemeralWindowContinuation {
    pub continuation_id: ContinuationId,
    pub token_digest: HandleTokenDigest,
    pub binding_id: BindingId,
    pub plan_fingerprint: PlanFingerprint,
    pub result_fence: ResultFence,
    pub candidate_window_ref: OpaqueRef,
    pub issued_candidate_identity_set_ref: OpaqueRef,
    pub epoch_pin_ref: OpaqueRef,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub status: LifecycleRecordStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableReplanCheckpoint {
    pub continuation_id: ContinuationId,
    pub token_digest: HandleTokenDigest,
    pub binding_id: BindingId,
    pub plan_fingerprint: PlanFingerprint,
    pub result_fence: ResultFence,
    pub durable_job_ref: OpaqueRef,
    pub replan_checkpoint_ref: OpaqueRef,
    pub issued_candidate_identity_set_ref: OpaqueRef,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub status: LifecycleRecordStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuationRecord {
    EphemeralWindow(EphemeralWindowContinuation),
    DurableReplanCheckpoint(DurableReplanCheckpoint),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SecurityMutationPhase {
    Acquired,
    DurableCommitted,
    LiveSnapshotPublished,
    DependentsInvalidated,
    Acknowledged,
    FailClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityMutationBarrierState {
    pub security_domain_ref: OpaqueRef,
    pub phase: SecurityMutationPhase,
    pub access_policy_revision: AccessPolicyRevision,
    pub live_deny_generation: u64,
    pub mutation_receipt_ref: ReceiptRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveDenySnapshotRef {
    pub security_domain_ref: OpaqueRef,
    pub live_deny_generation: u64,
    pub snapshot_digest: Blake3Digest32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PublicationIntentState {
    Prepared,
    IntentDurable,
    NewPointsAcknowledged,
    OldPointsClosedAcknowledged,
    ReadbackVerified,
    ControlCommitted,
    Reclaimable,
    Compensating,
    Aborted,
    InvalidationOnlyCommitted,
    PublicationBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationIntent {
    pub publication_intent_id: PublicationIntentId,
    pub target_epoch: Epoch,
    pub prepared_manifest_ref: ReceiptRef,
    pub owner_source_membership_access_guards: BoundedList<StateDependency, MAX_LIST_ITEMS>,
    pub state: PublicationIntentState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationReceipt {
    pub publication_receipt_id: PublicationReceiptId,
    pub target_epoch: Epoch,
    pub exact_new_manifest_ref: ReceiptRef,
    pub exact_retired_manifest_ref: ReceiptRef,
    pub readback_digest: Blake3Digest32,
    pub control_commit_revision: NonZeroRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbandonedPublicationFence {
    pub publication_intent_id: PublicationIntentId,
    pub collection_generation_id: CollectionGenerationId,
    pub excluded_projection_memberships: BoundedSet<ProjectionMembershipId, MAX_SET_ITEMS>,
    pub excluded_partition_refs: BoundedSet<OpaqueRef, MAX_SET_ITEMS>,
    pub fence_revision: NonZeroRevision,
    pub receipt_ref: ReceiptRef,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompletionState {
    Pending,
    Complete,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeletionState {
    NotApplicable,
    Pending,
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BackupSnapshotStatus {
    NotPresent,
    Pending,
    RetainedTombstone,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SecureEraseStatus {
    NotGuaranteed,
    EvidenceAvailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalSecureErase {
    pub status: SecureEraseStatus,
    pub evidence_ref: Option<ReceiptRef>,
}

impl PhysicalSecureErase {
    pub fn validate(&self) -> Result<(), ContractError> {
        match (self.status, self.evidence_ref.is_some()) {
            (SecureEraseStatus::NotGuaranteed, false)
            | (SecureEraseStatus::EvidenceAvailable, true) => Ok(()),
            _ => Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "physical_secure_erase",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeReceipt {
    pub request_ref: OpaqueRef,
    pub fence_revision: PurgeFenceRevision,
    pub logical_non_accessibility: CompletionState,
    pub index_deletion: DeletionState,
    pub cache_deletion: DeletionState,
    pub backup_snapshot_status: BackupSnapshotStatus,
    pub physical_secure_erase: PhysicalSecureErase,
    pub revoked_handle_count: u64,
    pub tombstone_ref: ReceiptRef,
}

impl PurgeReceipt {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.physical_secure_erase.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedRecoveryManifest {
    pub installation_incarnation_id: InstallationIncarnationId,
    pub redb_checkpoint_digest: Blake3Digest32,
    pub qdrant_snapshot_identity: OpaqueId,
    pub collection_generation_id: CollectionGenerationId,
    pub schema_identity_digest: Blake3Digest32,
    pub committed_visible_epoch: Epoch,
    pub latest_publication_receipt_ref: ReceiptRef,
    pub purge_tombstone_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RestoreState {
    RestorePendingRevalidation,
    DirectOnly,
    IndexedAdmitted,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreDecision {
    pub state: RestoreState,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    pub validation_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalProviderLifecycleState {
    Absent,
    Stopped,
    Starting,
    Ready,
    Degraded,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionalProviderState {
    pub profile_id: ProfileId,
    pub state: OptionalProviderLifecycleState,
    pub artifact_identity_digest: Option<Blake3Digest32>,
    pub degraded_reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipReadiness {
    pub source_membership_id: crate::SourceMembershipId,
    pub direct_ready: bool,
    pub lexical_ready: bool,
    pub code_ready: bool,
    pub semantic_ready: bool,
    pub document_ready: bool,
    pub visible_epoch: Option<Epoch>,
    pub observation_freshness: ObservationFreshness,
    pub degraded_reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

// Prevent accidental substitution of executable predicates or open strings for
// lifecycle invalidation records.
crate::impl_wire_enum!(LifecycleRecordStatus {
    Active => "ACTIVE",
    Revoked => "REVOKED",
    Expired => "EXPIRED",
});
crate::impl_wire_enum!(SecurityMutationPhase {
    Acquired => "ACQUIRED",
    DurableCommitted => "DURABLE_COMMITTED",
    LiveSnapshotPublished => "LIVE_SNAPSHOT_PUBLISHED",
    DependentsInvalidated => "DEPENDENTS_INVALIDATED",
    Acknowledged => "ACKNOWLEDGED",
    FailClosed => "FAIL_CLOSED",
});
crate::impl_wire_enum!(PublicationIntentState {
    Prepared => "PREPARED",
    IntentDurable => "INTENT_DURABLE",
    NewPointsAcknowledged => "NEW_POINTS_ACKNOWLEDGED",
    OldPointsClosedAcknowledged => "OLD_POINTS_CLOSED_ACKNOWLEDGED",
    ReadbackVerified => "READBACK_VERIFIED",
    ControlCommitted => "CONTROL_COMMITTED",
    Reclaimable => "RECLAIMABLE",
    Compensating => "COMPENSATING",
    Aborted => "ABORTED",
    InvalidationOnlyCommitted => "INVALIDATION_ONLY_COMMITTED",
    PublicationBlocked => "PUBLICATION_BLOCKED",
});
crate::impl_wire_enum!(CompletionState {
    Pending => "pending",
    Complete => "complete",
    Failed => "failed",
});
crate::impl_wire_enum!(DeletionState {
    NotApplicable => "not_applicable",
    Pending => "pending",
    Complete => "complete",
    Partial => "partial",
    Failed => "failed",
});
crate::impl_wire_enum!(BackupSnapshotStatus {
    NotPresent => "not_present",
    Pending => "pending",
    RetainedTombstone => "retained_tombstone",
    Unresolved => "unresolved",
});
crate::impl_wire_enum!(SecureEraseStatus {
    NotGuaranteed => "not_guaranteed",
    EvidenceAvailable => "evidence_available",
});
crate::impl_wire_enum!(RestoreState {
    RestorePendingRevalidation => "RESTORE_PENDING_REVALIDATION",
    DirectOnly => "DIRECT_ONLY",
    IndexedAdmitted => "INDEXED_ADMITTED",
    Quarantined => "QUARANTINED",
});
crate::impl_wire_enum!(OptionalProviderLifecycleState {
    Absent => "absent",
    Stopped => "stopped",
    Starting => "starting",
    Ready => "ready",
    Degraded => "degraded",
    Quarantined => "quarantined",
});

const _: Option<BoundedExpression> = None;
