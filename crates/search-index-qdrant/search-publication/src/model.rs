//! Immutable publication inputs, guards, receipts, and recovery evidence.

use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, ReceiptRef,
};
use search_point_identity::PointId128;
use search_projection_planner::ProjectionManifest;

/// Shared publication guard value; this re-export preserves the existing import path.
pub use search_contracts::PublicationGuards;

/// Exact immutable publication input prepared before epoch reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPublication {
    /// Stable transaction identity.
    pub transaction_id: OpaqueId,
    /// Exact target collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Current exact manifest, when one is already visible.
    pub old_manifest: Option<ProjectionManifest>,
    /// Exact replacement manifest.
    pub new_manifest: ProjectionManifest,
    /// Digest of the old exact manifest, when present.
    pub old_manifest_digest: Option<Blake3Digest32>,
    /// Digest of the replacement exact manifest.
    pub new_manifest_digest: Blake3Digest32,
    /// Complete load-bearing guards.
    pub guards: PublicationGuards,
    /// Immutable receipt proving preparation inputs.
    pub preparation_receipt: ReceiptRef,
}

/// Exact stage acknowledgement and readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Reserved target epoch.
    pub target_epoch: Epoch,
    /// Exact point IDs inserted or updated.
    pub staged_ids: Vec<PointId128>,
    /// IDs missing from exact readback.
    pub missing_ids: Vec<PointId128>,
    /// IDs not present in the manifest but returned by readback.
    pub unexpected_ids: Vec<PointId128>,
    /// Digest of exact staged payload/vector readback.
    pub readback_digest: Blake3Digest32,
    /// External mutation receipt.
    pub mutation_receipt: ReceiptRef,
}

/// Exact old-point closure acknowledgement and readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Reserved target epoch used as exclusive upper bound.
    pub target_epoch: Epoch,
    /// Exact IDs closed at the target epoch.
    pub closed_ids: Vec<PointId128>,
    /// IDs whose closure was missing on exact readback.
    pub missing_ids: Vec<PointId128>,
    /// IDs not present in the retired manifest but acknowledged/read back.
    pub unexpected_ids: Vec<PointId128>,
    /// Digest of exact closure readback.
    pub readback_digest: Blake3Digest32,
    /// External mutation receipt.
    pub mutation_receipt: ReceiptRef,
}

/// Exact combined verification after staging and closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadbackVerified {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Reserved target epoch.
    pub target_epoch: Epoch,
    /// Digest of exact staged point readback.
    pub staged_readback_digest: Blake3Digest32,
    /// Digest of exact closure readback.
    pub closure_readback_digest: Blake3Digest32,
    /// Exact newly visible manifest digest.
    pub new_manifest_digest: Blake3Digest32,
    /// Exact retired manifest digest, when any points retire.
    pub retired_manifest_digest: Option<Blake3Digest32>,
}

/// Guarded control-state compare-and-swap observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCommitObservation {
    /// Visible epoch before the commit.
    pub before_visible_epoch: Epoch,
    /// Visible epoch after the commit.
    pub after_visible_epoch: Epoch,
    /// Guards read in the same control transaction.
    pub observed_guards: PublicationGuards,
    /// New control generation after the commit.
    pub control_generation: u64,
    /// Digest of exact committed control state.
    pub control_state_digest: Blake3Digest32,
}

/// Linearization-point receipt for a visible publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleCommitReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Newly visible epoch.
    pub visible_epoch: Epoch,
    /// Exact visible manifest digest.
    pub visible_manifest_digest: Blake3Digest32,
    /// Retired manifest digest, when present.
    pub retired_manifest_digest: Option<Blake3Digest32>,
    /// New control generation.
    pub control_generation: u64,
    /// Digest of exact committed control state.
    pub control_state_digest: Blake3Digest32,
}

/// Immutable in-memory control snapshot publication receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPublishReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Published visible epoch.
    pub visible_epoch: Epoch,
    /// Published control generation.
    pub control_generation: u64,
    /// Digest of the immutable snapshot.
    pub snapshot_digest: Blake3Digest32,
}

