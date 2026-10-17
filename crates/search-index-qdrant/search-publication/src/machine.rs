//! Single-flight publication coordinator and linearization state machine.

use std::fmt::Write as _;

mod abort;
mod restore;
pub use abort::{
    AbortControlCommitObservation, AbortFinalizationRequest, AbortedPublicationResolution,
};
pub use restore::PublicationRestoreInput;

use crate::admission::{
    AdmissionPurpose, ProjectionEvidence, ProjectionUpdate, PublicationAdmissionOwners,
    PublicationProposal, PublicationScope, validate_prepared_generation,
};
use search_contracts::indexed::{PointPayload, VectorValue};
use search_contracts::{Blake3Digest32, Epoch, OpaqueId, OpaqueRef, ReceiptRef, RequestId};
use search_point_identity::s11::PointId128;
use search_ports::{CancellationProbe, IdempotencyClass, MutationIdentity, OperationContext};
use search_projection_planner::{
    ManifestDiff, PlannedUnit, ProjectionBudget, ProjectionManifest, plan_projection,
};

use crate::model::PreparedPointSets;
use crate::{
    ClosureReceipt, CompensationPlan, PreparedGeneration, PublicationEffectBinding,
    PublicationEffectKind, PublicationEffectOutcome, PublicationEffectReadback, PublicationError,
    PublicationGuards, PublicationPointExpectation, PublicationPointReadback, ReadbackVerified,
    RetiredManifest, SnapshotPublishReceipt, StageReceipt, VisibleCommitReceipt,
};

/// Maximum exact points in one publication transaction.
pub const DEFAULT_MAX_PUBLICATION_POINTS: usize = 1_000_000;

/// Closed publication transaction phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPhase {
    Prepared,
    IntentDurable,
    NewPointsAcknowledged,
    OldPointsClosedAcknowledged,
    ReadbackVerified,
    ControlCommitted,
    SnapshotPublished,
    Compensating,
    Aborted,
    PublicationBlocked,
}

/// Durable intent record prepared for an external control journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableIntent {
    pub(crate) transaction_id: OpaqueId,
    pub(crate) target_epoch: Epoch,
    pub(crate) old_manifest_digest: Option<Blake3Digest32>,
    pub(crate) new_manifest_digest: Blake3Digest32,
    pub(crate) guards: PublicationGuards,
    pub(crate) persist_operation_id: OpaqueId,
    pub(crate) request_id: RequestId,
    pub(crate) control_generation: u64,
    pub(crate) control_state_digest: Blake3Digest32,
}

impl DurableIntent {
    /// Original immutable transaction identity.
    #[must_use]
    pub const fn transaction_id(&self) -> &OpaqueId {
        &self.transaction_id
    }
    /// Exact complete guard set read back from the durable intent owner.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }
    /// Stable original mutation identity for this intent.
    #[must_use]
    pub const fn persist_operation_id(&self) -> &OpaqueId {
        &self.persist_operation_id
    }
    /// Request identity retained by the original operation context.
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }
    /// Reserved epoch retained by the intent.
    #[must_use]
    pub const fn target_epoch(&self) -> Epoch {
        self.target_epoch
    }
    /// Original full frozen input identity.
    #[must_use]
    pub const fn new_manifest_digest(&self) -> Blake3Digest32 {
        self.new_manifest_digest
    }
    /// Exact prior manifest identity, when present.
    #[must_use]
    pub const fn old_manifest_digest(&self) -> Option<Blake3Digest32> {
        self.old_manifest_digest
    }
    /// Stable owner generation that recorded this intent.
    #[must_use]
    pub const fn control_generation(&self) -> u64 {
        self.control_generation
    }
    /// Exact control record commitment retained with the intent.
    #[must_use]
    pub const fn control_state_digest(&self) -> Blake3Digest32 {
        self.control_state_digest
    }
}

/// Complete in-flight publication transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationTransaction {
    pub(crate) prepared: PreparedGeneration,
    /// The actual visible epoch before reservation, which may precede skipped epochs.
    pub(crate) previous_visible_epoch: Epoch,
    pub(crate) target_epoch: Epoch,
    pub(crate) phase: PublicationPhase,
    pub(crate) durable_intent: Option<DurableIntent>,
    pub(crate) stage_receipt: Option<StageReceipt>,
    pub(crate) closure_receipt: Option<ClosureReceipt>,
    pub(crate) verified: Option<ReadbackVerified>,
    pub(crate) visible_commit: Option<VisibleCommitReceipt>,
    pub(crate) snapshot_receipt: Option<SnapshotPublishReceipt>,
    expected_points: Option<PreparedPointSets>,
    pub(crate) pending_effect: Option<PublicationEffectKind>,
    pub(crate) original_context: Option<OperationContextBinding>,
    // Retain the caller's finite bound for recovery of this exact transaction.
    pub(crate) max_points: usize,
}

impl PublicationTransaction {
    #[must_use]
    pub const fn transaction_id(&self) -> &OpaqueId {
        &self.prepared.transaction_id
    }

    /// Exact phase retained by the owner.
    #[must_use]
    pub const fn phase(&self) -> PublicationPhase {
        self.phase
    }
    /// Previously visible epoch before this reservation.
    #[must_use]
    pub const fn previous_visible_epoch(&self) -> Epoch {
        self.previous_visible_epoch
    }
    /// Exact reserved epoch for this transaction.
    #[must_use]
    pub const fn target_epoch(&self) -> Epoch {
        self.target_epoch
    }
    /// Canonically admitted immutable inputs.
    #[must_use]
    pub const fn prepared(&self) -> &PreparedGeneration {
        &self.prepared
    }
    /// Read-only durable intent accepted by the owner.
    #[must_use]
    pub const fn durable_intent(&self) -> Option<&DurableIntent> {
        self.durable_intent.as_ref()
    }
    /// Read-only privately validated stage observation.
    #[must_use]
    pub const fn stage_receipt(&self) -> Option<&StageReceipt> {
        self.stage_receipt.as_ref()
    }
    /// Read-only privately validated closure observation.
    #[must_use]
    pub const fn closure_receipt(&self) -> Option<&ClosureReceipt> {
        self.closure_receipt.as_ref()
    }
    /// Read-only private combined point verification.
    #[must_use]
    pub const fn verified(&self) -> Option<&ReadbackVerified> {
        self.verified.as_ref()
    }
    /// Read-only private visibility commit proof.
    #[must_use]
    pub const fn visible_commit(&self) -> Option<&VisibleCommitReceipt> {
        self.visible_commit.as_ref()
    }
    /// Read-only current-process snapshot acknowledgement.
    #[must_use]
    pub const fn snapshot_receipt(&self) -> Option<&SnapshotPublishReceipt> {
        self.snapshot_receipt.as_ref()
    }
}

