//! Immutable publication inputs, guards, receipts, and recovery evidence.

use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch, ReceiptRef,
};
use search_point_identity::PointId128;
use search_projection_planner::ProjectionManifest;

/// Complete load-bearing generation fence for one publication transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationGuards {
    /// Runtime-owner epoch.
    pub owner_epoch: OwnerEpoch,
    /// Source catalog generation.
    pub source_catalog_generation: u64,
    /// Membership catalog generation.
    pub membership_generation: u64,
    /// Access-policy generation.
    pub access_generation: u64,
    /// Shadow-fence generation.
    pub shadow_generation: u64,
    /// Purge-fence generation.
    pub purge_generation: u64,
    /// Accepted projection-profile digest.
    pub profile_digest: Blake3Digest32,
}

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbandonFence {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Reserved epoch that remains consumed.
    pub target_epoch: Epoch,
    /// Exact affected point IDs excluded before retrieval and IDF.
    pub excluded_point_ids: BTreeSet<PointId128>,
    /// Exact affected membership/partition-set digest.
    pub excluded_scope_digest: Blake3Digest32,
    /// Durable exclusion receipt.
    pub exclusion_receipt: ReceiptRef,
}

/// Exact compensation acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensationReceipt {
    /// Matching transaction identity.
    pub transaction_id: OpaqueId,
    /// Exact staged IDs removed or made invisible.
    pub compensated_ids: Vec<PointId128>,
    /// IDs not verified compensated.
    pub remaining_ids: Vec<PointId128>,
    /// Exact compensation readback receipt.
    pub readback_receipt: ReceiptRef,
}

/// Complete recovery observation for one unresolved publication.
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
    /// Whether an exclusion fence is already durable.
    pub abandon_fence_durable: bool,
}

/// Fail-closed recovery action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationRecoveryDecision {
    /// Continue exact staging or closure from durable intent.
    Continue,
    /// Control commit completed; publish/rebuild the immutable snapshot.
    PublishSnapshot,
    /// Remove or exclude only exact staged IDs.
    CompensateExact,
    /// Commit invalidation-only after a complete exclusion fence.
    CommitInvalidationOnly,
    /// Contradictory evidence blocks all later publications.
    PublicationBlocked,
}