/// Exact committed retired-point manifest emitted for ordinary reclaim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredManifest {
    /// Collection generation containing retired points.
    pub collection_generation_id: CollectionGenerationId,
    /// First epoch at which those points are invisible.
    pub retirement_epoch_exclusive: Epoch,
    /// Canonically ordered exact point IDs.
    pub point_ids: Vec<PointId128>,
    /// Digest of the exact retired-ID manifest.
    pub manifest_digest: Blake3Digest32,
    /// Matching visible commit receipt reference.
    pub publication_receipt: ReceiptRef,
}

/// Exclusion fence required before abandoning a publication.
/// The access/control adapter must verify its durable effective scope before
/// constructing this value; these fields do not perform retrieval/IDF filtering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbandonFence {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Reserved epoch that remains consumed.
    pub target_epoch: Epoch,
    /// Exact affected point IDs, retained for effect accounting only.
    pub excluded_point_ids: BTreeSet<PointId128>,
    /// Complete affected projection memberships excluded before retrieval and IDF.
    /// Excluding only `excluded_point_ids` does not establish this wider fence.
    pub excluded_projection_memberships: BTreeSet<OpaqueId>,
    /// Exact affected membership/partition-set digest supplied by the scope owner.
    pub excluded_scope_digest: Blake3Digest32,
    /// Durable exclusion receipt.
    pub exclusion_receipt: ReceiptRef,
}

/// Exact compensation plan derived from both sides of the immutable manifest diff.
/// The adapter removes/excludes staged points and restores the prior validity of
/// old closed points. It must verify the original bounds, not just ID existence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensationPlan {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Consumed epoch whose effects are being reversed.
    pub target_epoch: Epoch,
    /// Canonically ordered exact new IDs to remove or exclude.
    pub staged_ids: Vec<PointId128>,
    /// Canonically ordered exact old IDs whose original validity must be restored.
    pub closed_ids: Vec<PointId128>,
}

/// Exact staged-point compensation acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensationReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Exact reservation; a receipt for another epoch cannot finish this attempt.
    pub target_epoch: Epoch,
    /// Exact staged IDs removed or made invisible.
    pub compensated_ids: Vec<PointId128>,
    /// IDs not verified compensated.
    pub remaining_ids: Vec<PointId128>,
    /// Exact compensation readback receipt.
    pub readback_receipt: ReceiptRef,
}

/// Verified restoration of the exact old point bounds from the prior manifest.
/// Separate from ClosureReceipt: closing old points is not their restoration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestorationReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Exact consumed epoch being compensated.
    pub target_epoch: Epoch,
    /// Canonically ordered IDs whose original validity was read back exactly.
    pub restored_ids: Vec<PointId128>,
    /// IDs not yet verified restored; any nonempty result blocks completion.
    pub remaining_ids: Vec<PointId128>,
    /// Adapter-owned exact restoration readback receipt.
    pub readback_receipt: ReceiptRef,
}

/// Recovery observation already bound to this transaction by its producing ports.
/// IDs nominate observed effects; they do not independently verify payload/vector
/// bytes, authorize a control commit or prove a complete exclusion scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationRecoveryObservation {
    /// Whether durable intent exists.
    pub intent_durable: bool,
    /// Exact IDs currently staged at the reserved epoch.
    pub staged_ids: Vec<PointId128>,
    /// Exact old IDs currently closed at the reserved epoch.
    pub closed_ids: Vec<PointId128>,
    /// Control visible epoch observed by authoritative readback.
    pub control_visible_epoch: Epoch,
    /// Whether immutable control snapshot publication is complete.
    pub snapshot_published: bool,
    /// Hint only: not sufficient for CommitInvalidationOnly or abandonment.
    /// The complete typed fence and its authoritative readback remain mandatory.
    pub abandon_fence_durable: bool,
}

/// Fail-closed recovery action; every write still goes through its normal verified port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationRecoveryDecision {
    /// Continue exact staging or closure from durable intent, not commit permission.
    Continue,
    /// Control commit completed; publish/rebuild the immutable snapshot.
    PublishSnapshot,
    /// Remove/exclude staged IDs and restore exact old bounds using the complete plan.
    CompensateExact,
    /// Reserved for a separately verified complete invalidation-only protocol.
    /// A boolean observation cannot produce this decision.
    CommitInvalidationOnly,
    /// Contradictory or insufficient evidence blocks later publications.
    PublicationBlocked,
}
