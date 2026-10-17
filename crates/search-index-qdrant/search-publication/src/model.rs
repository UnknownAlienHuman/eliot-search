//! Immutable publication inputs, guards, receipts, and recovery evidence.

use search_contracts::indexed::{PointPayload, VectorValue};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OpaqueRef, ReceiptRef, RequestId,
};
use search_point_identity::s11::{PointId128, PointIdentity};
use search_projection_planner::ProjectionManifest;

/// Shared publication guard value; this re-export preserves the existing import path.
pub use search_contracts::PublicationGuards;

/// Owner-validated immutable preparation, issued only by canonical admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedGeneration {
    pub(crate) transaction_id: OpaqueId,
    pub(crate) collection_generation_id: CollectionGenerationId,
    pub(crate) target: crate::admission::PublicationTarget,
    pub(crate) old_manifest: Option<ProjectionManifest>,
    pub(crate) new_manifest: ProjectionManifest,
    pub(crate) old_manifest_digest: Option<Blake3Digest32>,
    pub(crate) new_manifest_digest: Blake3Digest32,
    pub(crate) guards: PublicationGuards,
    pub(crate) prepared_input_digest: Blake3Digest32,
    pub(crate) difference: search_projection_planner::ManifestDiff,
    pub(crate) observed: crate::admission::PublicationOwnerObservation,
}

/// Complete immutable point state retained from the canonical projection plan.
/// This is an effect command input, never a readback proof or receipt.
#[derive(Clone, Debug)]
pub struct PublicationPointExpectation {
    pub(crate) point_id: PointId128,
    pub(crate) identity: PointIdentity,
    pub(crate) payload: PointPayload,
    pub(crate) vectors: std::collections::BTreeMap<String, VectorValue>,
}

impl PartialEq for PublicationPointExpectation {
    fn eq(&self, other: &Self) -> bool {
        self.point_id == other.point_id
            && self.identity == other.identity
            && self.payload == other.payload
            && vector_maps_equal(&self.vectors, &other.vectors)
    }
}
impl Eq for PublicationPointExpectation {}

impl PublicationPointExpectation {
    /// Exact collision-safe point address.
    #[must_use]
    pub const fn point_id(&self) -> PointId128 {
        self.point_id
    }
    /// Complete S11 identity, including its full digest.
    #[must_use]
    pub const fn identity(&self) -> &PointIdentity {
        &self.identity
    }
    /// Exact typed S9.5 payload, including its validity interval.
    #[must_use]
    pub const fn payload(&self) -> &PointPayload {
        &self.payload
    }
    /// Exact named vector values produced by the admitted encoder.
    #[must_use]
    pub const fn vectors(&self) -> &std::collections::BTreeMap<String, VectorValue> {
        &self.vectors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPointSets {
    pub(crate) old: Vec<PublicationPointExpectation>,
    pub(crate) new: Vec<PublicationPointExpectation>,
}

impl PreparedGeneration {
    /// Original immutable transaction identity.
    #[must_use]
    pub const fn transaction_id(&self) -> &OpaqueId {
        &self.transaction_id
    }
    /// Exact admitted target epoch.
    #[must_use]
    pub const fn target_epoch(&self) -> Epoch {
        self.observed.target_epoch
    }
    /// Route generation bound by canonical admission.
    #[must_use]
    pub const fn collection_generation_id(&self) -> CollectionGenerationId {
        self.collection_generation_id
    }
    /// Original owner guards bound by canonical admission.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }
    /// Exact old manifest digest, when replacing or retiring.
    #[must_use]
    pub const fn old_manifest_digest(&self) -> Option<Blake3Digest32> {
        self.old_manifest_digest
    }
    /// Exact canonical target manifest digest.
    #[must_use]
    pub const fn new_manifest_digest(&self) -> Blake3Digest32 {
        self.new_manifest_digest
    }
    /// Complete owner-bound target scope, including an explicit retirement scope.
    #[must_use]
    pub const fn scope(&self) -> &crate::admission::PublicationScope {
        self.target.scope()
    }
    /// Whether this is a complete representation or a zero-entry retirement.
    #[must_use]
    pub const fn target(&self) -> &crate::admission::PublicationTarget {
        &self.target
    }
    /// Exact immutable old manifest admitted through its owner token.
    #[must_use]
    pub const fn old_manifest(&self) -> Option<&ProjectionManifest> {
        self.old_manifest.as_ref()
    }
    /// Exact canonical replacement; an empty value is an admitted retirement.
    #[must_use]
    pub const fn new_manifest(&self) -> &ProjectionManifest {
        &self.new_manifest
    }
    /// Once-computed exact change set; later phases borrow it.
    #[must_use]
    pub const fn difference(&self) -> &search_projection_planner::ManifestDiff {
        &self.difference
    }
    /// Canonical C00 digest over the actual full frozen inputs, not an I/O receipt.
    #[must_use]
    pub const fn prepared_input_digest(&self) -> Blake3Digest32 {
        self.prepared_input_digest
    }
}

/// Phase identity carried on an effect command and its independent readback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationEffectKind {
    /// Persist the original immutable intent before index effects.
    PersistIntent,
    /// Stage new points at the reserved epoch.
    StageNewPoints,
    /// Close exact retired points at the reserved upper bound.
    CloseOldPoints,
    /// Independently re-read both sides of the complete publication diff.
    VerifyReadback,
    /// Commit the guarded visible epoch in control state.
    CommitVisibleEpoch,
    /// Publish the current-process immutable snapshot.
    PublishControlSnapshot,
    /// Remove/exclude staged points and restore exact retired point state.
    Compensate,
    /// Persist and read back a complete membership-wide exclusion fence.
    Abandon,
    /// Persist the guarded finalization of an aborted operation.
    CommitAbortFinalization,
    /// Publish the current-process snapshot after abort finalization.
    PublishAbortSnapshot,
}

/// An untrusted disposition returned by an effect/readback port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationEffectOutcome {
    /// Exact requested postcondition is present in authoritative readback.
    Applied,
    /// Authoritative readback proves the effect did not apply.
    NotApplied,
    /// Readback conflicts with the original immutable request.
    Conflict,
    /// The owner cannot yet determine whether the effect applied.
    Unknown,
}