/// Bounded effect command assembled only from the canonical admitted transaction.
/// Its private fields prevent replacing scope, diff, expected point state or phase identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEffectCommand {
    effect: PublicationEffectKind,
    binding: PublicationEffectBinding,
    current_guards: PublicationGuards,
    points: Vec<PublicationPointExpectation>,
    original_points: Vec<PublicationPointExpectation>,
    required_absent_point_ids: Vec<PointId128>,
    abort_request: Option<AbortFinalizationRequest>,
}

impl PublicationEffectCommand {
    /// Requested phase effect.
    #[must_use]
    pub const fn effect(&self) -> PublicationEffectKind {
        self.effect
    }
    /// Exact immutable request binding.
    #[must_use]
    pub const fn binding(&self) -> &PublicationEffectBinding {
        &self.binding
    }
    /// Fresh owner guards observed with the original request.
    #[must_use]
    pub const fn current_guards(&self) -> PublicationGuards {
        self.current_guards
    }
    /// Exact affected point expectations; stage and closure carry full state.
    #[must_use]
    pub fn points(&self) -> &[PublicationPointExpectation] {
        &self.points
    }
    /// Exact pre-effect point state for conditional closure operations.
    #[must_use]
    pub fn original_points(&self) -> &[PublicationPointExpectation] {
        &self.original_points
    }
    /// Exact IDs whose absence is required by a compensation readback.
    #[must_use]
    pub fn required_absent_point_ids(&self) -> &[PointId128] {
        &self.required_absent_point_ids
    }
    /// Latched abort request when this is an abort-finalization effect.
    #[must_use]
    pub const fn abort_request(&self) -> Option<&AbortFinalizationRequest> {
        self.abort_request.as_ref()
    }
}

/// Result of one mutation attempt. The independent readback, not this value,
/// determines whether the machine advances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationMutationAttempt {
    /// Transport acknowledged the request.
    Acknowledged,
    /// Transport rejected before it could apply the request.
    Rejected,
    /// Transport cannot determine whether the effect applied.
    Unknown,
}

/// Owner effect seam. Implementations receive the original operation context
/// and the exact phase command derived from the admitted transaction.
pub trait PublicationEffectPort<C: CancellationProbe> {
    /// Attempts one effect with the command's stable operation ID.
    fn apply(
        &mut self,
        command: &PublicationEffectCommand,
        identity: &MutationIdentity,
        context: &OperationContext<C>,
    ) -> PublicationMutationAttempt;
}

/// Independent exact readback seam. Observations are untrusted; the coordinator
/// compares every field to its admitted command before minting private proof.
pub trait PublicationReadbackPort<C: CancellationProbe> {
    /// Reads the exact outcome for the original operation, never by collection scan/count.
    fn readback(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> PublicationEffectReadback;
}

/// Additional immutable products required to stage and verify actual point content.
/// This seam does not admit a generation or compute a second manifest diff.
pub trait PublicationEffectPreparationOwners: PublicationAdmissionOwners {
    /// Raw planned vectors from the same private projection token used by admission.
    fn planned_vectors<'a>(
        &self,
        token: &'a Self::Projection,
    ) -> Result<Option<&'a [PlannedUnit]>, PublicationError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationContextBinding {
    request_id: RequestId,
    relative_deadline_ms: u64,
    budget_ref: OpaqueRef,
}

impl OperationContextBinding {
    fn from_context<C: CancellationProbe>(context: &OperationContext<C>) -> Self {
        Self {
            request_id: context.request_id(),
            relative_deadline_ms: context.relative_deadline_ms().get(),
            budget_ref: context.budget_ref().clone(),
        }
    }
}