/// Exact request binding echoed in an untrusted effect readback.
///
/// The publication owner compares this complete value to the command it
/// issued. An equal caller-computed digest or receipt reference is not enough.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEffectBinding {
    /// Original immutable publication operation.
    pub transaction_id: OpaqueId,
    /// Request identity from the retained original `OperationContext`.
    pub request_id: RequestId,
    /// Original finite relative deadline supplied with the request.
    pub relative_deadline_ms: u64,
    /// Original opaque budget reference supplied with the request.
    pub budget_ref: OpaqueRef,
    /// Stable phase-specific mutation identity; unknown outcomes never switch it.
    pub operation_id: OpaqueId,
    /// Full canonical admitted input identity.
    pub prepared_input_digest: Blake3Digest32,
    /// Complete owner-admitted namespace, source and projection scope.
    pub scope: crate::admission::PublicationScope,
    /// Original reserved target epoch.
    pub target_epoch: Epoch,
    /// Exact predecessor of the original reservation.
    pub previous_visible_epoch: Epoch,
    /// Actual consumed reservation floor at admission/restore.
    pub last_reserved_epoch: Epoch,
    /// Exact old manifest identity, absent only on initial publication.
    pub old_manifest_digest: Option<Blake3Digest32>,
    /// Complete exact old manifest, when present.
    pub old_manifest: Option<search_projection_planner::ProjectionManifest>,
    /// Exact new manifest identity (the canonical empty manifest for retirement).
    pub new_manifest_digest: Blake3Digest32,
    /// Complete exact new manifest, including every identity and vector digest.
    pub new_manifest: search_projection_planner::ProjectionManifest,
    /// Full immutable create/retain/retire diff.
    pub difference: search_projection_planner::ManifestDiff,
    /// Original owner guard set latched at admission.
    pub guards: PublicationGuards,
}

/// Complete untrusted point state returned by exact point readback.
#[derive(Clone, Debug)]
pub struct PublicationPointReadback {
    /// Exact compact point address.
    pub point_id: PointId128,
    /// Complete S11 key and full identity digest.
    pub identity: PointIdentity,
    /// Full typed payload, including validity bounds.
    pub payload: PointPayload,
    /// Full named vector contents, not caller-provided digests.
    pub vectors: std::collections::BTreeMap<String, VectorValue>,
}

impl PartialEq for PublicationPointReadback {
    fn eq(&self, other: &Self) -> bool {
        self.point_id == other.point_id
            && self.identity == other.identity
            && self.payload == other.payload
            && vector_maps_equal(&self.vectors, &other.vectors)
    }
}
impl Eq for PublicationPointReadback {}

/// Durable intent state observed through the control owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationIntentReadback {
    /// Complete original request binding persisted with the intent.
    pub binding: PublicationEffectBinding,
    /// Last durable phase checkpoint retained by the control owner.
    pub recorded_phase: crate::machine::PublicationPhase,
    /// Control-store generation that durably recorded this intent.
    pub control_generation: u64,
    /// Exact control record digest returned by the readback owner.
    pub control_state_digest: Blake3Digest32,
}

/// Current control state observed around the visibility compare-and-swap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationControlReadback {
    /// Visible epoch before this operation.
    pub before_visible_epoch: Epoch,
    /// Visible epoch after this operation.
    pub after_visible_epoch: Epoch,
    /// Actual consumed reservation floor.
    pub last_reserved_epoch: Epoch,
    /// Current owner guards from the same control readback.
    pub guards: PublicationGuards,
    /// Generation of the durable control record.
    pub control_generation: u64,
    /// Exact digest of the observed control record.
    pub control_state_digest: Blake3Digest32,
    /// Collection route selected by the same control record.
    pub collection_generation_id: CollectionGenerationId,
    /// Manifest selected before the compare-and-swap.
    pub previous_visible_manifest_digest: Option<Blake3Digest32>,
    /// Manifest identity selected by the same control record.
    pub visible_manifest_digest: Option<Blake3Digest32>,
}

/// Current-process immutable snapshot state observed after publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationSnapshotReadback {
    /// Opaque identity of the process snapshot instance.
    pub process_snapshot_id: OpaqueId,
    /// Snapshot visible epoch.
    pub visible_epoch: Epoch,
    /// Control generation represented by the snapshot.
    pub control_generation: u64,
    /// Full current owner guards represented by the snapshot.
    pub guards: PublicationGuards,
    /// Selected collection route represented by the snapshot.
    pub collection_generation_id: CollectionGenerationId,
    /// Current manifest identity represented by the snapshot.
    pub visible_manifest_digest: Option<Blake3Digest32>,
    /// Digest of the immutable snapshot content.
    pub snapshot_digest: Blake3Digest32,
}

/// Complete untrusted readback of an owner-established abandonment fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationAbandonReadback {
    /// Exact original publication binding and request identity.
    pub binding: PublicationEffectBinding,
    /// Complete affected point set excluded by the owner.
    pub excluded_point_ids: Vec<PointId128>,
    /// Complete affected projection membership set excluded by the owner.
    pub excluded_projection_memberships: Vec<search_contracts::ProjectionMembershipId>,
}

/// Full untrusted response from one independent effect/readback port call.
///
/// The port returns all relevant fields, not a receipt flag. The publication
/// owner validates the request binding and each complete point/control/snapshot
/// record before privately advancing its phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEffectReadback {
    /// Effect whose outcome was read back.
    pub effect: PublicationEffectKind,
    /// Exact original request binding echoed by the readback source.
    pub binding: PublicationEffectBinding,
    /// Owner-observed outcome; `Unknown` is never success or permission to replay.
    pub outcome: PublicationEffectOutcome,
    /// Complete exact point records for stage/closure readback.
    pub points: Vec<PublicationPointReadback>,
    /// Exact requested IDs that the owner readback proved absent.
    pub missing_point_ids: Vec<PointId128>,
    /// Point IDs returned outside the exact request set.
    pub unexpected_point_ids: Vec<PointId128>,
    /// Durable intent record for intent readback.
    pub intent: Option<PublicationIntentReadback>,
    /// Exact control-state observation for commit readback.
    pub control: Option<PublicationControlReadback>,
    /// Current-process snapshot observation for snapshot readback.
    pub snapshot: Option<PublicationSnapshotReadback>,
    /// Owner-established complete exclusion state for abandonment.
    pub abandonment: Option<PublicationAbandonReadback>,
    /// Exact abort-finalization command observed in the control journal.
    pub abort_request: Option<crate::machine::AbortFinalizationRequest>,
}

fn vector_maps_equal(
    left: &std::collections::BTreeMap<String, VectorValue>,
    right: &std::collections::BTreeMap<String, VectorValue>,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_name, left_value), (right_name, right_value))| {
                left_name == right_name && vector_values_equal(left_value, right_value)
            })
}