fn capture_point_expectations(
    evidence: &ProjectionEvidence<'_>,
    planned: &[PlannedUnit],
    budget: ProjectionBudget,
) -> Result<Vec<PublicationPointExpectation>, PublicationError> {
    if planned.len() != evidence.plan.points.len() || planned.len() > budget.max_points {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    let requirements = evidence
        .schema
        .vectors()
        .iter()
        .map(|requirement| {
            (
                requirement.name().as_str().to_owned(),
                requirement.requirement(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let rebuilt = plan_projection(
        evidence.units,
        evidence.planning_scope,
        planned,
        &requirements,
        budget,
    )
    .map_err(|_| PublicationError::InvalidPreparedPublication)?;
    if rebuilt != *evidence.plan {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    let by_unit = planned
        .iter()
        .map(|unit| (unit.unit_id, unit))
        .collect::<std::collections::BTreeMap<_, _>>();
    if by_unit.len() != planned.len() {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    evidence
        .plan
        .points
        .iter()
        .map(|point| {
            let unit = by_unit
                .get(&point.identity.key().unit_id)
                .ok_or(PublicationError::InvalidPreparedPublication)?;
            let payload =
                PointPayload::from_canonical_cbor(point.expected_readback.payload_cbor.as_slice())
                    .map_err(|_| PublicationError::InvalidPreparedPublication)?;
            if payload
                .to_canonical_cbor()
                .map_err(|_| PublicationError::InvalidPreparedPublication)?
                != point.expected_readback.payload_cbor
                || payload
                    .identity()
                    .map_err(|_| PublicationError::InvalidPreparedPublication)?
                    != point.expected_readback.payload_identity
            {
                return Err(PublicationError::InvalidPreparedPublication);
            }
            let vectors = unit
                .vectors
                .iter()
                .map(|vector| (vector.name.as_str().to_owned(), vector.value.clone()))
                .collect::<std::collections::BTreeMap<String, VectorValue>>();
            if vectors.len() != unit.vectors.len()
                || vectors
                    .keys()
                    .ne(point.expected_readback.vector_digests.keys())
            {
                return Err(PublicationError::InvalidPreparedPublication);
            }
            Ok(PublicationPointExpectation {
                point_id: point.point_id,
                identity: point.identity.clone(),
                payload,
                vectors,
            })
        })
        .collect()
}

fn expected_point_sets<P: PublicationEffectPreparationOwners + ?Sized>(
    owners: &P,
    proposal: &PublicationProposal<'_, P::Projection>,
    prepared: &PreparedGeneration,
    budget: ProjectionBudget,
) -> Result<PreparedPointSets, PublicationError> {
    let old = match proposal.old {
        Some(token) => {
            let evidence = owners.projection(token)?;
            if prepared.old_manifest.as_ref() != Some(&evidence.plan.manifest) {
                return Err(PublicationError::InvalidPreparedPublication);
            }
            let vectors = owners
                .planned_vectors(token)?
                .ok_or(PublicationError::InvalidPreparedPublication)?;
            capture_point_expectations(&evidence, vectors, budget)?
        }
        None => Vec::new(),
    };
    let new = match &proposal.update {
        ProjectionUpdate::Publish(token) => {
            let evidence = owners.projection(token)?;
            if prepared.new_manifest != evidence.plan.manifest {
                return Err(PublicationError::InvalidPreparedPublication);
            }
            let vectors = owners
                .planned_vectors(token)?
                .ok_or(PublicationError::InvalidPreparedPublication)?;
            capture_point_expectations(&evidence, vectors, budget)?
        }
        ProjectionUpdate::Retirement => {
            if !prepared.new_manifest.entries.is_empty() {
                return Err(PublicationError::InvalidPreparedPublication);
            }
            Vec::new()
        }
    };
    Ok(PreparedPointSets { old, new })
}

/// Process-local single-flight coordinator. Deliberately not cloneable.
///
/// Its epoch floor is reconstructed from authoritative control history by the
/// composition owner. It is not another persistent ledger or ownership lease.
#[derive(Debug)]
pub struct PublicationCoordinator {
    initialized: bool,
    current_scope: Option<PublicationScope>,
    visible_epoch: Epoch,
    last_reserved_epoch: Epoch,
    current_manifest: Option<ProjectionManifest>,
    current_manifest_digest: Option<Blake3Digest32>,
    active: Option<PublicationTransaction>,
    abort_finalization: abort::AbortFinalizationProgress,
    max_points: usize,
}

impl PublicationCoordinator {
    /// Creates an uninitialized transient coordinator with finite capacity only.
    /// No caller epochs, manifests or digests become accepted state here.
    /// `submit` obtains its initial state only after complete owner admission.
    pub fn new(max_points: usize) -> Result<Self, PublicationError> {
        if max_points == 0 {
            return Err(PublicationError::InvalidPreparedPublication);
        }
        Ok(Self {
            initialized: false,
            current_scope: None,
            visible_epoch: Epoch::new(0).map_err(|_| PublicationError::ContractExhausted)?,
            last_reserved_epoch: Epoch::new(0).map_err(|_| PublicationError::ContractExhausted)?,
            current_manifest: None,
            current_manifest_digest: None,
            active: None,
            abort_finalization: abort::AbortFinalizationProgress::default(),
            max_points,
        })
    }

    #[must_use]
    pub const fn visible_epoch(&self) -> Option<Epoch> {
        if self.initialized {
            Some(self.visible_epoch)
        } else {
            None
        }
    }

    /// Highest consumed reservation in this coordinator, including aborted work.
    #[must_use]
    pub const fn last_reserved_epoch(&self) -> Option<Epoch> {
        if self.initialized {
            Some(self.last_reserved_epoch)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn active(&self) -> Option<&PublicationTransaction> {
        self.active.as_ref()
    }

    #[must_use]
    pub const fn current_manifest(&self) -> Option<&ProjectionManifest> {
        self.current_manifest.as_ref()
    }

    /// Reserves the next unused epoch for one prepared transaction.
    /// A reservation is never returned to the pool, even after compensation.
    /// A physical point ID shared with the current manifest must retain its full
    /// immutable entry. Changed entries need distinct identities from the point
    /// owner; the coordinator never invents replacement IDs or mutates manifests.
    pub fn submit<P: PublicationEffectPreparationOwners + ?Sized>(
        &mut self,
        proposal: &PublicationProposal<'_, P::Projection>,
        owners: &mut P,
        snapshot: &P::Snapshot,
        budget: ProjectionBudget,
    ) -> Result<Epoch, PublicationError> {
        self.submit_with_effect_preparation(proposal, owners, snapshot, budget)
    }

    /// Canonically admits one proposal and retains the full C13 payload/vector
    /// products required by effect readback. This uses the same #260 validator
    /// and diff as `submit`; missing vector products fail before slot creation.
    pub fn submit_with_effect_preparation<P: PublicationEffectPreparationOwners + ?Sized>(
        &mut self,
        proposal: &PublicationProposal<'_, P::Projection>,
        owners: &mut P,
        snapshot: &P::Snapshot,
        mut budget: ProjectionBudget,
    ) -> Result<Epoch, PublicationError> {
        if self.active.is_some() {
            return Err(PublicationError::PublicationBusy);
        }
        budget.max_points = budget.max_points.min(self.max_points);
        let prepared = validate_prepared_generation(
            owners,
            snapshot,
            proposal,
            AdmissionPurpose::Prepare,
            budget,
        )?;
        let expected_points = expected_point_sets(owners, proposal, &prepared, budget)?;
        self.install_prepared(prepared, Some(expected_points))
    }

    fn install_prepared(
        &mut self,
        prepared: PreparedGeneration,
        expected_points: Option<PreparedPointSets>,
    ) -> Result<Epoch, PublicationError> {
        if self.active.is_some() {
            return Err(PublicationError::PublicationBusy);
        }
        let observed = prepared.observed;
        let observed_visible = observed.previous_visible_epoch;
        if self.initialized
            && (observed_visible < self.visible_epoch
                || observed.last_reserved_epoch < self.last_reserved_epoch
                || self.current_scope.as_ref().is_some_and(|scope| {
                    scope.collection_generation_id != prepared.scope().collection_generation_id
                }))
        {
            return Err(PublicationError::ControlConflict);
        }
        if self.initialized
            && self.current_scope.as_ref().is_some_and(|scope| {
                scope.projection_membership_id == prepared.scope().projection_membership_id
            })
            && (prepared.old_manifest != self.current_manifest
                || prepared.old_manifest_digest != self.current_manifest_digest)
        {
            return Err(PublicationError::InvalidPreparedPublication);
        }
        let target_epoch = observed.target_epoch;
        self.initialized = true;
        self.current_scope = Some(prepared.scope().clone());
        self.visible_epoch = observed.previous_visible_epoch;
        self.current_manifest.clone_from(&prepared.old_manifest);
        self.current_manifest_digest = prepared.old_manifest_digest;
        self.active = Some(PublicationTransaction {
            prepared,
            previous_visible_epoch: self.visible_epoch,
            target_epoch,
            phase: PublicationPhase::Prepared,
            durable_intent: None,
            stage_receipt: None,
            closure_receipt: None,
            verified: None,
            visible_commit: None,
            snapshot_receipt: None,
            expected_points,
            pending_effect: None,
            original_context: None,
            max_points: self.max_points,
        });
        self.last_reserved_epoch = target_epoch;
        Ok(target_epoch)
    }

    /// Persists and independently reads back the exact original intent.
    pub fn persist_intent<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<DurableIntent, PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::Prepared)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::PersistIntent,
        )?;
        let intent = observed.intent.ok_or(PublicationError::RecoveryBlocked)?;
        if intent.binding != command.binding
            || intent.control_generation == 0
            || intent.recorded_phase != PublicationPhase::IntentDurable
            || !observed.points.is_empty()
            || !observed.missing_point_ids.is_empty()
            || !observed.unexpected_point_ids.is_empty()
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::OperationMismatch);
        }
        transaction.pending_effect = None;
        let durable = DurableIntent {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            old_manifest_digest: transaction.prepared.old_manifest_digest,
            new_manifest_digest: transaction.prepared.new_manifest_digest,
            guards: transaction.prepared.guards,
            persist_operation_id: command.binding.operation_id,
            request_id: command.binding.request_id,
            control_generation: intent.control_generation,
            control_state_digest: intent.control_state_digest,
        };
        transaction.durable_intent = Some(durable.clone());
        transaction.phase = PublicationPhase::IntentDurable;
        Ok(durable)
    }

    /// Stages exact full point content and advances only after independent readback.
    pub fn stage_new_points<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::IntentDurable)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::StageNewPoints,
        )?;
        if !observed.missing_point_ids.is_empty()
            || !observed.unexpected_point_ids.is_empty()
            || !point_readback_matches(&command.points, &observed.points)
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::StageReadbackMismatch);
        }
        transaction.pending_effect = None;
        transaction.stage_receipt = Some(StageReceipt {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            points: observed.points,
        });
        transaction.phase = PublicationPhase::NewPointsAcknowledged;
        Ok(())
    }

    /// Closes exact old points while preserving every original field and vector.
    pub fn close_old_points<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::NewPointsAcknowledged)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::CloseOldPoints,
        )?;
        if !observed.missing_point_ids.is_empty()
            || !observed.unexpected_point_ids.is_empty()
            || !point_readback_matches(&command.points, &observed.points)
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        transaction.pending_effect = None;
        transaction.closure_receipt = Some(ClosureReceipt {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            points: observed.points,
        });
        transaction.phase = PublicationPhase::OldPointsClosedAcknowledged;
        Ok(())
    }

    /// Re-reads all changed points and privately mints the combined proof.
    pub fn verify_readback<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<ReadbackVerified, PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::OldPointsClosedAcknowledged)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::VerifyReadback,
        )?;
        if !observed.missing_point_ids.is_empty()
            || !observed.unexpected_point_ids.is_empty()
            || !point_readback_matches(&command.points, &observed.points)
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::StageReadbackMismatch);
        }
        let retired_manifest_digest = if manifest_diff(transaction).retire.is_empty() {
            None
        } else {
            transaction.prepared.old_manifest_digest
        };
        if !manifest_diff(transaction).retire.is_empty() && retired_manifest_digest.is_none() {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        transaction.pending_effect = None;
        let verified = ReadbackVerified {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            new_manifest_digest: transaction.prepared.new_manifest_digest,
            retired_manifest_digest,
        };
        transaction.verified = Some(verified.clone());
        transaction.phase = PublicationPhase::ReadbackVerified;
        Ok(verified)
    }

    /// Executes one guarded visibility CAS and verifies the exact control record.
    pub fn commit_visible_epoch<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<VisibleCommitReceipt, PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::ReadbackVerified)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::CommitVisibleEpoch,
        )?;
        let Some(control) = observed.control else {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::ControlConflict);
        };
        let intent_generation = transaction
            .durable_intent
            .as_ref()
            .ok_or(PublicationError::ControlConflict)?
            .control_generation;
        let expected_manifest = if matches!(
            transaction.prepared.target,
            crate::admission::PublicationTarget::Retirement(_)
        ) {
            None
        } else {
            Some(transaction.prepared.new_manifest_digest)
        };
        if control.before_visible_epoch != transaction.previous_visible_epoch
            || control.after_visible_epoch != transaction.target_epoch
            || control.last_reserved_epoch != transaction.target_epoch
            || control.guards != transaction.prepared.guards
            || control.collection_generation_id != transaction.prepared.collection_generation_id
            || control.previous_visible_manifest_digest != transaction.prepared.old_manifest_digest
            || control.visible_manifest_digest != expected_manifest
            || intent_generation.checked_add(1) != Some(control.control_generation)
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::ControlConflict);
        }
        if observed.binding != command.binding {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::OperationMismatch);
        }
        let verified = transaction
            .verified
            .as_ref()
            .ok_or(PublicationError::StageReadbackMismatch)?;
        transaction.pending_effect = None;
        let receipt = VisibleCommitReceipt {
            transaction_id: transaction.prepared.transaction_id.clone(),
            visible_epoch: transaction.target_epoch,
            visible_manifest_digest: transaction.prepared.new_manifest_digest,
            retired_manifest_digest: verified.retired_manifest_digest,
            control_generation: control.control_generation,
            control_state_digest: control.control_state_digest,
        };
        transaction.visible_commit = Some(receipt.clone());
        transaction.phase = PublicationPhase::ControlCommitted;
        self.visible_epoch = control.after_visible_epoch;
        Ok(receipt)
    }

    /// Publishes and reads back this process's exact immutable snapshot.
    pub fn publish_control_snapshot<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let transaction = self.active_mut(PublicationPhase::ControlCommitted)?;
        let (command, observed) = run_effect(
            transaction,
            effects,
            readback,
            context,
            PublicationEffectKind::PublishControlSnapshot,
        )?;
        let Some(snapshot) = observed.snapshot else {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::SnapshotPublicationFailed);
        };
        let commit = transaction
            .visible_commit
            .as_ref()
            .ok_or(PublicationError::ControlConflict)?;
        let expected_manifest = if matches!(
            transaction.prepared.target,
            crate::admission::PublicationTarget::Retirement(_)
        ) {
            None
        } else {
            Some(transaction.prepared.new_manifest_digest)
        };
        if observed.binding != command.binding
            || snapshot.visible_epoch != commit.visible_epoch
            || snapshot.control_generation != commit.control_generation
            || snapshot.guards != transaction.prepared.guards
            || snapshot.collection_generation_id != transaction.prepared.collection_generation_id
            || snapshot.visible_manifest_digest != expected_manifest
        {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::SnapshotPublicationFailed);
        }
        transaction.pending_effect = None;
        transaction.snapshot_receipt = Some(SnapshotPublishReceipt {
            transaction_id: transaction.prepared.transaction_id.clone(),
            visible_epoch: snapshot.visible_epoch,
            control_generation: snapshot.control_generation,
            process_snapshot_id: snapshot.process_snapshot_id,
            snapshot_digest: snapshot.snapshot_digest,
        });
        transaction.phase = PublicationPhase::SnapshotPublished;
        Ok(())
    }

    /// Emits exact retired IDs only after visible control and snapshot commit.
    pub fn emit_retired_manifest(&self) -> Result<Option<RetiredManifest>, PublicationError> {
        let transaction = self.active_at(PublicationPhase::SnapshotPublished)?;
        let retired = &manifest_diff(transaction).retire;
        if retired.is_empty() {
            return Ok(None);
        }
        let verified = transaction
            .verified
            .as_ref()
            .ok_or(PublicationError::ClosureReadbackMismatch)?;
        let manifest_digest = transaction
            .prepared
            .old_manifest_digest
            .ok_or(PublicationError::ClosureReadbackMismatch)?;
        if verified.retired_manifest_digest != Some(manifest_digest) {
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        let control_generation = transaction
            .visible_commit
            .as_ref()
            .ok_or(PublicationError::ControlConflict)?
            .control_generation;
        let publication_receipt = ReceiptRef::new(format!("publication:{control_generation}"))
            .map_err(|_| PublicationError::OperationMismatch)?;
        Ok(Some(RetiredManifest {
            collection_generation_id: transaction.prepared.collection_generation_id,
            retirement_epoch_exclusive: transaction.target_epoch,
            point_ids: entry_ids(retired),
            manifest_digest,
            publication_receipt,
        }))
    }

    /// Reconcile the active phase through exact owner readback only. The
    /// original mutation IDs are retained; this method never replays effects.
    pub fn recover<C, R>(
        &mut self,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<crate::PublicationRecoveryDecision, PublicationError>
    where
        C: CancellationProbe,
        R: PublicationReadbackPort<C>,
    {
        let (decision, visible_epoch) = {
            let transaction = self
                .active
                .as_mut()
                .ok_or(PublicationError::InvalidTransition)?;
            crate::recovery::recover_active(transaction, readback, context)?
        };
        if let Some(epoch) = visible_epoch {
            self.visible_epoch = epoch;
        }
        Ok(decision)
    }

    /// Completes the transaction and installs the new current manifest.
    pub fn complete(&mut self) -> Result<VisibleCommitReceipt, PublicationError> {
        // Validate before taking the sole active slot: an error must not release
        // serialization or discard the transaction needed by recovery.
        let receipt = self
            .active_at(PublicationPhase::SnapshotPublished)?
            .visible_commit
            .clone()
            .ok_or(PublicationError::ControlConflict)?;
        let transaction = self
            .active
            .take()
            .ok_or(PublicationError::InvalidTransition)?;
        if matches!(
            transaction.prepared.target,
            crate::admission::PublicationTarget::Retirement(_)
        ) {
            self.current_manifest = None;
            self.current_manifest_digest = None;
        } else {
            self.current_manifest = Some(transaction.prepared.new_manifest);
            self.current_manifest_digest = Some(transaction.prepared.new_manifest_digest);
        }
        Ok(receipt)
    }

    /// Begins exact compensation and returns both mutation directions.
    /// Retry while COMPENSATING returns the same complete plan.
    pub fn begin_compensation_plan(&mut self) -> Result<CompensationPlan, PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction
            .pending_effect
            .is_some_and(|pending| pending != PublicationEffectKind::Compensate)
        {
            return Err(PublicationError::OutcomeUnknown);
        }
        if !matches!(
            transaction.phase,
            PublicationPhase::IntentDurable
                | PublicationPhase::NewPointsAcknowledged
                | PublicationPhase::OldPointsClosedAcknowledged
                | PublicationPhase::ReadbackVerified
                | PublicationPhase::Compensating
        ) {
            return Err(PublicationError::InvalidTransition);
        }
        let difference = manifest_diff(transaction);
        let plan = CompensationPlan {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            staged_ids: entry_ids(&difference.create),
            closed_ids: entry_ids(&difference.retire),
        };
        transaction.phase = PublicationPhase::Compensating;
        Ok(plan)
    }

    /// Compatibility projection for create-only callers. With retired points,
    /// `compensate_exact` refuses completion; use the full plan and restoration.
    pub fn begin_compensation(&mut self) -> Result<Vec<PointId128>, PublicationError> {
        Ok(self.begin_compensation_plan()?.staged_ids)
    }

    /// Removes staged points and restores exact original old point state through
    /// an owner effect port. The complete readback, not receipt IDs, completes it.
    pub fn compensate_exact<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        self.compensate_with_ports(effects, readback, context, false)
    }

    /// Two-sided compensation for replacement: remove new IDs and restore every
    /// retired point's original payload, vectors and validity interval.
    pub fn compensate_and_restore<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        self.compensate_with_ports(effects, readback, context, true)
    }

    fn compensate_with_ports<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
        restore_old: bool,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let resolution = {
            let transaction = self.active_mut(PublicationPhase::Compensating)?;
            if !restore_old && !manifest_diff(transaction).retire.is_empty() {
                return Err(PublicationError::CompensationIncomplete);
            }
            let (command, observed) = run_effect(
                transaction,
                effects,
                readback,
                context,
                PublicationEffectKind::Compensate,
            )
            .map_err(|error| {
                if error == PublicationError::EffectNotObserved {
                    PublicationError::CompensationIncomplete
                } else {
                    error
                }
            })?;
            if observed.outcome != PublicationEffectOutcome::Applied
                || observed.missing_point_ids != command.required_absent_point_ids
                || !observed.unexpected_point_ids.is_empty()
                || !point_readback_matches(command.points(), &observed.points)
            {
                return Err(PublicationError::CompensationIncomplete);
            }
            transaction.pending_effect = None;
            transaction.phase = PublicationPhase::Aborted;
            AbortedPublicationResolution::Compensated {
                operation_id: command.binding.operation_id,
                restored_old_points: !manifest_diff(transaction).retire.is_empty(),
            }
        };
        self.abort_finalization.resolution = Some(resolution);
        Ok(())
    }

    /// Applies and reads back a complete membership-wide exclusion fence.
    /// Point-only or caller-constructed receipts cannot release the transaction.
    pub fn abandon<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        let resolution = {
            let transaction = self
                .active
                .as_mut()
                .ok_or(PublicationError::InvalidTransition)?;
            if !matches!(
                transaction.phase,
                PublicationPhase::IntentDurable
                    | PublicationPhase::NewPointsAcknowledged
                    | PublicationPhase::OldPointsClosedAcknowledged
                    | PublicationPhase::ReadbackVerified
                    | PublicationPhase::Compensating
            ) {
                return Err(PublicationError::InvalidTransition);
            }
            let (command, observed) = run_effect(
                transaction,
                effects,
                readback,
                context,
                PublicationEffectKind::Abandon,
            )?;
            let fence = observed
                .abandonment
                .ok_or(PublicationError::AbandonFenceMissing)?;
            let difference = manifest_diff(transaction);
            let affected = difference.create.iter().chain(&difference.retire);
            let mut point_ids = affected
                .clone()
                .map(|entry| entry.point_id)
                .collect::<Vec<_>>();
            point_ids.sort_unstable();
            let mut memberships = affected
                .map(|entry| entry.projection_membership_id)
                .collect::<Vec<_>>();
            memberships.sort_unstable();
            memberships.dedup();
            if observed.outcome != PublicationEffectOutcome::Applied
                || fence.binding != command.binding
                || fence.excluded_point_ids != point_ids
                || fence.excluded_projection_memberships != memberships
                || !observed.points.is_empty()
                || !observed.missing_point_ids.is_empty()
                || !observed.unexpected_point_ids.is_empty()
            {
                return Err(PublicationError::AbandonFenceMissing);
            }
            transaction.pending_effect = None;
            transaction.phase = PublicationPhase::Aborted;
            AbortedPublicationResolution::Excluded {
                operation_id: command.binding.operation_id,
                excluded_projection_memberships: memberships,
            }
        };
        self.abort_finalization.resolution = Some(resolution);
        Ok(())
    }

    /// Blocks later publication until explicit repair.
    pub fn block_publication(&mut self) -> Result<(), PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        transaction.phase = PublicationPhase::PublicationBlocked;
        Ok(())
    }

    fn active_mut(
        &mut self,
        expected: PublicationPhase,
    ) -> Result<&mut PublicationTransaction, PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != expected {
            return Err(PublicationError::InvalidTransition);
        }
        Ok(transaction)
    }

    fn active_at(
        &self,
        expected: PublicationPhase,
    ) -> Result<&PublicationTransaction, PublicationError> {
        let transaction = self
            .active
            .as_ref()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != expected {
            return Err(PublicationError::InvalidTransition);
        }
        Ok(transaction)
    }
}