fn vector_values_equal(left: &VectorValue, right: &VectorValue) -> bool {
    let bits_equal = |a: &[f32], b: &[f32]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
    };
    match (left, right) {
        (VectorValue::Dense(a), VectorValue::Dense(b)) => bits_equal(a, b),
        (
            VectorValue::Sparse {
                indices: a_i,
                values: a_v,
            },
            VectorValue::Sparse {
                indices: b_i,
                values: b_v,
            },
        ) => a_i == b_i && bits_equal(a_v, b_v),
        _ => false,
    }
}

/// Exact stage acknowledgement and readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageReceipt {
    /// Matching transaction identity.
    pub(crate) transaction_id: OpaqueId,
    /// Reserved target epoch.
    pub(crate) target_epoch: Epoch,
    /// Complete exact point records returned by the owner readback port.
    pub(crate) points: Vec<PublicationPointReadback>,
}

/// Exact old-point closure acknowledgement and readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureReceipt {
    /// Matching transaction identity.
    pub(crate) transaction_id: OpaqueId,
    /// Reserved target epoch used as exclusive upper bound.
    pub(crate) target_epoch: Epoch,
    /// Complete exact point records returned by the owner readback port.
    pub(crate) points: Vec<PublicationPointReadback>,
}

/// Exact combined verification after staging and closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadbackVerified {
    /// Matching transaction identity.
    pub(crate) transaction_id: OpaqueId,
    /// Reserved target epoch.
    pub(crate) target_epoch: Epoch,
    /// Exact newly visible manifest digest.
    pub(crate) new_manifest_digest: Blake3Digest32,
    /// Exact retired manifest digest, when any points retire.
    pub(crate) retired_manifest_digest: Option<Blake3Digest32>,
}

/// Linearization-point receipt for a visible publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleCommitReceipt {
    /// Matching transaction identity.
    pub(crate) transaction_id: OpaqueId,
    /// Newly visible epoch.
    pub(crate) visible_epoch: Epoch,
    /// Exact visible manifest digest.
    pub(crate) visible_manifest_digest: Blake3Digest32,
    /// Retired manifest digest, when present.
    pub(crate) retired_manifest_digest: Option<Blake3Digest32>,
    /// New control generation.
    pub(crate) control_generation: u64,
    /// Digest of exact committed control state.
    pub(crate) control_state_digest: Blake3Digest32,
}

/// Immutable in-memory control snapshot publication receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPublishReceipt {
    /// Matching transaction identity.
    pub(crate) transaction_id: OpaqueId,
    /// Published visible epoch.
    pub(crate) visible_epoch: Epoch,
    /// Published control generation.
    pub(crate) control_generation: u64,
    /// Exact current-process snapshot instance read back from the owner port.
    pub(crate) process_snapshot_id: OpaqueId,
    /// Digest of the immutable snapshot.
    pub(crate) snapshot_digest: Blake3Digest32,
}

/// Exact committed retired-point manifest emitted for ordinary reclaim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredManifest {
    /// Collection generation containing retired points.
    pub(crate) collection_generation_id: CollectionGenerationId,
    /// First epoch at which those points are invisible.
    pub(crate) retirement_epoch_exclusive: Epoch,
    /// Canonically ordered exact point IDs.
    pub(crate) point_ids: Vec<PointId128>,
    /// Digest of the exact retired-ID manifest.
    pub(crate) manifest_digest: Blake3Digest32,
    /// Matching visible commit receipt reference.
    pub(crate) publication_receipt: ReceiptRef,
}

impl RetiredManifest {
    /// Collection generation containing the retired points.
    #[must_use]
    pub const fn collection_generation_id(&self) -> CollectionGenerationId {
        self.collection_generation_id
    }
    /// First epoch at which the points are invisible.
    #[must_use]
    pub const fn retirement_epoch_exclusive(&self) -> Epoch {
        self.retirement_epoch_exclusive
    }
    /// Full canonical point IDs from the admitted retirement diff.
    #[must_use]
    pub fn point_ids(&self) -> &[PointId128] {
        &self.point_ids
    }
    /// Exact old manifest identity.
    #[must_use]
    pub const fn manifest_digest(&self) -> Blake3Digest32 {
        self.manifest_digest
    }
    /// Opaque correlation reference from the private visible-commit proof.
    #[must_use]
    pub const fn publication_receipt(&self) -> &ReceiptRef {
        &self.publication_receipt
    }
}

/// Exact compensation plan derived from both sides of the immutable manifest diff.
///
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