fn operation_id(
    transaction: &PublicationTransaction,
    effect: PublicationEffectKind,
) -> Result<OpaqueId, PublicationError> {
    let label = match effect {
        PublicationEffectKind::PersistIntent => "intent",
        PublicationEffectKind::StageNewPoints => "stage",
        PublicationEffectKind::CloseOldPoints => "close",
        PublicationEffectKind::VerifyReadback => "verify",
        PublicationEffectKind::CommitVisibleEpoch => "visibility",
        PublicationEffectKind::PublishControlSnapshot => "snapshot",
        PublicationEffectKind::Compensate => "compensate",
        PublicationEffectKind::Abandon => "abandon",
        PublicationEffectKind::CommitAbortFinalization => "abort-commit",
        PublicationEffectKind::PublishAbortSnapshot => "abort-snapshot",
    };
    let mut value = format!("publication-{label}-");
    for byte in transaction.prepared.prepared_input_digest.as_bytes() {
        let _ = write!(value, "{byte:02x}");
    }
    OpaqueId::new(value).map_err(|_| PublicationError::OperationMismatch)
}

fn effect_points(
    transaction: &PublicationTransaction,
    effect: PublicationEffectKind,
) -> Result<Vec<PublicationPointExpectation>, PublicationError> {
    let expected = transaction.expected_points.as_ref();
    let ids = match effect {
        PublicationEffectKind::StageNewPoints => Some(&manifest_diff(transaction).create),
        PublicationEffectKind::CloseOldPoints | PublicationEffectKind::Compensate => {
            Some(&manifest_diff(transaction).retire)
        }
        PublicationEffectKind::VerifyReadback => None,
        _ => return Ok(Vec::new()),
    };
    let expected = expected.ok_or(PublicationError::InvalidPreparedPublication)?;
    let mut points = Vec::new();
    match effect {
        PublicationEffectKind::StageNewPoints => {
            let ids = ids.ok_or(PublicationError::InvalidPreparedPublication)?;
            if ids.len() > transaction.max_points {
                return Err(PublicationError::BudgetExceeded);
            }
            for id in ids.iter().map(|entry| entry.point_id) {
                let point = expected
                    .new
                    .iter()
                    .find(|point| point.point_id == id)
                    .ok_or(PublicationError::InvalidPreparedPublication)?;
                if point.payload.valid_from_epoch != transaction.target_epoch
                    || point.payload.valid_until_epoch_exclusive.is_some()
                {
                    return Err(PublicationError::EpochMismatch);
                }
                points.push(point.clone());
            }
        }
        PublicationEffectKind::CloseOldPoints | PublicationEffectKind::Compensate => {
            let ids = ids.ok_or(PublicationError::InvalidPreparedPublication)?;
            if ids.len() > transaction.max_points {
                return Err(PublicationError::BudgetExceeded);
            }
            for id in ids.iter().map(|entry| entry.point_id) {
                let mut point = expected
                    .old
                    .iter()
                    .find(|point| point.point_id == id)
                    .ok_or(PublicationError::InvalidPreparedPublication)?
                    .clone();
                if effect == PublicationEffectKind::CloseOldPoints
                    && point.payload.valid_from_epoch >= transaction.target_epoch
                {
                    return Err(PublicationError::EpochMismatch);
                }
                if effect == PublicationEffectKind::CloseOldPoints {
                    point.payload.valid_until_epoch_exclusive = Some(transaction.target_epoch);
                    point
                        .payload
                        .validate()
                        .map_err(|_| PublicationError::ClosureReadbackMismatch)?;
                }
                points.push(point);
            }
        }
        PublicationEffectKind::VerifyReadback => {
            if manifest_diff(transaction).create.len() + manifest_diff(transaction).retire.len()
                > transaction.max_points
            {
                return Err(PublicationError::BudgetExceeded);
            }
            for entry in &manifest_diff(transaction).create {
                let point = expected
                    .new
                    .iter()
                    .find(|point| point.point_id == entry.point_id)
                    .ok_or(PublicationError::InvalidPreparedPublication)?;
                points.push(point.clone());
            }
            for entry in &manifest_diff(transaction).retire {
                let mut point = expected
                    .old
                    .iter()
                    .find(|point| point.point_id == entry.point_id)
                    .ok_or(PublicationError::InvalidPreparedPublication)?
                    .clone();
                point.payload.valid_until_epoch_exclusive = Some(transaction.target_epoch);
                point
                    .payload
                    .validate()
                    .map_err(|_| PublicationError::ClosureReadbackMismatch)?;
                points.push(point);
            }
            points.sort_by_key(|point| point.point_id);
        }
        _ => {}
    }
    Ok(points)
}

pub fn effect_command<C: CancellationProbe>(
    transaction: &PublicationTransaction,
    effect: PublicationEffectKind,
    context: &OperationContext<C>,
) -> Result<PublicationEffectCommand, PublicationError> {
    let binding = PublicationEffectBinding {
        transaction_id: transaction.prepared.transaction_id.clone(),
        request_id: context.request_id(),
        relative_deadline_ms: context.relative_deadline_ms().get(),
        budget_ref: context.budget_ref().clone(),
        operation_id: operation_id(transaction, effect)?,
        prepared_input_digest: transaction.prepared.prepared_input_digest,
        scope: transaction.prepared.scope().clone(),
        target_epoch: transaction.target_epoch,
        previous_visible_epoch: transaction.previous_visible_epoch,
        last_reserved_epoch: transaction.target_epoch,
        old_manifest_digest: transaction.prepared.old_manifest_digest,
        old_manifest: transaction.prepared.old_manifest.clone(),
        new_manifest_digest: transaction.prepared.new_manifest_digest,
        new_manifest: transaction.prepared.new_manifest.clone(),
        difference: transaction.prepared.difference.clone(),
        guards: transaction.prepared.guards,
    };
    let original_points = if effect == PublicationEffectKind::CloseOldPoints {
        let expected = transaction
            .expected_points
            .as_ref()
            .ok_or(PublicationError::InvalidPreparedPublication)?;
        manifest_diff(transaction)
            .retire
            .iter()
            .map(|entry| {
                expected
                    .old
                    .iter()
                    .find(|point| point.point_id == entry.point_id)
                    .cloned()
                    .ok_or(PublicationError::InvalidPreparedPublication)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let required_absent_point_ids = if effect == PublicationEffectKind::Compensate {
        entry_ids(&manifest_diff(transaction).create)
    } else {
        Vec::new()
    };
    Ok(PublicationEffectCommand {
        effect,
        binding,
        current_guards: transaction.prepared.observed.current_guards,
        points: effect_points(transaction, effect)?,
        original_points,
        required_absent_point_ids,
        abort_request: None,
    })
}

fn run_effect<C, E, R>(
    transaction: &mut PublicationTransaction,
    effects: &mut E,
    readback: &mut R,
    context: &OperationContext<C>,
    effect: PublicationEffectKind,
) -> Result<(PublicationEffectCommand, PublicationEffectReadback), PublicationError>
where
    C: CancellationProbe,
    E: PublicationEffectPort<C>,
    R: PublicationReadbackPort<C>,
{
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let context_binding = OperationContextBinding::from_context(context);
    match &transaction.original_context {
        Some(original) if original != &context_binding => {
            return Err(PublicationError::OperationMismatch);
        }
        Some(_) => {}
        None => transaction.original_context = Some(context_binding),
    }
    if transaction
        .pending_effect
        .is_some_and(|pending| pending != effect)
    {
        return Err(PublicationError::OutcomeUnknown);
    }
    let command = effect_command(transaction, effect, context)?;
    let is_readback_only = effect == PublicationEffectKind::VerifyReadback;
    if transaction.pending_effect.is_none() && !is_readback_only {
        let identity = MutationIdentity::new(
            command.binding.operation_id.clone(),
            IdempotencyClass::RetrySameIdentity,
        );
        // Retain the unresolved effect before dispatch so cancellation or any
        // interruption before readback cannot make a later call replay it.
        transaction.pending_effect = Some(effect);
        // Acknowledgement and rejection are hints only. Always read the owner state.
        let _attempt = effects.apply(&command, &identity, context);
    }
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let observed = readback.readback(&command, context);
    if observed.effect != effect || observed.binding != command.binding {
        transaction.phase = PublicationPhase::PublicationBlocked;
        transaction.pending_effect = None;
        return Err(PublicationError::OperationMismatch);
    }
    if observed.points.len() > transaction.max_points
        || observed.missing_point_ids.len() > transaction.max_points
        || observed.unexpected_point_ids.len() > transaction.max_points
    {
        transaction.phase = PublicationPhase::PublicationBlocked;
        transaction.pending_effect = None;
        return Err(PublicationError::BudgetExceeded);
    }
    match observed.outcome {
        // The caller clears this only after validating the complete effect-specific
        // readback, not merely the port's disposition.
        PublicationEffectOutcome::Applied => {}
        PublicationEffectOutcome::NotApplied => {
            transaction.pending_effect = None;
            return Err(PublicationError::EffectNotObserved);
        }
        PublicationEffectOutcome::Conflict => {
            transaction.pending_effect = None;
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Err(PublicationError::ControlConflict);
        }
        PublicationEffectOutcome::Unknown => {
            transaction.pending_effect = Some(effect);
            return Err(PublicationError::OutcomeUnknown);
        }
    }
    Ok((command, observed))
}

pub fn readback_only<C, R>(
    transaction: &mut PublicationTransaction,
    readback: &mut R,
    context: &OperationContext<C>,
    effect: PublicationEffectKind,
) -> Result<(PublicationEffectCommand, PublicationEffectReadback), PublicationError>
where
    C: CancellationProbe,
    R: PublicationReadbackPort<C>,
{
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let context_binding = OperationContextBinding::from_context(context);
    match &transaction.original_context {
        Some(original) if original != &context_binding => {
            return Err(PublicationError::OperationMismatch);
        }
        Some(_) => {}
        None => transaction.original_context = Some(context_binding),
    }
    let command = effect_command(transaction, effect, context)?;
    let observed = readback.readback(&command, context);
    if observed.effect != effect || observed.binding != command.binding {
        transaction.phase = PublicationPhase::PublicationBlocked;
        return Err(PublicationError::OperationMismatch);
    }
    if observed.points.len() > transaction.max_points
        || observed.missing_point_ids.len() > transaction.max_points
        || observed.unexpected_point_ids.len() > transaction.max_points
    {
        transaction.phase = PublicationPhase::PublicationBlocked;
        return Err(PublicationError::BudgetExceeded);
    }
    if observed.outcome == PublicationEffectOutcome::Unknown {
        if transaction.pending_effect.is_none() {
            transaction.pending_effect = Some(effect);
        }
        return Err(PublicationError::OutcomeUnknown);
    }
    Ok((command, observed))
}

pub fn point_readback_matches(
    expected: &[PublicationPointExpectation],
    observed: &[PublicationPointReadback],
) -> bool {
    expected.len() == observed.len()
        && expected.iter().zip(observed).all(|(expected, observed)| {
            expected.point_id == observed.point_id
                && expected.identity == observed.identity
                && expected.payload == observed.payload
                && expected.payload.validate().is_ok()
                && observed.payload.validate().is_ok()
                && expected.payload.identity().ok() == observed.payload.identity().ok()
                && expected.vectors == observed.vectors
        })
}

/// Inputs must already have passed bounded, strictly sorted manifest validation.
/// A merge walk uses no cloned manifests, maps, ID sets or additional allocation.
/// Exact retained entries are allowed; even a digest-only change is not retained.
pub fn changed_point_id_is_reused(
    old: Option<&ProjectionManifest>,
    new: &ProjectionManifest,
) -> bool {
    let Some(old) = old else {
        return false;
    };
    let mut previous = old.entries.iter().peekable();
    for proposed in &new.entries {
        while previous
            .peek()
            .is_some_and(|entry| entry.point_id < proposed.point_id)
        {
            let _ = previous.next();
        }
        if let Some(existing) = previous.peek()
            && existing.point_id == proposed.point_id
        {
            if *existing != proposed {
                return true;
            }
            let _ = previous.next();
        }
    }
    false
}

const fn manifest_diff(transaction: &PublicationTransaction) -> &ManifestDiff {
    &transaction.prepared.difference
}

fn entry_ids(entries: &[search_projection_planner::ProjectionManifestEntry]) -> Vec<PointId128> {
    // The planner groups changed and removed old entries separately. Sort the
    // combined exact ID list; never rely on incidental diff grouping for receipts.
    let mut ids = entries
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}
