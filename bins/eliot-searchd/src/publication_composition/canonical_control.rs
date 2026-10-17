//! Root-owned composition of the canonical publication owner with the existing
//! C02 journal/snapshot publisher and the retained Qdrant bridge.
//!
//! This module owns no durable state. The borrowed `BoundControlJournal` is the
//! existing redb owner; the C07 set input is the exact object/readback emitted
//! by the preparation owner. Qdrant mutations are single bridge calls followed
//! by exact point-ID readback. Each adapter pair is bound to one invocation's
//! current context; original intent and abort-attempt coordinates are rehydrated
//! from finite C02 metadata. Lost acknowledgements are reconciled from the
//! original deterministic operation ID and command digest before retry.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

use search_contracts::indexed::{
    NamedVectorValue, PointPayload, PointReadback, VectorName, VectorValue,
};
use search_contracts::{
    Blake3Digest32, CanonicalDigestDomain, DigestInputLimit, OpaqueId, PublicationGuards,
    PublicationIntent, PublicationIntentId, PublicationIntentState, PublicationReceiptId,
    ReceiptRef, RequestId, blake3_raw,
};
use search_control_redb::{
    AbortResolutionMetadataV1, BoundControlJournal, CommitRecoveryDecision, ControlCommitReceipt,
    ControlPortCommand, ControlSnapshotPublisher, MigrationBootstrapWork, MutationId,
    PersistentControlJournal, PreparedArtifactSetRecordV1, PublicationAbortFinalizationMetadataV1,
    PublicationAbortFinalizationRecordV1, PublicationIntentBindingMetadataV1,
    PublicationIntentBindingRecordV1, PublicationIntentHead, PublicationIntentUpdate,
    PublicationManifestChange, PublicationReadbackEvidence, PublicationRecoveryProjectionSnapshot,
    PublicationSuccessor, PublicationVisibilityState, VisibleEpochCommit,
    control_mutation_identity,
};
use search_ports::{CancellationProbe, ControlSnapshotPort, MutationIdentity, OperationContext};
use search_publication::{
    AbortFinalizationRequest, AbortedPublicationResolution, PublicationControlReadback,
    PublicationEffectBinding, PublicationEffectCommand, PublicationEffectKind,
    PublicationEffectOutcome, PublicationEffectPort, PublicationEffectReadback,
    PublicationIntentReadback, PublicationMutationAttempt, PublicationPhase,
    PublicationPointExpectation, PublicationPointReadback, PublicationReadbackPort,
    PublicationSnapshotReadback,
};
use search_qdrant_bridge::CollectionRoute as BridgeRoute;
use search_qdrant_bridge::real::{
    BlockingRealQueryPlane, CanonicalPointInput, CanonicalRequestContext,
    CanonicalValidityRestoreOutcome,
};

use crate::owner_composition::NativeReopenedArtifactSet;
use crate::preparation_composition::{
    artifact_payloads::PreparedRawCodeArtifacts, native_artifact_store::NativeStoredArtifactSet,
};
use crate::registry_control::SourceIdentityIdsV2;

const OPERATION_DOMAIN: &str = "eliot/raw/search-publication-c02-operation/v1";
const COMMAND_DOMAIN: &str = "eliot/raw/search-publication-c02-command/v1";
const READBACK_DOMAIN: &str = "eliot/raw/search-publication-point-readback/v1";
const CONTROL_STATE_DOMAIN: &str = "eliot/raw/search-publication-control-state/v1";
const SNAPSHOT_DOMAIN: &str = "eliot/raw/search-publication-control-snapshot/v1";
const SNAPSHOT_ROW_DOMAIN: &str = "eliot/raw/search-publication-control-snapshot-row/v1";
const ABORT_REQUEST_DOMAIN: &str = "eliot/raw/search-publication-abort-request/v1";
const ABORT_SCOPE_DOMAIN: &str = "eliot/raw/search-publication-abort-scope/v1";
const ABORT_MEMBERSHIPS_DOMAIN: &str = "eliot/raw/search-publication-abort-memberships/v1";
const MAX_DIGEST_INPUT: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    ContextMismatch,
    InvalidBinding,
    ControlUnavailable,
    /// No C02 membership-shadow writer is available for the Abandon phase.
    AbandonmentUnavailable,
    PointEffectUnavailable,
    ReadbackUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextBinding {
    request_id: RequestId,
    relative_deadline_ms: u64,
    budget_ref: search_contracts::OpaqueRef,
}

impl ContextBinding {
    fn from_context<C: CancellationProbe>(context: &OperationContext<C>) -> Self {
        Self {
            request_id: context.request_id(),
            relative_deadline_ms: context.relative_deadline_ms().get(),
            budget_ref: context.budget_ref().clone(),
        }
    }
}

struct VerifiedPoints {
    binding: PublicationEffectBinding,
    visibility_operation_id: Option<OpaqueId>,
    readback_digest: Blake3Digest32,
    new_manifest_ref: ReceiptRef,
    retired_manifest_ref: ReceiptRef,
}

type PointReadbackObservation = (
    Vec<PublicationPointReadback>,
    Vec<search_point_identity::s11::PointId128>,
    Vec<search_point_identity::s11::PointId128>,
    PublicationEffectOutcome,
);

struct PendingVisibility {
    binding: PublicationEffectBinding,
    request: VisibleEpochCommit,
    mutation: MutationIdentity,
}

struct CanonicalPublicationIo<'a, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    control: BoundControlJournal<'a, C>,
    plane: &'a mut BlockingRealQueryPlane,
    route: BridgeRoute,
    c13: CanonicalPublicationArtifacts<'a>,
    deadline: Instant,
    work: &'a F,
    context: CanonicalRequestContext,
    context_binding: ContextBinding,
    publication_intent_id: PublicationIntentId,
    publication_receipt_id: PublicationReceiptId,
    verified_points: Option<VerifiedPoints>,
    pending_visibility: Option<PendingVisibility>,
}

/// One request-scoped Root adapter pair for the canonical publication machine.
///
/// The owner creates this from the same `PersistentControlJournal`, snapshot
/// publisher, C13 storage result, route, current absolute deadline and
/// cancellation capability that it passes to the coordinator call. On restore,
/// these are the recovery invocation's context; C02 retains the original intent
/// and successful abort-attempt coordinates. `split` returns
/// two borrowed views because the model deliberately keeps effect and readback
/// ports separate; the `RefCell` serializes those synchronous calls without
/// creating another owner or durable map.
pub struct CanonicalPublicationPorts<'a, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    io: RefCell<CanonicalPublicationIo<'a, C, F, E>>,
}

/// Borrowed, source-backed C13 data needed by publication effects.
///
/// Constructors borrow only actual owner-issued C07 write or complete reopen
/// output. Ordinary metadata, decoded references and pure preparation alone
/// cannot construct this view. The adapter still rechecks current identities.
#[derive(Clone, Copy)]
pub struct CanonicalPublicationArtifacts<'a> {
    prepared: &'a PreparedRawCodeArtifacts,
    record: &'a PreparedArtifactSetRecordV1,
    object_ref: &'a search_revision_store::derived_artifact::DerivedArtifactObjectRefV1,
}

impl<'a> CanonicalPublicationArtifacts<'a> {
    /// Borrows the exact outputs of the normal C07 set-write path.
    pub(crate) const fn from_stored(value: &'a NativeStoredArtifactSet) -> Self {
        Self {
            prepared: value.prepared(),
            record: value.c02_candidate(),
            object_ref: value.c07_write().object_ref(),
        }
    }

    /// Borrows source-backed C13 outputs after the C07 set and its members were
    /// reopened and verified by their existing owner.
    pub(crate) const fn from_reopened(value: &'a NativeReopenedArtifactSet<'_, '_>) -> Self {
        Self {
            prepared: value.prepared(),
            record: value.record(),
            object_ref: value.c07_observation().object_ref(),
        }
    }
}

pub struct CanonicalPublicationEffectPort<'ports, 'io, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    io: &'ports RefCell<CanonicalPublicationIo<'io, C, F, E>>,
}

pub struct CanonicalPublicationReadbackPort<'ports, 'io, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    io: &'ports RefCell<CanonicalPublicationIo<'io, C, F, E>>,
}

impl<'a, C, F, E> CanonicalPublicationPorts<'a, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    /// Binds the actual C02 journal and publisher without opening another store.
    /// `deadline` is the absolute deadline for this coordinator invocation; on
    /// recovery it is the current recovery deadline. The supplied C13 value can
    /// only be borrowed from actual Root C07/C02 preparation or reopen outputs.
    /// `ids` must use the same original deadline and owner-work callback.
    #[allow(clippy::too_many_arguments)] // Each argument is one existing Root owner/context.
    pub(crate) fn bind(
        journal: &'a mut PersistentControlJournal,
        publisher: &'a mut ControlSnapshotPublisher,
        ids: &SourceIdentityIdsV2<'_, C>,
        plane: &'a mut BlockingRealQueryPlane,
        route: BridgeRoute,
        c13: CanonicalPublicationArtifacts<'a>,
        context: &OperationContext<C>,
        deadline: Instant,
        work: &'a F,
    ) -> Result<Self, AdapterError> {
        context
            .preflight()
            .map_err(|_| AdapterError::ContextMismatch)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(AdapterError::ContextMismatch)?;
        if remaining.as_millis() > u128::from(context.relative_deadline_ms().get()) {
            return Err(AdapterError::ContextMismatch);
        }
        let cancellation: Arc<dyn CancellationProbe + Send + Sync> =
            Arc::new(context.cancellation().clone());
        let request_context = CanonicalRequestContext::with_deadline(deadline, cancellation)
            .map_err(|_| AdapterError::ContextMismatch)?;
        let control = journal
            .bind_control_port_with_original_context::<C, E>(publisher, deadline, work)
            .map_err(|_| AdapterError::ControlUnavailable)?;
        // Read the original C02 identity record before requesting any new IDs.
        // An existing operation reuses its exact Root-issued identities; only a
        // verified first intent or a resolved predecessor allocates a new pair.
        let checkpoint = control
            .read_publication_checkpoint_with_original_context(deadline, work)
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        let candidate = c13.record;
        let candidate_target = c13.prepared.scope().valid_from_epoch;
        let (publication_intent_id, publication_receipt_id) =
            match (checkpoint.intent(), checkpoint.original_binding()) {
                (None, None)
                    if checkpoint.abort_finalization().is_none()
                        && checkpoint.visibility().visible_epoch.get() == 0
                        && checkpoint.last_reserved_epoch().get() == 0 =>
                {
                    allocate_publication_ids(ids, context)?
                }
                (Some(intent), Some(record)) if record.matches_intent(intent) => {
                    let metadata = record.metadata();
                    let same_original = intent.target_epoch == candidate_target
                        && intent.prepared_manifest_ref == *candidate.reference();
                    if same_original {
                        (
                            metadata.publication_intent_id,
                            metadata.publication_receipt_id,
                        )
                    } else if intent.state == PublicationIntentState::ControlCommitted
                        && intent.target_epoch.checked_next().ok() == Some(candidate_target)
                        && checkpoint.visibility().visible_epoch == intent.target_epoch
                        && checkpoint.last_reserved_epoch() == intent.target_epoch
                        && checkpoint.abort_finalization().is_none()
                    {
                        allocate_publication_ids(ids, context)?
                    } else {
                        return Err(AdapterError::InvalidBinding);
                    }
                }
                _ => return Err(AdapterError::ControlUnavailable),
            };
        if publication_intent_id.as_bytes() == publication_receipt_id.as_bytes() {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(Self {
            io: RefCell::new(CanonicalPublicationIo {
                control,
                plane,
                route,
                c13,
                deadline,
                work,
                context: request_context,
                context_binding: ContextBinding::from_context(context),
                publication_intent_id,
                publication_receipt_id,
                verified_points: None,
                pending_visibility: None,
            }),
        })
    }

    /// Split synchronous effect and independent readback views over one Root owner.
    pub(crate) const fn split(
        &self,
    ) -> (
        CanonicalPublicationEffectPort<'_, 'a, C, F, E>,
        CanonicalPublicationReadbackPort<'_, 'a, C, F, E>,
    ) {
        (
            CanonicalPublicationEffectPort { io: &self.io },
            CanonicalPublicationReadbackPort { io: &self.io },
        )
    }
}

impl<C, F, E> CanonicalPublicationIo<'_, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    fn check_context(&self, context: &OperationContext<C>) -> Result<(), AdapterError> {
        context
            .preflight()
            .map_err(|_| AdapterError::ContextMismatch)?;
        if ContextBinding::from_context(context) != self.context_binding {
            return Err(AdapterError::ContextMismatch);
        }
        self.context
            .check()
            .map_err(|_| AdapterError::ContextMismatch)
    }

    fn abort_attempt_matches_context(
        binding: &PublicationEffectBinding,
        context: &OperationContext<C>,
    ) -> bool {
        binding.request_id == context.request_id()
            && binding.relative_deadline_ms == context.relative_deadline_ms().get()
            && binding.budget_ref == *context.budget_ref()
    }

    fn validate_c13(&self, binding: &PublicationEffectBinding) -> Result<(), AdapterError> {
        let candidate = self.c13.record;
        let metadata = candidate.metadata();
        let prepared = self.c13.prepared;
        let scope = &binding.scope;
        let planning = prepared.scope();
        let membership = &planning.membership;
        let schema_identity = prepared.schema().identity().ok();
        if self.route.generation != scope.collection_generation_id
            || metadata.operation_id != binding.transaction_id
            || metadata.source_revision != scope.source
            || metadata.materialization_id != scope.materialization_id
            || metadata.representation_id != scope.representation_id
            || prepared.plan().source_membership_id != scope.source_membership_id
            || prepared.plan().projection_membership_id != scope.projection_membership_id
            || prepared.plan().manifest != binding.new_manifest
            || membership.installation_incarnation_id != scope.installation_incarnation_id
            || membership.collection_generation_id != scope.collection_generation_id
            || membership.projection_membership_id != scope.projection_membership_id
            || membership.source_membership_id != scope.source_membership_id
            || membership.source_id != scope.source.source_id
            || membership.source_revision_id != scope.source.revision_id
            || membership.representation_id != scope.representation_id
            || membership.projection_schema_id != scope.projection_schema_id
            || membership.projection_profile_set_id != scope.projection_profile_set_id
            || membership.access_partition_id != scope.access_partition_id
            || membership.scoring_partition_id != scope.scoring_partition_id
            || prepared.units().manifest_digest() != scope.unit_manifest_digest
            || planning.valid_from_epoch != binding.target_epoch
            || planning.valid_until_epoch_exclusive.is_some()
            || prepared.schema().installation_incarnation_id() != &scope.installation_incarnation_id
            || prepared.schema().collection_generation_id() != &scope.collection_generation_id
            || prepared.schema().projection_profile_set_id() != &scope.projection_profile_set_id
            || schema_identity.map(|identity| *identity.as_digest()) != Some(scope.schema_identity)
        {
            return Err(AdapterError::InvalidBinding);
        }
        let object_ref = self.c13.object_ref;
        if object_ref.stored_object_digest() != metadata.stored_object_digest
            || object_ref.stored_bytes() != metadata.stored_object_length
            || object_ref
                .encode()
                .map_err(|_| AdapterError::InvalidBinding)?
                != metadata.object_reference_canonical
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(())
    }

    fn c02_intent(
        &self,
        binding: &PublicationEffectBinding,
    ) -> PublicationIntent {
        let candidate = self.c13.record;
        PublicationIntent {
            publication_intent_id: self.publication_intent_id,
            target_epoch: binding.target_epoch,
            prepared_manifest_ref: candidate.reference().clone(),
            owner_source_membership_access_guards: binding.guards,
            state: PublicationIntentState::Prepared,
        }
    }

    fn original_binding_record(
        &self,
        binding: &PublicationEffectBinding,
        intent: &PublicationIntent,
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
    ) -> Result<PublicationIntentBindingRecordV1, AdapterError> {
        PublicationIntentBindingRecordV1::prepare_with_original_context(
            PublicationIntentBindingMetadataV1 {
                publication_intent_id: intent.publication_intent_id,
                publication_receipt_id: self.publication_receipt_id,
                target_epoch: intent.target_epoch,
                prepared_manifest_reference: intent.prepared_manifest_ref.clone(),
                guards: intent.owner_source_membership_access_guards,
                c02_mutation_id: operation_id,
                c02_command_digest: command_digest,
                expected_control_generation: expected_generation,
                original_operation_id: binding.operation_id.clone(),
                original_request_id: binding.request_id,
                original_relative_deadline_ms: binding.relative_deadline_ms,
                original_budget_ref: binding.budget_ref.clone(),
            },
            self.control.limits(),
            self.deadline,
            self.work,
        )
        .map_err(|_| AdapterError::ControlUnavailable)
    }

    fn c02_mutation_id(
        binding: &PublicationEffectBinding,
        phase: &str,
    ) -> Result<MutationId, AdapterError> {
        Self::c02_mutation_id_for_operation(binding, &binding.operation_id, phase)
    }

    fn c02_mutation_id_for_operation(
        binding: &PublicationEffectBinding,
        operation: &OpaqueId,
        phase: &str,
    ) -> Result<MutationId, AdapterError> {
        let digest = digest_parts(
            OPERATION_DOMAIN,
            &[
                operation.as_str().as_bytes(),
                binding.transaction_id.as_str().as_bytes(),
                phase.as_bytes(),
            ],
        )?;
        Ok(MutationId(*digest.as_bytes()))
    }

    fn c02_command_digest(
        &self,
        binding: &PublicationEffectBinding,
        phase: &str,
        state: PublicationIntentState,
        extra: Option<Blake3Digest32>,
    ) -> Result<Blake3Digest32, AdapterError> {
        let candidate_ref = self.c13.record.reference();
        let state = intent_state_label(state);
        let extra = extra.unwrap_or_else(|| Blake3Digest32::from_bytes([0; 32]));
        let relative_deadline = binding.relative_deadline_ms.to_be_bytes();
        digest_parts(
            COMMAND_DOMAIN,
            &[
                binding.prepared_input_digest.as_bytes(),
                self.publication_intent_id.as_bytes(),
                self.publication_receipt_id.as_bytes(),
                binding.transaction_id.as_str().as_bytes(),
                binding.operation_id.as_str().as_bytes(),
                phase.as_bytes(),
                state.as_bytes(),
                candidate_ref.as_str().as_bytes(),
                binding.request_id.as_bytes(),
                &relative_deadline,
                binding.budget_ref.as_str().as_bytes(),
                extra.as_bytes(),
            ],
        )
    }

    fn read_head(&self) -> Result<PublicationIntentHead, AdapterError> {
        self.control
            .read_publication_intent_with_original_context(self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)
    }

    fn read_checkpoint(
        &self,
        _context: &OperationContext<C>,
    ) -> Result<search_control_redb::PublicationRecoveryCheckpoint, AdapterError> {
        self.control
            .read_publication_checkpoint_with_original_context(self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)
    }

    fn read_recovery_projection(
        &self,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
    ) -> Result<PublicationRecoveryProjectionSnapshot, AdapterError> {
        let projection = self
            .control
            .read_publication_recovery_projection_snapshot_with_original_context(
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        if projection.identity() != checkpoint.identity()
            || projection.generation() != checkpoint.generation()
            || projection.visibility() != checkpoint.visibility()
            || projection.intent() != checkpoint.intent()
            || projection.last_reserved_epoch() != checkpoint.last_reserved_epoch()
        {
            return Err(AdapterError::ReadbackUnknown);
        }
        Ok(projection)
    }

    fn bind_original_intent_context(
        &self,
        binding: &PublicationEffectBinding,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
    ) -> Result<PublicationEffectBinding, AdapterError> {
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        // Verify the map references against the same checkpoint snapshot even
        // though this reconstruction consumes only the retained intent.
        let _projection = self.read_recovery_projection(checkpoint)?;
        self.expected_c02_intent(binding, intent)?;
        let record = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?;
        if !record.matches_intent(intent) {
            return Err(AdapterError::InvalidBinding);
        }
        let metadata = record.metadata();
        let mut exact = binding.clone();
        exact.request_id = metadata.original_request_id;
        exact.relative_deadline_ms = metadata.original_relative_deadline_ms;
        exact.budget_ref.clone_from(&metadata.original_budget_ref);
        Ok(exact)
    }

    fn read_candidate_record(&self) -> Result<(), AdapterError> {
        let candidate = self.c13.record;
        let observed = self
            .control
            .read_prepared_artifact_set_with_original_context(
                candidate.reference(),
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        if observed.record() != candidate {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(())
    }

    /// Runs the parent-supplied owner/source/security poll at an explicit C02
    /// boundary. The callback is borrowed from the original operation context;
    /// it does not synthesize guard generations or renew the deadline.
    fn owner_checkpoint(&self) -> Result<(), AdapterError> {
        (self.work)(MigrationBootstrapWork::Checkpoint)
            .map_err(|_| AdapterError::ControlUnavailable)
    }

    /// Resolve the exact retained C02 command before any retry. A fresh absence
    /// observation alone is not permission to redraw a transaction after an
    /// interrupted acknowledgement; the owning journal must validate the same
    /// operation, digest, generation and record preconditions first.
    fn transact_publication_with_recovery(
        &mut self,
        command: &ControlPortCommand,
        mutation: &MutationIdentity,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        match self
            .control
            .recover_publication_with_original_context(command, mutation, self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            CommitRecoveryDecision::Committed(receipt) => return Ok(receipt),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => {}
            CommitRecoveryDecision::ConflictingInput
            | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                return Err(AdapterError::ControlUnavailable);
            }
        }

        self.owner_checkpoint()?;
        if let Ok(receipt) = self.control.transact_publication_with_original_context(
            command,
            mutation,
            self.deadline,
            self.work,
        ) {
            return Ok(receipt);
        }

        match self
            .control
            .recover_publication_with_original_context(command, mutation, self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            CommitRecoveryDecision::Committed(receipt) => Ok(receipt),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => {
                self.owner_checkpoint()?;
                if let Ok(receipt) = self.control.transact_publication_with_original_context(
                    command,
                    mutation,
                    self.deadline,
                    self.work,
                ) {
                    return Ok(receipt);
                }
                match self
                    .control
                    .recover_publication_with_original_context(
                        command,
                        mutation,
                        self.deadline,
                        self.work,
                    )
                    .map_err(|_| AdapterError::ControlUnavailable)?
                {
                    CommitRecoveryDecision::Committed(receipt) => Ok(receipt),
                    CommitRecoveryDecision::NotCommittedRetrySameOperation
                    | CommitRecoveryDecision::ConflictingInput
                    | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                        Err(AdapterError::ControlUnavailable)
                    }
                }
            }
            CommitRecoveryDecision::ConflictingInput
            | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                Err(AdapterError::ControlUnavailable)
            }
        }
    }

    fn expected_c02_intent(
        &self,
        binding: &PublicationEffectBinding,
        actual: &PublicationIntent,
    ) -> Result<(), AdapterError> {
        let prepared = self.c02_intent(binding);
        if actual.publication_intent_id != prepared.publication_intent_id
            || actual.target_epoch != prepared.target_epoch
            || actual.prepared_manifest_ref != prepared.prepared_manifest_ref
            || actual.owner_source_membership_access_guards
                != prepared.owner_source_membership_access_guards
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(())
    }

    fn original_intent_binding(
        &self,
        current_binding: &PublicationEffectBinding,
        intent: &PublicationIntent,
        record: &PublicationIntentBindingRecordV1,
    ) -> Result<(PublicationEffectBinding, ControlCommitReceipt), AdapterError> {
        let metadata = record.metadata();
        if !record.matches_intent(intent)
            || metadata.publication_intent_id != self.publication_intent_id
            || metadata.publication_receipt_id != self.publication_receipt_id
        {
            return Err(AdapterError::InvalidBinding);
        }
        let mut original = current_binding.clone();
        original
            .operation_id
            .clone_from(&metadata.original_operation_id);
        original.request_id = metadata.original_request_id;
        original.relative_deadline_ms = metadata.original_relative_deadline_ms;
        original
            .budget_ref
            .clone_from(&metadata.original_budget_ref);
        self.expected_c02_intent(&original, intent)?;
        let operation_id = Self::c02_mutation_id(&original, "intent-durable")?;
        let command_digest = self.c02_command_digest(
            &original,
            "intent-durable",
            PublicationIntentState::IntentDurable,
            None,
        )?;
        if operation_id != metadata.c02_mutation_id || command_digest != metadata.c02_command_digest
        {
            return Err(AdapterError::InvalidBinding);
        }
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                metadata.c02_mutation_id,
                metadata.c02_command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        if receipt.before_generation != metadata.expected_control_generation
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok((original, receipt))
    }

    fn validate_abort_original_intent(
        &self,
        binding: &PublicationEffectBinding,
        request: &AbortFinalizationRequest,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
        intent: &PublicationIntent,
    ) -> Result<PublicationEffectBinding, AdapterError> {
        let original_record = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?;
        let (original_binding, receipt) =
            self.original_intent_binding(binding, intent, original_record)?;
        let original_metadata = original_record.metadata();
        let mut original_intent = intent.clone();
        original_intent.state = PublicationIntentState::IntentDurable;
        if original_metadata.original_operation_id != *request.intent.persist_operation_id()
            || original_metadata.original_request_id != request.intent.request_id()
            || request.intent.control_generation() != receipt.after_generation
            || request.intent.control_state_digest() != intent_digest(&original_intent)?
            || request.intent.transaction_id() != &original_binding.transaction_id
            || request.intent.target_epoch() != original_binding.target_epoch
            || request.intent.guards() != original_binding.guards
            || request.intent.old_manifest_digest() != original_binding.old_manifest_digest
            || request.intent.new_manifest_digest() != original_binding.new_manifest_digest
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(original_binding)
    }

    #[allow(clippy::too_many_lines)] // One exact C02 command/recovery sequence stays linear.
    #[allow(clippy::too_many_lines)] // Keeps one exact intent command and its recovery branch together.
    fn persist_or_reconcile_intent(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        self.check_context(context)?;
        self.validate_c13(command.binding())?;

        let binding = command.binding();
        let phase = "intent-durable";
        let operation_id = Self::c02_mutation_id(binding, phase)?;
        let prepared = self.c02_intent(binding);
        let head = self.read_head()?;
        let checkpoint = self.read_checkpoint(context)?;
        let digest_binding = if checkpoint
            .intent()
            .is_some_and(|intent| self.expected_c02_intent(binding, intent).is_ok())
        {
            self.bind_original_intent_context(binding, &checkpoint)?
        } else {
            binding.clone()
        };
        let command_digest = self.c02_command_digest(
            &digest_binding,
            phase,
            PublicationIntentState::IntentDurable,
            None,
        )?;
        let mutation =
            control_mutation_identity(operation_id).map_err(|_| AdapterError::InvalidBinding)?;
        let already_durable = head.intent.as_ref().is_some_and(|actual| {
            actual.publication_intent_id == prepared.publication_intent_id
                && actual.target_epoch == prepared.target_epoch
                && actual.prepared_manifest_ref == prepared.prepared_manifest_ref
                && actual.owner_source_membership_access_guards
                    == prepared.owner_source_membership_access_guards
                && actual.state == PublicationIntentState::IntentDurable
        });
        if head.generation != checkpoint.generation()
            || checkpoint.visibility().visible_epoch != binding.previous_visible_epoch
            || checkpoint.last_reserved_epoch() > binding.target_epoch
            || (checkpoint.last_reserved_epoch() == binding.target_epoch && !already_durable)
            || checkpoint.visibility().collection_generation_id
                != binding.scope.collection_generation_id
            || checkpoint.visibility().schema_identity_digest != binding.scope.schema_identity
            || checkpoint.visibility().guards != binding.guards
        {
            return Err(AdapterError::InvalidBinding);
        }

        if let Some(receipt) = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            let actual = head
                .intent
                .as_ref()
                .ok_or(AdapterError::ControlUnavailable)?;
            self.expected_c02_intent(binding, actual)?;
            self.read_candidate_record()?;
            return Ok(receipt);
        }

        let port_command = match head.intent.as_ref() {
            None => {
                let original_binding = self.original_binding_record(
                    binding,
                    &prepared,
                    operation_id,
                    command_digest,
                    head.generation,
                )?;
                let update = PublicationIntentUpdate::begin(
                    operation_id,
                    command_digest,
                    head.generation,
                    &prepared,
                )
                .and_then(|update| update.with_prepared_artifact_set_v1(self.c13.record.clone()))
                .and_then(|update| update.with_original_binding_v1(original_binding))
                .map_err(|_| AdapterError::InvalidBinding)?;
                ControlPortCommand::PublicationIntent(update)
            }
            Some(previous) if previous.state == PublicationIntentState::ControlCommitted => {
                let previous_binding = checkpoint
                    .original_binding()
                    .cloned()
                    .ok_or(AdapterError::ControlUnavailable)?;
                let original_binding = self.original_binding_record(
                    binding,
                    &prepared,
                    operation_id,
                    command_digest,
                    head.generation,
                )?;
                let successor = PublicationSuccessor::new(
                    operation_id,
                    command_digest,
                    head.generation,
                    previous.clone(),
                    *checkpoint.visibility(),
                    &prepared,
                )
                .and_then(|successor| {
                    successor.with_prepared_artifact_set_v1(self.c13.record.clone())
                })
                .and_then(|successor| {
                    successor.with_original_binding_v1(original_binding, previous_binding)
                })
                .map_err(|_| AdapterError::InvalidBinding)?;
                ControlPortCommand::PublicationSuccessor(successor)
            }
            Some(previous) => {
                return if previous.publication_intent_id == prepared.publication_intent_id
                    && previous.target_epoch == prepared.target_epoch
                    && previous.prepared_manifest_ref == prepared.prepared_manifest_ref
                    && previous.owner_source_membership_access_guards
                        == prepared.owner_source_membership_access_guards
                {
                    Err(AdapterError::ControlUnavailable)
                } else {
                    Err(AdapterError::InvalidBinding)
                };
            }
        };
        let _receipt = self.transact_publication_with_recovery(&port_command, &mutation)?;

        // Even a successful return is only a hint until the exact C02 operation
        // receipt and full intent value are read from the owner again.
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        let current = self
            .read_head()?
            .intent
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, &current)?;
        if current.state != PublicationIntentState::IntentDurable
            || receipt.operation_id != operation_id
            || receipt.command_digest != command_digest
        {
            return Err(AdapterError::InvalidBinding);
        }
        self.read_candidate_record()?;
        Ok(receipt)
    }

    fn advance_intent(
        &mut self,
        command: &PublicationEffectCommand,
        next: PublicationIntentState,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        self.check_context(context)?;
        self.validate_c13(command.binding())?;
        self.read_candidate_record()?;
        let binding = command.binding();
        let phase = intent_state_label(next);
        let operation_id = Self::c02_mutation_id(binding, phase)?;
        let checkpoint = self.read_checkpoint(context)?;
        let previous = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, previous)?;
        let digest_binding = self.bind_original_intent_context(binding, &checkpoint)?;
        let command_digest = self.c02_command_digest(&digest_binding, phase, next, None)?;
        let mutation =
            control_mutation_identity(operation_id).map_err(|_| AdapterError::InvalidBinding)?;

        if let Some(receipt) = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            let actual_checkpoint = self.read_checkpoint(context)?;
            let actual = actual_checkpoint
                .intent()
                .ok_or(AdapterError::ControlUnavailable)?;
            self.expected_c02_intent(binding, actual)?;
            if actual.state != next {
                return Err(AdapterError::InvalidBinding);
            }
            return Ok(receipt);
        }

        if !is_allowed_intent_predecessor(previous.state, next) {
            return Err(AdapterError::InvalidBinding);
        }
        let update = PublicationIntentUpdate::advance(
            operation_id,
            command_digest,
            checkpoint.generation(),
            previous.clone(),
            next,
        )
        .and_then(|update| update.with_prepared_artifact_set_v1(self.c13.record.clone()))
        .map_err(|_| AdapterError::InvalidBinding)?;
        let port_command = ControlPortCommand::PublicationIntent(update);
        let _receipt = self.transact_publication_with_recovery(&port_command, &mutation)?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        let actual = self
            .read_head()?
            .intent
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, &actual)?;
        if actual.state != next {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(receipt)
    }

    fn canonical_inputs(
        points: &[PublicationPointExpectation],
    ) -> Result<Vec<CanonicalPointInput>, AdapterError> {
        points
            .iter()
            .map(|expected| {
                let vectors = expected
                    .vectors()
                    .iter()
                    .map(|(name, value)| {
                        Ok(NamedVectorValue {
                            name: VectorName::new(name)
                                .map_err(|_| AdapterError::InvalidBinding)?,
                            value: value.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(CanonicalPointInput {
                    identity: expected.identity().clone(),
                    point: PointReadback {
                        payload: expected.payload().clone(),
                        vectors,
                    },
                })
            })
            .collect()
    }

    fn canonical_created_inputs(
        &self,
        command: &PublicationEffectCommand,
    ) -> Result<Vec<CanonicalPointInput>, AdapterError> {
        let create = &command.binding().difference.create;
        let expected_ids = create
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>();
        if expected_ids != command.required_absent_point_ids() {
            return Err(AdapterError::InvalidBinding);
        }

        let prepared = self.c13.prepared;
        let plan_points = prepared
            .plan()
            .points
            .iter()
            .map(|point| (point.point_id, point))
            .collect::<std::collections::BTreeMap<_, _>>();
        let planned_units = prepared
            .vectors()
            .iter()
            .map(|unit| (unit.unit_id, unit))
            .collect::<std::collections::BTreeMap<_, _>>();
        if plan_points.len() != prepared.plan().points.len()
            || planned_units.len() != prepared.vectors().len()
        {
            return Err(AdapterError::InvalidBinding);
        }

        create
            .iter()
            .map(|entry| {
                let point = plan_points
                    .get(&entry.point_id)
                    .ok_or(AdapterError::InvalidBinding)?;
                let unit = planned_units
                    .get(&point.identity.key().unit_id)
                    .ok_or(AdapterError::InvalidBinding)?;
                let payload = PointPayload::from_canonical_cbor(
                    point.expected_readback.payload_cbor.as_slice(),
                )
                .map_err(|_| AdapterError::InvalidBinding)?;
                if point.identity.point_id() != entry.point_id
                    || payload
                        .to_canonical_cbor()
                        .map_err(|_| AdapterError::InvalidBinding)?
                        != point.expected_readback.payload_cbor
                    || payload
                        .identity()
                        .map_err(|_| AdapterError::InvalidBinding)?
                        != point.expected_readback.payload_identity
                    || payload.valid_from_epoch != command.binding().target_epoch
                    || payload.valid_until_epoch_exclusive.is_some()
                {
                    return Err(AdapterError::InvalidBinding);
                }
                let vectors = unit
                    .vectors
                    .iter()
                    .map(|vector| (vector.name.as_str().to_owned(), vector.value.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>();
                if vectors.len() != unit.vectors.len()
                    || vectors
                        .keys()
                        .ne(point.expected_readback.vector_digests.keys())
                {
                    return Err(AdapterError::InvalidBinding);
                }
                let vectors = vectors
                    .iter()
                    .map(|(name, value)| {
                        Ok(NamedVectorValue {
                            name: VectorName::new(name)
                                .map_err(|_| AdapterError::InvalidBinding)?,
                            value: value.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(CanonicalPointInput {
                    identity: point.identity.clone(),
                    point: PointReadback { payload, vectors },
                })
            })
            .collect()
    }

    fn compensate_points(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<(), AdapterError> {
        let original_points = Self::canonical_inputs(command.points())?;
        let created_points = self.canonical_created_inputs(command)?;
        self.advance_intent(command, PublicationIntentState::Compensating, context)?;
        if !original_points.is_empty() {
            self.owner_checkpoint()?;
            match self.plane.restore_indexed_validity_exact(
                &self.route,
                &original_points,
                command.binding().target_epoch,
                &self.context,
            ) {
                Ok(
                    CanonicalValidityRestoreOutcome::MutationAcknowledged(_)
                    | CanonicalValidityRestoreOutcome::AlreadyRestoredVerified(_),
                ) => {}
                Err(_) => return Err(AdapterError::ReadbackUnknown),
            }
        }

        if created_points.is_empty() {
            return Ok(());
        }
        let current = self
            .plane
            .readback_indexed_points_exact(&self.route, &created_points, &self.context)
            .map_err(|_| AdapterError::ReadbackUnknown)?;
        let expected_ids = command.required_absent_point_ids();
        if current.missing_ids == expected_ids && current.points.is_empty() {
            return Ok(());
        }
        if !current.missing_ids.is_empty() || current.points.len() != created_points.len() {
            return Err(AdapterError::ReadbackUnknown);
        }
        self.owner_checkpoint()?;
        let deleted = self
            .plane
            .delete_indexed_exact(&self.route, &created_points, &self.context)
            .map_err(|_| AdapterError::ReadbackUnknown)?;
        if deleted.absent_ids != expected_ids {
            return Err(AdapterError::ReadbackUnknown);
        }
        Ok(())
    }

    fn read_compensation_effect(
        &self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        self.check_context(context)?;
        self.read_candidate_record()?;
        let head = self.read_head()?;
        let intent = head
            .intent
            .as_ref()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(command.binding(), intent)?;
        if intent.state != PublicationIntentState::Compensating {
            return Err(AdapterError::InvalidBinding);
        }

        let original_points = Self::canonical_inputs(command.points())?;
        if !original_points.is_empty() {
            match self.plane.readback_indexed_points_exact(
                &self.route,
                &original_points,
                &self.context,
            ) {
                Ok(observed)
                    if observed.missing_ids.is_empty()
                        && observed.points.len() == original_points.len() => {}
                _ => {
                    return Ok(empty_observation(
                        command.effect(),
                        command.binding().clone(),
                        PublicationEffectOutcome::Unknown,
                    ));
                }
            }
        }

        let created_points = self.canonical_created_inputs(command)?;
        let created_readback = if created_points.is_empty() {
            search_qdrant_bridge::real::CanonicalMutationReadback {
                points: Vec::new(),
                missing_ids: Vec::new(),
            }
        } else {
            match self.plane.readback_indexed_points_exact(
                &self.route,
                &created_points,
                &self.context,
            ) {
                Ok(observed) => observed,
                Err(_) => {
                    return Ok(empty_observation(
                        command.effect(),
                        command.binding().clone(),
                        PublicationEffectOutcome::Unknown,
                    ));
                }
            }
        };
        let expected_missing = command.required_absent_point_ids();
        if created_readback.missing_ids != expected_missing || !created_readback.points.is_empty() {
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            ));
        }
        let (points, missing, unexpected, outcome) =
            self.qdrant_readback(command, command.points())?;
        if outcome != PublicationEffectOutcome::Applied
            || !missing.is_empty()
            || !unexpected.is_empty()
        {
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            ));
        }
        Ok(PublicationEffectReadback {
            effect: command.effect(),
            binding: command.binding().clone(),
            outcome: PublicationEffectOutcome::Applied,
            points,
            missing_point_ids: created_readback.missing_ids,
            unexpected_point_ids: unexpected,
            intent: None,
            control: None,
            snapshot: None,
            abandonment: None,
            abort_request: None,
            abort_binding: None,
        })
    }

    fn qdrant_readback(
        &self,
        command: &PublicationEffectCommand,
        expected: &[PublicationPointExpectation],
    ) -> Result<PointReadbackObservation, AdapterError> {
        let inputs = Self::canonical_inputs(expected)?;
        let (observed, readback_expected, close_not_applied) = match self
            .plane
            .readback_indexed_points_exact(&self.route, &inputs, &self.context)
        {
            Ok(observed) => (observed, expected, false),
            Err(search_qdrant_bridge::BridgeError::ExactReadbackMismatch)
                if command.effect() == PublicationEffectKind::CloseOldPoints =>
            {
                let original = command.original_points();
                let original_inputs = Self::canonical_inputs(original)?;
                let observed = self
                    .plane
                    .readback_indexed_points_exact(&self.route, &original_inputs, &self.context)
                    .map_err(|_| AdapterError::ReadbackUnknown)?;
                (observed, original, true)
            }
            Err(_) => return Err(AdapterError::ReadbackUnknown),
        };
        let requested = readback_expected
            .iter()
            .map(PublicationPointExpectation::point_id)
            .collect::<Vec<_>>();
        let requested_set = requested
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let mut points = Vec::with_capacity(observed.points.len());
        let mut unexpected = Vec::new();
        let mut all_match =
            observed.missing_ids.is_empty() && observed.points.len() == readback_expected.len();
        for actual in observed.points {
            let Some(owner_expected) = readback_expected
                .iter()
                .find(|point| point.point_id() == actual.address)
            else {
                unexpected.push(actual.address);
                all_match = false;
                continue;
            };
            let mut vectors = std::collections::BTreeMap::new();
            for vector in &actual.point.vectors {
                if vectors
                    .insert(vector.name.as_str().to_owned(), vector.value.clone())
                    .is_some()
                {
                    all_match = false;
                }
            }
            let readback = PublicationPointReadback {
                point_id: actual.address,
                // `readback_indexed_points_exact` verifies the full S11 identity
                // against this exact request before it returns the point.
                identity: owner_expected.identity().clone(),
                payload: actual.point.payload,
                vectors,
            };
            if !point_matches(owner_expected, &readback) {
                all_match = false;
            }
            points.push(readback);
        }
        let missing = observed.missing_ids;
        if missing.iter().any(|id| !requested_set.contains(id)) {
            return Err(AdapterError::InvalidBinding);
        }
        let sorted = points
            .windows(2)
            .all(|pair| pair[0].point_id < pair[1].point_id);
        all_match &= sorted;
        let stage_not_applied = command.effect() == PublicationEffectKind::StageNewPoints
            && points.is_empty()
            && missing.len() == expected.len()
            && unexpected.is_empty();
        let close_not_applied = close_not_applied && all_match && missing.is_empty();
        let outcome = if stage_not_applied || close_not_applied {
            PublicationEffectOutcome::NotApplied
        } else if all_match && missing.is_empty() && unexpected.is_empty() {
            PublicationEffectOutcome::Applied
        } else {
            PublicationEffectOutcome::Conflict
        };
        Ok((points, missing, unexpected, outcome))
    }

    #[allow(clippy::too_many_lines)] // Keeps the guarded CAS and exact recovery adjacent.
    fn commit_visible(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        self.check_context(context)?;
        if self.verified_points.is_none() {
            // Reconcile a retained C02 CAS by its original stable identity before
            // refusing recovery. The current model command does not carry the
            // full point expectations needed for a fresh Qdrant recovery read;
            // an observed C02 commit therefore remains unknown until the model
            // supplies those admitted expectations again.
            let checkpoint = self.read_checkpoint(context)?;
            if let Some(publication) = checkpoint.publication_receipt()
                && publication.target_epoch == command.binding().target_epoch
            {
                let operation_id = Self::c02_mutation_id(command.binding(), "visible-epoch")?;
                let command_digest = visibility_command_digest(
                    command.binding(),
                    self.publication_receipt_id,
                    publication.readback_digest,
                )?;
                let _reconciled = self
                    .control
                    .read_publication_operation_receipt_with_original_context(
                        operation_id,
                        command_digest,
                        self.deadline,
                        self.work,
                    )
                    .map_err(|_| AdapterError::ControlUnavailable)?;
            }
            return Err(AdapterError::ReadbackUnknown);
        }
        let verified = self
            .verified_points
            .as_ref()
            .filter(|verified| same_transaction_inputs(&verified.binding, command.binding()))
            .ok_or(AdapterError::ReadbackUnknown)?;
        let binding = command.binding();
        let checkpoint = self.read_checkpoint(context)?;
        let projection = self.read_recovery_projection(&checkpoint)?;
        let intent = checkpoint
            .intent()
            .cloned()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, &intent)?;
        if intent.state != PublicationIntentState::ReadbackVerified
            || projection.visibility().visible_epoch != binding.previous_visible_epoch
            || projection.visibility().guards != binding.guards
            || projection.visibility().collection_generation_id
                != binding.scope.collection_generation_id
            || projection.visibility().schema_identity_digest != binding.scope.schema_identity
            || projection.last_reserved_epoch() != binding.target_epoch
        {
            return Err(AdapterError::InvalidBinding);
        }
        let membership = binding.scope.projection_membership_id;
        let previous_manifest = projection.manifests().get(&membership).cloned();
        if previous_manifest.is_some() != binding.old_manifest.is_some()
            || projection.shadows().contains_key(&membership)
        {
            return Err(AdapterError::InvalidBinding);
        }
        let current_receipt = self
            .control
            .read_current_control_commit_with_original_context(self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)?;
        if current_receipt.after_generation != projection.generation() {
            return Err(AdapterError::ControlUnavailable);
        }
        let receipt_id = self.publication_receipt_id;
        let new_manifest_ref = self.c13.record.reference().clone();
        let retirement =
            binding.new_manifest.entries.is_empty() && !binding.difference.retire.is_empty();
        let change = PublicationManifestChange {
            membership_id: membership,
            previous_manifest,
            next_manifest: if retirement {
                None
            } else {
                Some(new_manifest_ref)
            },
            matching_shadow: None,
        };
        let evidence = PublicationReadbackEvidence {
            exact_new_manifest_ref: verified.new_manifest_ref.clone(),
            exact_retired_manifest_ref: verified.retired_manifest_ref.clone(),
            readback_digest: verified.readback_digest,
        };
        let operation_id = Self::c02_mutation_id(binding, "visible-epoch")?;
        let command_digest = visibility_command_digest(
            binding,
            self.publication_receipt_id,
            verified.readback_digest,
        )?;
        let request = VisibleEpochCommit::new(
            operation_id,
            command_digest,
            current_receipt,
            intent,
            *projection.visibility(),
            receipt_id,
            evidence,
            vec![change],
        )
        .and_then(|request| {
            if retirement {
                Ok(request)
            } else {
                request.with_prepared_artifact_set_v1(self.c13.record.clone())
            }
        })
        .map_err(|_| AdapterError::InvalidBinding)?;
        let mutation =
            control_mutation_identity(operation_id).map_err(|_| AdapterError::InvalidBinding)?;
        self.pending_visibility = Some(PendingVisibility {
            binding: binding.clone(),
            request: request.clone(),
            mutation: mutation.clone(),
        });

        self.commit_visible_with_recovery(&request, &mutation)
    }

    /// Uses C02's exact original command and condition recovery before dispatch,
    /// and authorizes at most one replay after an explicit `NotCommitted` result.
    fn commit_visible_with_recovery(
        &mut self,
        request: &VisibleEpochCommit,
        mutation: &MutationIdentity,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        match self
            .control
            .recover_visible_epoch_with_original_context(
                request,
                mutation,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            CommitRecoveryDecision::Committed(receipt) => return Ok(receipt),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => {}
            CommitRecoveryDecision::ConflictingInput
            | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                return Err(AdapterError::ControlUnavailable);
            }
        }

        self.owner_checkpoint()?;
        if let Ok(receipt) = self.control.commit_visible_epoch_with_original_context(
            request,
            mutation,
            self.deadline,
            self.work,
        ) {
            return Ok(receipt);
        }

        match self
            .control
            .recover_visible_epoch_with_original_context(
                request,
                mutation,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
        {
            CommitRecoveryDecision::Committed(receipt) => Ok(receipt),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => {
                self.owner_checkpoint()?;
                if let Ok(receipt) = self.control.commit_visible_epoch_with_original_context(
                    request,
                    mutation,
                    self.deadline,
                    self.work,
                ) {
                    return Ok(receipt);
                }
                match self
                    .control
                    .recover_visible_epoch_with_original_context(
                        request,
                        mutation,
                        self.deadline,
                        self.work,
                    )
                    .map_err(|_| AdapterError::ControlUnavailable)?
                {
                    CommitRecoveryDecision::Committed(receipt) => Ok(receipt),
                    CommitRecoveryDecision::NotCommittedRetrySameOperation
                    | CommitRecoveryDecision::ConflictingInput
                    | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                        Err(AdapterError::ControlUnavailable)
                    }
                }
            }
            CommitRecoveryDecision::ConflictingInput
            | CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                Err(AdapterError::ControlUnavailable)
            }
        }
    }

    fn read_visible_commit(
        &self,
        binding: &PublicationEffectBinding,
        visibility_operation: &OpaqueId,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
        verified: &VerifiedPoints,
    ) -> Result<ControlCommitReceipt, AdapterError> {
        let projection = self.read_recovery_projection(checkpoint)?;
        let publication = checkpoint
            .publication_receipt()
            .ok_or(AdapterError::ControlUnavailable)?;
        let expected_receipt_id = self.publication_receipt_id;
        if publication.publication_receipt_id != expected_receipt_id
            || publication.target_epoch != binding.target_epoch
            || publication.readback_digest != verified.readback_digest
            || publication.exact_new_manifest_ref != verified.new_manifest_ref
            || publication.exact_retired_manifest_ref != verified.retired_manifest_ref
            || publication.control_commit_revision.get() != projection.generation()
            || projection.visibility().last_receipt != Some(expected_receipt_id)
            || projection.visibility().visible_epoch != binding.target_epoch
            || projection.visibility().collection_generation_id
                != binding.scope.collection_generation_id
            || projection.visibility().schema_identity_digest != binding.scope.schema_identity
            || projection.visibility().guards != binding.guards
        {
            return Err(AdapterError::InvalidBinding);
        }
        let membership = binding.scope.projection_membership_id;
        let expected_manifest =
            if binding.new_manifest.entries.is_empty() && !binding.difference.retire.is_empty() {
                None
            } else {
                Some(verified.new_manifest_ref.clone())
            };
        if projection.manifests().get(&membership).cloned() != expected_manifest
            || projection.shadows().contains_key(&membership)
        {
            return Err(AdapterError::InvalidBinding);
        }
        let operation_id =
            Self::c02_mutation_id_for_operation(binding, visibility_operation, "visible-epoch")?;
        let command_digest = visibility_command_digest_for_operation(
            binding,
            visibility_operation,
            self.publication_receipt_id,
            verified.readback_digest,
        )?;
        let by_id = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        let by_generation = self
            .control
            .read_control_commit_at_generation_with_original_context(
                publication.control_commit_revision.get(),
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        if by_id != by_generation || by_id.after_generation != checkpoint.generation() {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(by_id)
    }

    fn read_control_effect(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        self.check_context(context)?;
        self.validate_c13(command.binding())?;
        match command.effect() {
            PublicationEffectKind::PersistIntent => self.read_intent_effect(command, context),
            PublicationEffectKind::StageNewPoints
            | PublicationEffectKind::CloseOldPoints
            | PublicationEffectKind::VerifyReadback => self.read_point_effect(command, context),
            PublicationEffectKind::Compensate => self.read_compensation_effect(command, context),
            PublicationEffectKind::CommitVisibleEpoch => {
                self.read_visibility_effect(command, context)
            }
            PublicationEffectKind::PublishControlSnapshot => {
                self.read_snapshot_effect(command, context)
            }
            PublicationEffectKind::CommitAbortFinalization => {
                self.read_abort_finalization_effect(command, context)
            }
            PublicationEffectKind::PublishAbortSnapshot => {
                self.read_abort_snapshot_effect(command, context)
            }
            PublicationEffectKind::Abandon => Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            )),
        }
    }

    fn read_intent_effect(
        &self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        self.read_candidate_record()?;
        let binding = command.binding();
        let checkpoint = self.read_checkpoint(context)?;
        let projection = self.read_recovery_projection(&checkpoint)?;
        if let Some(intent) = checkpoint.intent() {
            self.expected_c02_intent(binding, intent)?;
            let original_record = checkpoint
                .original_binding()
                .ok_or(AdapterError::ControlUnavailable)?;
            let (original_binding, intent_receipt) =
                self.original_intent_binding(binding, intent, original_record)?;
            if original_binding.operation_id != binding.operation_id {
                return Err(AdapterError::InvalidBinding);
            }
            if intent.state == PublicationIntentState::Aborted {
                return self.read_aborted_intent_effect(
                    command,
                    binding,
                    &original_binding,
                    intent,
                    &intent_receipt,
                    &checkpoint,
                );
            }
            let mut observation = empty_observation(
                command.effect(),
                binding.clone(),
                PublicationEffectOutcome::Applied,
            );
            observation.intent = Some(PublicationIntentReadback {
                binding: original_binding,
                recorded_phase: model_phase(intent.state).ok_or(AdapterError::InvalidBinding)?,
                control_generation: intent_receipt.after_generation,
                control_state_digest: intent_digest(intent)?,
            });
            return Ok(observation);
        }
        if checkpoint.original_binding().is_some() || checkpoint.abort_finalization().is_some() {
            return Err(AdapterError::InvalidBinding);
        }
        let operation_id = Self::c02_mutation_id(binding, "intent-durable")?;
        let command_digest = self.c02_command_digest(
            binding,
            "intent-durable",
            PublicationIntentState::IntentDurable,
            None,
        )?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let predecessor_matches = projection.visibility().visible_epoch
            == binding.previous_visible_epoch
            && projection.visibility().guards == binding.guards
            && projection.visibility().collection_generation_id
                == binding.scope.collection_generation_id
            && projection.visibility().schema_identity_digest == binding.scope.schema_identity
            && projection.last_reserved_epoch() < binding.target_epoch
            && projection
                .manifests()
                .get(&binding.scope.projection_membership_id)
                .is_some()
                == binding.old_manifest.is_some();
        let previous_is_committed = checkpoint.intent().is_none_or(|previous| {
            previous.state == PublicationIntentState::ControlCommitted
                && previous.target_epoch == binding.previous_visible_epoch
                && previous.owner_source_membership_access_guards == binding.guards
        });
        let outcome = if receipt.is_none() && predecessor_matches && previous_is_committed {
            PublicationEffectOutcome::NotApplied
        } else {
            PublicationEffectOutcome::Conflict
        };
        Ok(empty_observation(
            command.effect(),
            binding.clone(),
            outcome,
        ))
    }

    fn read_aborted_intent_effect(
        &self,
        command: &PublicationEffectCommand,
        binding: &PublicationEffectBinding,
        original_binding: &PublicationEffectBinding,
        intent: &PublicationIntent,
        intent_receipt: &ControlCommitReceipt,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let Some(record) = checkpoint.abort_finalization() else {
            return Ok(empty_observation(
                command.effect(),
                binding.clone(),
                PublicationEffectOutcome::Unknown,
            ));
        };
        let metadata = record.metadata();
        let abort_binding = self.rehydrate_abort_binding(binding, metadata, checkpoint)?;
        let abort_receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                metadata.c02_mutation_id,
                metadata.c02_command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let mut original = intent.clone();
        original.state = PublicationIntentState::IntentDurable;
        let original_digest = intent_digest(&original)?;
        let original_metadata = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?
            .metadata();
        if !abort_record_matches_intent(record.metadata(), intent)
            || intent_receipt.after_generation != metadata.original_control_generation
            || intent_receipt.operation_id != original_metadata.c02_mutation_id
            || intent_receipt.command_digest != original_metadata.c02_command_digest
            || metadata.original_persist_operation_id != original_metadata.original_operation_id
            || metadata.original_request_id != original_metadata.original_request_id
            || metadata.original_relative_deadline_ms
                != original_metadata.original_relative_deadline_ms
            || metadata.original_budget_ref != original_metadata.original_budget_ref
            || original_metadata.expected_control_generation.checked_add(1)
                != Some(metadata.original_control_generation)
            || original_digest != metadata.original_control_state_digest
            || abort_request_digest_from_metadata(metadata)? != metadata.abort_request_digest
            || abort_finalization_command_digest_from_metadata(binding, metadata)?
                != metadata.c02_command_digest
            || abort_receipt.as_ref().is_none_or(|receipt| {
                receipt.operation_id != metadata.c02_mutation_id
                    || receipt.command_digest != metadata.c02_command_digest
                    || receipt.before_generation != metadata.expected_control_generation
                    || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
            })
        {
            return Ok(empty_observation(
                command.effect(),
                binding.clone(),
                PublicationEffectOutcome::Unknown,
            ));
        }
        let mut observation = empty_observation(
            command.effect(),
            original_binding.clone(),
            PublicationEffectOutcome::Applied,
        );
        observation.intent = Some(PublicationIntentReadback {
            binding: original_binding.clone(),
            recorded_phase: PublicationPhase::Aborted,
            control_generation: metadata.original_control_generation,
            control_state_digest: metadata.original_control_state_digest,
        });
        // A restart PersistIntent readback has no typed AbortFinalizationRequest
        // in its command. Keep this absent until the model exposes an untrusted
        // persisted-request observation API; never fabricate its private DurableIntent.
        observation.abort_request = command.abort_request().cloned();
        observation.abort_binding = Some(abort_binding);
        Ok(observation)
    }
    fn read_point_effect(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let head = self.read_head()?;
        let intent = head
            .intent
            .as_ref()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(command.binding(), intent)?;
        let allowed_phase = match command.effect() {
            PublicationEffectKind::StageNewPoints => {
                intent.state == PublicationIntentState::IntentDurable
            }
            PublicationEffectKind::CloseOldPoints => {
                intent.state == PublicationIntentState::NewPointsAcknowledged
            }
            PublicationEffectKind::VerifyReadback => matches!(
                intent.state,
                PublicationIntentState::NewPointsAcknowledged
                    | PublicationIntentState::OldPointsClosedAcknowledged
                    | PublicationIntentState::ReadbackVerified
            ),
            _ => return Err(AdapterError::PointEffectUnavailable),
        };
        if !allowed_phase {
            return Err(AdapterError::InvalidBinding);
        }
        let (points, missing, unexpected, outcome) =
            self.qdrant_readback(command, command.points())?;
        if command.effect() == PublicationEffectKind::VerifyReadback
            && outcome == PublicationEffectOutcome::Applied
        {
            if intent.state == PublicationIntentState::NewPointsAcknowledged {
                self.advance_intent(
                    command,
                    PublicationIntentState::OldPointsClosedAcknowledged,
                    context,
                )?;
            }
            let checkpoint = self.read_checkpoint(context)?;
            let projection = self.read_recovery_projection(&checkpoint)?;
            let old_ref = projection
                .manifests()
                .get(&command.binding().scope.projection_membership_id)
                .cloned();
            let readback_digest = point_readback_digest(command.binding(), &points)?;
            let retired_manifest_ref = match old_ref {
                Some(reference) => reference,
                None => ReceiptRef::new(format!("publication-empty-retired:v1:{readback_digest}"))
                    .map_err(|_| AdapterError::InvalidBinding)?,
            };
            self.verified_points = Some(VerifiedPoints {
                binding: command.binding().clone(),
                visibility_operation_id: None,
                readback_digest,
                new_manifest_ref: self.c13.record.reference().clone(),
                retired_manifest_ref,
            });
        }
        Ok(PublicationEffectReadback {
            effect: command.effect(),
            binding: command.binding().clone(),
            outcome,
            points,
            missing_point_ids: missing,
            unexpected_point_ids: unexpected,
            intent: None,
            control: None,
            snapshot: None,
            abandonment: None,
            abort_request: None,
            abort_binding: None,
        })
    }

    #[allow(clippy::too_many_lines)] // One exact CAS reconciliation path keeps the C02 fields adjacent.
    fn read_visibility_effect(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let Some(verified) = self
            .verified_points
            .as_ref()
            .filter(|verified| same_transaction_inputs(&verified.binding, command.binding()))
        else {
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            ));
        };
        let operation_id = Self::c02_mutation_id(command.binding(), "visible-epoch")?;
        let command_digest = visibility_command_digest(
            command.binding(),
            self.publication_receipt_id,
            verified.readback_digest,
        )?;
        if let Some(pending) = self
            .pending_visibility
            .as_ref()
            .filter(|pending| pending.binding == *command.binding())
        {
            match self.control.recover_visible_epoch_with_original_context(
                &pending.request,
                &pending.mutation,
                self.deadline,
                self.work,
            ) {
                Ok(
                    CommitRecoveryDecision::Committed(_)
                    | CommitRecoveryDecision::NotCommittedRetrySameOperation,
                ) => {}
                _ => {
                    return Ok(empty_observation(
                        command.effect(),
                        command.binding().clone(),
                        PublicationEffectOutcome::Unknown,
                    ));
                }
            }
        }
        let operation_receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let checkpoint = self.read_checkpoint(context)?;
        let projection = self.read_recovery_projection(&checkpoint)?;
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        let retirement = command.binding().new_manifest.entries.is_empty()
            && !command.binding().difference.retire.is_empty();
        let expected_manifest_ref = if retirement {
            None
        } else {
            Some(self.c13.record.reference().clone())
        };
        let actual_manifest_ref = projection
            .manifests()
            .get(&command.binding().scope.projection_membership_id);
        let manifest_matches = match (&expected_manifest_ref, actual_manifest_ref) {
            (Some(expected), Some(actual)) => expected == actual,
            (None, None) => true,
            _ => false,
        };
        let receipt = if operation_receipt.is_some()
            && intent.state == PublicationIntentState::ControlCommitted
            && manifest_matches
        {
            self.read_visible_commit(
                command.binding(),
                &command.binding().operation_id,
                &checkpoint,
                verified,
            )
            .ok()
        } else {
            None
        };
        if receipt.is_none() {
            let prior_unchanged = intent.state == PublicationIntentState::ReadbackVerified
                && projection.visibility().visible_epoch
                    == command.binding().previous_visible_epoch
                && projection.visibility().guards == command.binding().guards
                && projection.visibility().collection_generation_id
                    == command.binding().scope.collection_generation_id;
            let checkpoint_operation = Self::c02_mutation_id(
                command.binding(),
                intent_state_label(PublicationIntentState::ReadbackVerified),
            )?;
            let original_binding =
                self.bind_original_intent_context(command.binding(), &checkpoint)?;
            let checkpoint_digest = self.c02_command_digest(
                &original_binding,
                intent_state_label(PublicationIntentState::ReadbackVerified),
                PublicationIntentState::ReadbackVerified,
                None,
            )?;
            let checkpoint_receipt = self
                .control
                .read_publication_operation_receipt_with_original_context(
                    checkpoint_operation,
                    checkpoint_digest,
                    self.deadline,
                    self.work,
                )
                .map_err(|_| AdapterError::ControlUnavailable)?;
            let current_receipt = self
                .control
                .read_current_control_commit_with_original_context(self.deadline, self.work)
                .map_err(|_| AdapterError::ControlUnavailable)?;
            let no_intervening_control_write =
                checkpoint_receipt
                    .as_ref()
                    .is_some_and(|checkpoint_receipt| {
                        checkpoint_receipt.after_generation == projection.generation()
                            && current_receipt.after_generation == projection.generation()
                    });
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                if prior_unchanged && operation_receipt.is_none() && no_intervening_control_write {
                    PublicationEffectOutcome::NotApplied
                } else {
                    PublicationEffectOutcome::Unknown
                },
            ));
        }
        let receipt = receipt.ok_or(AdapterError::ControlUnavailable)?;
        let visible_manifest_digest = if retirement {
            None
        } else {
            Some(command.binding().new_manifest_digest)
        };
        let control = PublicationControlReadback {
            before_visible_epoch: command.binding().previous_visible_epoch,
            after_visible_epoch: projection.visibility().visible_epoch,
            last_reserved_epoch: projection.last_reserved_epoch(),
            guards: command.binding().guards,
            before_control_generation: receipt.before_generation,
            after_control_generation: receipt.after_generation,
            control_state_digest: visibility_digest(projection.visibility())?,
            collection_generation_id: projection.visibility().collection_generation_id,
            previous_visible_manifest_digest: command.binding().old_manifest_digest,
            visible_manifest_digest,
        };
        let mut observation = empty_observation(
            command.effect(),
            command.binding().clone(),
            PublicationEffectOutcome::Applied,
        );
        observation.control = Some(control);
        self.verified_points
            .as_mut()
            .ok_or(AdapterError::ReadbackUnknown)?
            .visibility_operation_id = Some(command.binding().operation_id.clone());
        Ok(observation)
    }

    fn read_snapshot_effect(
        &self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let checkpoint = self.read_checkpoint(context)?;
        let projection = self.read_recovery_projection(&checkpoint)?;
        let Some(verified) = self
            .verified_points
            .as_ref()
            .filter(|verified| same_transaction_inputs(&verified.binding, command.binding()))
        else {
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            ));
        };
        let visibility_operation = verified
            .visibility_operation_id
            .as_ref()
            .ok_or(AdapterError::ReadbackUnknown)?;
        let commit = self.read_visible_commit(
            command.binding(),
            visibility_operation,
            &checkpoint,
            verified,
        )?;
        let snapshot = match self.control.current_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error)
                if *error.reason()
                    == search_control_redb::ControlError::SnapshotPublicationFailed =>
            {
                return Ok(empty_observation(
                    command.effect(),
                    command.binding().clone(),
                    PublicationEffectOutcome::NotApplied,
                ));
            }
            Err(_) => return Err(AdapterError::ControlUnavailable),
        };
        let expected_manifest = if command.binding().new_manifest.entries.is_empty()
            && !command.binding().difference.retire.is_empty()
        {
            None
        } else {
            Some(command.binding().new_manifest_digest)
        };
        let actual_manifest_ref = projection
            .manifests()
            .get(&command.binding().scope.projection_membership_id);
        let expected_manifest_ref = if expected_manifest.is_some() {
            Some(self.c13.record.reference())
        } else {
            None
        };
        let manifest_matches = match (expected_manifest_ref, actual_manifest_ref) {
            (Some(expected), Some(actual)) => expected == actual,
            (None, None) => true,
            _ => false,
        };
        if snapshot.generation != commit.after_generation
            || projection.generation() != commit.after_generation
            || projection.visibility().visible_epoch != command.binding().target_epoch
            || projection.visibility().guards != command.binding().guards
            || projection.visibility().collection_generation_id
                != command.binding().scope.collection_generation_id
            || !manifest_matches
        {
            return Ok(empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::NotApplied,
            ));
        }
        let snapshot_digest = control_snapshot_digest(&snapshot, self.control.limits(), self.work)?;
        let process_snapshot_id = snapshot_process_id(&snapshot, snapshot_digest)?;
        let mut observation = empty_observation(
            command.effect(),
            command.binding().clone(),
            PublicationEffectOutcome::Applied,
        );
        observation.snapshot = Some(PublicationSnapshotReadback {
            process_snapshot_id,
            visible_epoch: checkpoint.visibility().visible_epoch,
            control_generation: snapshot.generation,
            guards: checkpoint.visibility().guards,
            collection_generation_id: checkpoint.visibility().collection_generation_id,
            visible_manifest_digest: expected_manifest,
            snapshot_digest,
        });
        Ok(observation)
    }

    fn apply(
        &mut self,
        command: &PublicationEffectCommand,
        identity: &MutationIdentity,
        context: &OperationContext<C>,
    ) -> Result<(), AdapterError> {
        self.check_context(context)?;
        if identity.operation_id != command.binding().operation_id {
            return Err(AdapterError::InvalidBinding);
        }
        self.validate_c13(command.binding())?;
        match command.effect() {
            PublicationEffectKind::PersistIntent => {
                self.persist_or_reconcile_intent(command, context)?;
                Ok(())
            }
            PublicationEffectKind::Compensate => self.compensate_points(command, context),
            PublicationEffectKind::StageNewPoints => {
                self.require_intent_phase(
                    command.binding(),
                    PublicationIntentState::IntentDurable,
                )?;
                let points = Self::canonical_inputs(command.points())?;
                self.owner_checkpoint()?;
                self.plane
                    .upsert_indexed_exact(&self.route, &points, &self.context)
                    .map_err(|_| AdapterError::ReadbackUnknown)?;
                Ok(())
            }
            PublicationEffectKind::CloseOldPoints => {
                self.advance_intent(
                    command,
                    PublicationIntentState::NewPointsAcknowledged,
                    context,
                )?;
                let points = Self::canonical_inputs(command.original_points())?;
                self.owner_checkpoint()?;
                self.plane
                    .close_indexed_exact(
                        &self.route,
                        &points,
                        command.binding().target_epoch,
                        &self.context,
                    )
                    .map_err(|_| AdapterError::ReadbackUnknown)?;
                Ok(())
            }
            PublicationEffectKind::CommitVisibleEpoch => {
                self.advance_intent(command, PublicationIntentState::ReadbackVerified, context)?;
                self.commit_visible(command, context)?;
                Ok(())
            }
            PublicationEffectKind::PublishControlSnapshot => {
                self.publish_snapshot(command, context)?;
                Ok(())
            }
            PublicationEffectKind::CommitAbortFinalization => {
                self.commit_abort_finalization(command, context)?;
                Ok(())
            }
            PublicationEffectKind::PublishAbortSnapshot => {
                self.publish_abort_snapshot(command, context)?;
                Ok(())
            }
            PublicationEffectKind::Abandon => Err(AdapterError::AbandonmentUnavailable),
            // VerifyReadback is read-only. Abandonment requires the separate
            // membership-wide fence owner and is not synthesized here.
            PublicationEffectKind::VerifyReadback => Err(AdapterError::PointEffectUnavailable),
        }
    }

    fn abort_finalization_record(
        &self,
        binding: &PublicationEffectBinding,
        request: &AbortFinalizationRequest,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
    ) -> Result<
        (
            MutationId,
            Blake3Digest32,
            PublicationAbortFinalizationRecordV1,
        ),
        AdapterError,
    > {
        let mut abort_binding = binding.clone();
        abort_binding.operation_id.clone_from(&request.operation_id);
        validate_abort_request_binding(&abort_binding, request)?;
        self.read_candidate_record()?;
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        let projection = self.read_recovery_projection(checkpoint)?;
        self.expected_c02_intent(binding, intent)?;
        let original_binding =
            self.validate_abort_original_intent(binding, request, checkpoint, intent)?;
        let original_record = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?;
        let original_metadata = original_record.metadata();
        if original_metadata.original_relative_deadline_ms != original_binding.relative_deadline_ms
            || original_metadata.original_budget_ref != original_binding.budget_ref
        {
            return Err(AdapterError::InvalidBinding);
        }
        let old_manifest_reference = projection
            .manifests()
            .get(&binding.scope.projection_membership_id)
            .cloned();
        if old_manifest_reference.is_some() != binding.old_manifest.is_some() {
            return Err(AdapterError::InvalidBinding);
        }
        let (resolution_operation_id, resolution) = abort_resolution_metadata(&request.resolution)?;
        let scope_digest = abort_scope_digest(&binding.scope)?;
        let abort_request_digest =
            abort_request_digest(&abort_binding, request, original_metadata, scope_digest)?;
        let operation_id = Self::c02_mutation_id(&abort_binding, "abort-finalization")?;
        let command_digest =
            abort_finalization_command_digest(binding, request, abort_request_digest)?;
        let metadata = PublicationAbortFinalizationMetadataV1 {
            abort_operation_id: request.operation_id.clone(),
            c02_mutation_id: operation_id,
            c02_command_digest: command_digest,
            expected_control_generation: request.expected_control_generation,
            abort_request_digest,
            publication_intent_id: intent.publication_intent_id,
            transaction_id: binding.transaction_id.clone(),
            collection_generation_id: binding.scope.collection_generation_id,
            projection_membership_id: binding.scope.projection_membership_id,
            schema_identity_digest: binding.scope.schema_identity,
            target_epoch: binding.target_epoch,
            previous_visible_epoch: binding.previous_visible_epoch,
            old_manifest_digest: binding.old_manifest_digest,
            new_manifest_digest: binding.new_manifest_digest,
            old_manifest_reference,
            new_manifest_reference: self.c13.record.reference().clone(),
            scope_digest,
            prepared_input_digest: binding.prepared_input_digest,
            original_persist_operation_id: request.intent.persist_operation_id().clone(),
            original_request_id: original_metadata.original_request_id,
            original_relative_deadline_ms: original_metadata.original_relative_deadline_ms,
            original_budget_ref: original_metadata.original_budget_ref.clone(),
            abort_request_id: binding.request_id,
            abort_relative_deadline_ms: binding.relative_deadline_ms,
            abort_budget_ref: binding.budget_ref.clone(),
            original_control_generation: request.intent.control_generation(),
            original_control_state_digest: request.intent.control_state_digest(),
            original_guards: binding.guards,
            current_guards: request.current_guards,
            resolution_operation_id,
            resolution,
        };
        let record = PublicationAbortFinalizationRecordV1::prepare_with_original_context(
            metadata,
            self.control.limits(),
            self.deadline,
            self.work,
        )
        .map_err(|_| AdapterError::ControlUnavailable)?;
        Ok((operation_id, command_digest, record))
    }

    fn validate_abort_checkpoint(
        &self,
        binding: &PublicationEffectBinding,
        request: &AbortFinalizationRequest,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
        exact_current_guards: bool,
    ) -> Result<(), AdapterError> {
        let mut abort_binding = binding.clone();
        abort_binding.operation_id.clone_from(&request.operation_id);
        validate_abort_request_binding(&abort_binding, request)?;
        let projection = self.read_recovery_projection(checkpoint)?;
        let visibility = projection.visibility();
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, intent)?;
        self.validate_abort_original_intent(binding, request, checkpoint, intent)?;
        let guards_valid = if exact_current_guards {
            visibility.guards == request.current_guards
        } else {
            abort_guards_are_current_or_newer(visibility.guards, binding.guards)
        };
        if visibility.visible_epoch != request.previous_visible_epoch
            || projection.last_reserved_epoch() != request.intent.target_epoch()
            || visibility.collection_generation_id != request.collection_generation_id
            || visibility.schema_identity_digest != binding.scope.schema_identity
            || !guards_valid
            || projection
                .manifests()
                .get(&binding.scope.projection_membership_id)
                .is_some()
                != binding.old_manifest.is_some()
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(())
    }

    fn validate_abort_request_record(
        &self,
        binding: &PublicationEffectBinding,
        request: &AbortFinalizationRequest,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
        record: &PublicationAbortFinalizationRecordV1,
    ) -> Result<(MutationId, Blake3Digest32, PublicationEffectBinding), AdapterError> {
        let metadata = record.metadata();
        let abort_binding = self.rehydrate_abort_binding(binding, metadata, checkpoint)?;
        validate_abort_request_binding(&abort_binding, request)?;
        let scope_digest = abort_scope_digest(&abort_binding.scope)?;
        let original_metadata = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?
            .metadata();
        let request_digest =
            abort_request_digest(&abort_binding, request, original_metadata, scope_digest)?;
        let (resolution_operation_id, resolution) = abort_resolution_metadata(&request.resolution)?;
        let operation_id = Self::c02_mutation_id(&abort_binding, "abort-finalization")?;
        let command_digest =
            abort_finalization_command_digest(&abort_binding, request, request_digest)?;
        if metadata.abort_operation_id != request.operation_id
            || metadata.c02_mutation_id != operation_id
            || metadata.c02_command_digest != command_digest
            || metadata.expected_control_generation != request.expected_control_generation
            || metadata.abort_request_digest != request_digest
            || metadata.abort_request_id != abort_binding.request_id
            || metadata.abort_relative_deadline_ms != abort_binding.relative_deadline_ms
            || metadata.abort_budget_ref != abort_binding.budget_ref
            || request.intent.persist_operation_id() != &metadata.original_persist_operation_id
            || request.intent.request_id() != metadata.original_request_id
            || request.intent.control_generation() != metadata.original_control_generation
            || request.intent.control_state_digest() != metadata.original_control_state_digest
            || metadata.resolution_operation_id != resolution_operation_id
            || metadata.resolution != resolution
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok((operation_id, command_digest, abort_binding))
    }
    fn rehydrate_abort_binding(
        &self,
        binding: &PublicationEffectBinding,
        metadata: &PublicationAbortFinalizationMetadataV1,
        checkpoint: &search_control_redb::PublicationRecoveryCheckpoint,
    ) -> Result<PublicationEffectBinding, AdapterError> {
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        let original_record = checkpoint
            .original_binding()
            .ok_or(AdapterError::ControlUnavailable)?;
        let projection = self.read_recovery_projection(checkpoint)?;
        let (original_binding, original_receipt) =
            self.original_intent_binding(binding, intent, original_record)?;
        let scope_digest = abort_scope_digest(&original_binding.scope)?;
        let mut current_static_binding = binding.clone();
        current_static_binding
            .operation_id
            .clone_from(&original_binding.operation_id);
        current_static_binding.request_id = original_binding.request_id;
        current_static_binding.relative_deadline_ms = original_binding.relative_deadline_ms;
        current_static_binding
            .budget_ref
            .clone_from(&original_binding.budget_ref);
        let mut original_intent = intent.clone();
        original_intent.state = PublicationIntentState::IntentDurable;
        if metadata.original_persist_operation_id != original_binding.operation_id {
            return Err(AdapterError::InvalidBinding);
        }
        if metadata.transaction_id != original_binding.transaction_id
            || metadata.abort_operation_id == metadata.original_persist_operation_id
            || metadata.collection_generation_id != original_binding.scope.collection_generation_id
            || metadata.projection_membership_id != original_binding.scope.projection_membership_id
            || metadata.schema_identity_digest != original_binding.scope.schema_identity
            || metadata.target_epoch != original_binding.target_epoch
            || metadata.previous_visible_epoch != original_binding.previous_visible_epoch
            || metadata.old_manifest_digest != original_binding.old_manifest_digest
            || metadata.new_manifest_digest != original_binding.new_manifest_digest
            || metadata.scope_digest != scope_digest
            || metadata.prepared_input_digest != original_binding.prepared_input_digest
            || metadata.original_request_id != original_binding.request_id
            || metadata.original_relative_deadline_ms != original_binding.relative_deadline_ms
            || metadata.original_budget_ref != original_binding.budget_ref
            || metadata.original_control_generation != original_receipt.after_generation
            || metadata.original_control_state_digest != intent_digest(&original_intent)?
            || metadata.original_guards != original_binding.guards
            || current_static_binding != original_binding
            || projection.visibility().collection_generation_id != metadata.collection_generation_id
            || projection.visibility().schema_identity_digest != metadata.schema_identity_digest
            || projection.visibility().visible_epoch != metadata.previous_visible_epoch
            || projection.visibility().guards != metadata.current_guards
            || projection.last_reserved_epoch() != metadata.target_epoch
            || projection
                .manifests()
                .get(&metadata.projection_membership_id)
                != metadata.old_manifest_reference.as_ref()
            || checkpoint.original_binding().is_none_or(|record| {
                !record.matches_intent(intent)
                    || record.metadata().original_operation_id
                        != metadata.original_persist_operation_id
                    || record.metadata().original_request_id != metadata.original_request_id
                    || record.metadata().original_relative_deadline_ms
                        != metadata.original_relative_deadline_ms
                    || record.metadata().original_budget_ref != metadata.original_budget_ref
            })
        {
            return Err(AdapterError::InvalidBinding);
        }
        if intent.state != PublicationIntentState::Aborted
            || !abort_record_matches_intent(metadata, intent)
            || intent.prepared_manifest_ref != metadata.new_manifest_reference
            || self.c13.record.reference() != &metadata.new_manifest_reference
        {
            return Err(AdapterError::InvalidBinding);
        }
        let mut abort_binding = original_binding;
        abort_binding
            .operation_id
            .clone_from(&metadata.abort_operation_id);
        abort_binding.request_id = metadata.abort_request_id;
        abort_binding.relative_deadline_ms = metadata.abort_relative_deadline_ms;
        abort_binding
            .budget_ref
            .clone_from(&metadata.abort_budget_ref);
        Ok(abort_binding)
    }
    fn commit_abort_finalization(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<(), AdapterError> {
        self.check_context(context)?;
        let request = command
            .abort_request()
            .ok_or(AdapterError::InvalidBinding)?;
        let binding = command.binding();
        self.validate_c13(binding)?;
        let checkpoint = self.read_checkpoint(context)?;
        let (operation_id, command_digest, record) =
            self.abort_finalization_record(binding, request, &checkpoint)?;
        let prior_receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        if let Some(receipt) = prior_receipt {
            let stored = checkpoint
                .abort_finalization()
                .ok_or(AdapterError::ControlUnavailable)?;
            let intent = checkpoint
                .intent()
                .ok_or(AdapterError::ControlUnavailable)?;
            if !abort_record_matches_intent(stored.metadata(), intent)
                || stored != &record
                || intent.state != PublicationIntentState::Aborted
                || stored.metadata().c02_mutation_id != operation_id
                || stored.metadata().c02_command_digest != command_digest
                || receipt.before_generation != request.expected_control_generation
                || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
            {
                return Err(AdapterError::InvalidBinding);
            }
            return Ok(());
        }

        if !Self::abort_attempt_matches_context(binding, context) {
            return Err(AdapterError::ContextMismatch);
        }
        self.validate_abort_checkpoint(binding, request, &checkpoint, true)?;
        if checkpoint.generation() != request.expected_control_generation {
            return Err(AdapterError::ControlUnavailable);
        }
        let previous = checkpoint
            .intent()
            .cloned()
            .ok_or(AdapterError::ControlUnavailable)?;
        if !matches!(
            previous.state,
            PublicationIntentState::Compensating | PublicationIntentState::PublicationBlocked
        ) {
            return Err(AdapterError::InvalidBinding);
        }
        let mutation =
            control_mutation_identity(operation_id).map_err(|_| AdapterError::InvalidBinding)?;
        let update = PublicationIntentUpdate::advance(
            operation_id,
            command_digest,
            request.expected_control_generation,
            previous,
            PublicationIntentState::Aborted,
        )
        .and_then(|update| update.with_prepared_artifact_set_v1(self.c13.record.clone()))
        .and_then(|update| update.with_abort_finalization_v1(record.clone()))
        .map_err(|_| AdapterError::InvalidBinding)?;
        let port_command = ControlPortCommand::PublicationIntent(update);
        let _receipt = self.transact_publication_with_recovery(&port_command, &mutation)?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ControlUnavailable)?;
        let actual = self.read_checkpoint(context)?;
        let stored = actual
            .abort_finalization()
            .ok_or(AdapterError::ControlUnavailable)?;
        let intent = actual.intent().ok_or(AdapterError::ControlUnavailable)?;
        if !abort_record_matches_intent(stored.metadata(), intent)
            || stored != &record
            || intent.state != PublicationIntentState::Aborted
            || stored.metadata().c02_mutation_id != operation_id
            || stored.metadata().c02_command_digest != command_digest
            || receipt.before_generation != request.expected_control_generation
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        {
            return Err(AdapterError::InvalidBinding);
        }
        Ok(())
    }
    fn read_abort_finalization_effect(
        &self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let request = command
            .abort_request()
            .ok_or(AdapterError::InvalidBinding)?;
        let binding = command.binding();
        let mut abort_binding = binding.clone();
        abort_binding.operation_id.clone_from(&request.operation_id);
        if abort_binding != *binding {
            return Err(AdapterError::InvalidBinding);
        }
        let checkpoint = self.read_checkpoint(context)?;
        let (operation_id, command_digest, expected_record) =
            self.abort_finalization_record(binding, request, &checkpoint)?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let stored = checkpoint.abort_finalization();
        let intent = checkpoint
            .intent()
            .ok_or(AdapterError::ControlUnavailable)?;
        let visibility = checkpoint.visibility();
        let old_manifest_digest = binding.old_manifest_digest;
        let control = PublicationControlReadback {
            before_visible_epoch: visibility.visible_epoch,
            after_visible_epoch: visibility.visible_epoch,
            last_reserved_epoch: checkpoint.last_reserved_epoch(),
            guards: visibility.guards,
            before_control_generation: receipt.as_ref().map_or_else(
                || checkpoint.generation(),
                |receipt| receipt.before_generation,
            ),
            after_control_generation: receipt.as_ref().map_or_else(
                || checkpoint.generation(),
                |receipt| receipt.after_generation,
            ),
            control_state_digest: visibility_digest(visibility)?,
            collection_generation_id: visibility.collection_generation_id,
            previous_visible_manifest_digest: old_manifest_digest,
            visible_manifest_digest: old_manifest_digest,
        };
        let outcome = match (receipt.as_ref(), stored) {
            (Some(receipt), Some(stored))
                if intent.state == PublicationIntentState::Aborted
                    && stored == &expected_record
                    && abort_record_matches_intent(stored.metadata(), intent)
                    && receipt.operation_id == operation_id
                    && receipt.command_digest == command_digest
                    && receipt.before_generation == request.expected_control_generation
                    && receipt.before_generation.checked_add(1)
                        == Some(receipt.after_generation)
                    && visibility.guards == request.current_guards
                    && visibility.visible_epoch == request.previous_visible_epoch =>
            {
                PublicationEffectOutcome::Applied
            }
            (None, None)
                if matches!(
                    intent.state,
                    PublicationIntentState::Compensating
                        | PublicationIntentState::PublicationBlocked
                ) && Self::abort_attempt_matches_context(binding, context)
                    && self
                        .validate_abort_checkpoint(binding, request, &checkpoint, false)
                        .is_ok() =>
            {
                PublicationEffectOutcome::NotApplied
            }
            (Some(_), _) | (_, Some(_)) => PublicationEffectOutcome::Conflict,
            _ => PublicationEffectOutcome::Unknown,
        };
        let mut observation = empty_observation(command.effect(), binding.clone(), outcome);
        observation.control = Some(control);
        observation.abort_request = Some(request.clone());
        observation.abort_binding = Some(abort_binding);
        Ok(observation)
    }

    fn publish_abort_snapshot(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<(), AdapterError> {
        self.check_context(context)?;
        let request = command
            .abort_request()
            .ok_or(AdapterError::InvalidBinding)?;
        let binding = command.binding();
        self.validate_c13(binding)?;
        let checkpoint = self.read_checkpoint(context)?;
        let record = checkpoint
            .abort_finalization()
            .ok_or(AdapterError::ReadbackUnknown)?;
        let (operation_id, command_digest, _) =
            self.validate_abort_request_record(binding, request, &checkpoint, record)?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?
            .ok_or(AdapterError::ReadbackUnknown)?;
        if checkpoint.generation() != receipt.after_generation
            || receipt.before_generation != request.expected_control_generation
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        {
            return Err(AdapterError::ReadbackUnknown);
        }
        self.control
            .recover_abort_snapshot(context)
            .map_err(|_| AdapterError::ReadbackUnknown)?;
        Ok(())
    }
    fn read_abort_snapshot_effect(
        &self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<PublicationEffectReadback, AdapterError> {
        let request = command
            .abort_request()
            .ok_or(AdapterError::InvalidBinding)?;
        let binding = command.binding();
        let checkpoint = self.read_checkpoint(context)?;
        let record = checkpoint
            .abort_finalization()
            .ok_or(AdapterError::ReadbackUnknown)?;
        let (operation_id, command_digest, abort_binding) =
            self.validate_abort_request_record(binding, request, &checkpoint, record)?;
        let receipt = self
            .control
            .read_publication_operation_receipt_with_original_context(
                operation_id,
                command_digest,
                self.deadline,
                self.work,
            )
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let Some(receipt) = receipt else {
            return Ok(empty_observation(
                command.effect(),
                binding.clone(),
                PublicationEffectOutcome::Unknown,
            ));
        };
        let snapshot = self
            .control
            .current_snapshot()
            .map_err(|_| AdapterError::ControlUnavailable)?;

        if checkpoint.abort_finalization() != Some(record)
            || checkpoint.intent().is_none_or(|intent| {
                intent.state != PublicationIntentState::Aborted
                    || !abort_record_matches_intent(record.metadata(), intent)
            })
            || checkpoint.generation() != receipt.after_generation
            || checkpoint.visibility().visible_epoch != request.previous_visible_epoch
            || checkpoint.visibility().guards != request.current_guards
            || checkpoint.visibility().collection_generation_id != request.collection_generation_id
            || snapshot.generation != receipt.after_generation
            || receipt.command_digest != command_digest
            || receipt.before_generation != request.expected_control_generation
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        {
            return Ok(empty_observation(
                command.effect(),
                binding.clone(),
                PublicationEffectOutcome::Unknown,
            ));
        }
        let snapshot_digest = control_snapshot_digest(&snapshot, self.control.limits(), self.work)?;
        let process_snapshot_id = snapshot_process_id(&snapshot, snapshot_digest)?;
        let mut observation = empty_observation(
            command.effect(),
            binding.clone(),
            PublicationEffectOutcome::Applied,
        );
        observation.snapshot = Some(PublicationSnapshotReadback {
            process_snapshot_id,
            visible_epoch: checkpoint.visibility().visible_epoch,
            control_generation: snapshot.generation,
            guards: checkpoint.visibility().guards,
            collection_generation_id: checkpoint.visibility().collection_generation_id,
            visible_manifest_digest: request.intent.old_manifest_digest(),
            snapshot_digest,
        });
        observation.abort_request = Some(request.clone());
        observation.abort_binding = Some(abort_binding);
        Ok(observation)
    }
    fn require_intent_phase(
        &self,
        binding: &PublicationEffectBinding,
        phase: PublicationIntentState,
    ) -> Result<(), AdapterError> {
        let head = self.read_head()?;
        let intent = head
            .intent
            .as_ref()
            .ok_or(AdapterError::ControlUnavailable)?;
        self.expected_c02_intent(binding, intent)?;
        if intent.state != phase {
            return Err(AdapterError::InvalidBinding);
        }
        self.read_candidate_record()
    }

    fn publish_snapshot(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> Result<(), AdapterError> {
        self.check_context(context)?;
        let checkpoint = self.read_checkpoint(context)?;
        let verified = self
            .verified_points
            .as_ref()
            .filter(|verified| same_transaction_inputs(&verified.binding, command.binding()))
            .ok_or(AdapterError::ReadbackUnknown)?;
        let visibility_operation = verified
            .visibility_operation_id
            .as_ref()
            .ok_or(AdapterError::ReadbackUnknown)?;
        let receipt = self.read_visible_commit(
            command.binding(),
            visibility_operation,
            &checkpoint,
            verified,
        )?;
        self.control
            .publish_committed_snapshot_with_original_context(&receipt, self.deadline, self.work)
            .map_err(|_| AdapterError::ControlUnavailable)?;
        let current = self
            .control
            .current_snapshot()
            .map_err(|_| AdapterError::ControlUnavailable)?;
        if current.generation != receipt.after_generation {
            return Err(AdapterError::ControlUnavailable);
        }
        Ok(())
    }
}

impl<C, F, E> PublicationEffectPort<C> for CanonicalPublicationEffectPort<'_, '_, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    fn apply(
        &mut self,
        command: &PublicationEffectCommand,
        identity: &MutationIdentity,
        context: &OperationContext<C>,
    ) -> PublicationMutationAttempt {
        let Ok(mut io) = self.io.try_borrow_mut() else {
            return PublicationMutationAttempt::Unknown;
        };
        match io.apply(command, identity, context) {
            Ok(()) => PublicationMutationAttempt::Acknowledged,
            Err(
                AdapterError::ContextMismatch
                | AdapterError::InvalidBinding
                | AdapterError::AbandonmentUnavailable
                | AdapterError::PointEffectUnavailable,
            ) => PublicationMutationAttempt::Rejected,
            Err(AdapterError::ControlUnavailable | AdapterError::ReadbackUnknown) => {
                PublicationMutationAttempt::Unknown
            }
        }
    }
}

impl<C, F, E> PublicationReadbackPort<C> for CanonicalPublicationReadbackPort<'_, '_, C, F, E>
where
    C: CancellationProbe + Clone,
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    fn readback(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> PublicationEffectReadback {
        let Ok(mut io) = self.io.try_borrow_mut() else {
            return empty_observation(
                command.effect(),
                command.binding().clone(),
                PublicationEffectOutcome::Unknown,
            );
        };
        io.read_control_effect(command, context)
            .unwrap_or_else(|_| {
                empty_observation(
                    command.effect(),
                    command.binding().clone(),
                    PublicationEffectOutcome::Unknown,
                )
            })
    }
}

const fn empty_observation(
    effect: PublicationEffectKind,
    binding: PublicationEffectBinding,
    outcome: PublicationEffectOutcome,
) -> PublicationEffectReadback {
    PublicationEffectReadback {
        effect,
        binding,
        outcome,
        points: Vec::new(),
        missing_point_ids: Vec::new(),
        unexpected_point_ids: Vec::new(),
        intent: None,
        control: None,
        snapshot: None,
        abandonment: None,
        abort_request: None,
        abort_binding: None,
    }
}

const fn predecessor_state(next: PublicationIntentState) -> Option<PublicationIntentState> {
    match next {
        PublicationIntentState::NewPointsAcknowledged => {
            Some(PublicationIntentState::IntentDurable)
        }
        PublicationIntentState::OldPointsClosedAcknowledged => {
            Some(PublicationIntentState::NewPointsAcknowledged)
        }
        PublicationIntentState::ReadbackVerified => {
            Some(PublicationIntentState::OldPointsClosedAcknowledged)
        }
        _ => None,
    }
}

fn is_allowed_intent_predecessor(
    previous: PublicationIntentState,
    next: PublicationIntentState,
) -> bool {
    match next {
        PublicationIntentState::Compensating => matches!(
            previous,
            PublicationIntentState::IntentDurable
                | PublicationIntentState::NewPointsAcknowledged
                | PublicationIntentState::OldPointsClosedAcknowledged
                | PublicationIntentState::ReadbackVerified
        ),
        _ => matches!(predecessor_state(next), Some(expected) if expected == previous),
    }
}

const fn model_phase(state: PublicationIntentState) -> Option<PublicationPhase> {
    match state {
        PublicationIntentState::IntentDurable => Some(PublicationPhase::IntentDurable),
        PublicationIntentState::NewPointsAcknowledged => {
            Some(PublicationPhase::NewPointsAcknowledged)
        }
        PublicationIntentState::OldPointsClosedAcknowledged => {
            Some(PublicationPhase::OldPointsClosedAcknowledged)
        }
        PublicationIntentState::ReadbackVerified => Some(PublicationPhase::ReadbackVerified),
        PublicationIntentState::ControlCommitted => Some(PublicationPhase::ControlCommitted),
        PublicationIntentState::Compensating => Some(PublicationPhase::Compensating),
        PublicationIntentState::Aborted => Some(PublicationPhase::Aborted),
        PublicationIntentState::PublicationBlocked => Some(PublicationPhase::PublicationBlocked),
        PublicationIntentState::Prepared
        | PublicationIntentState::Reclaimable
        | PublicationIntentState::InvalidationOnlyCommitted => None,
    }
}

const fn intent_state_label(state: PublicationIntentState) -> &'static str {
    match state {
        PublicationIntentState::Prepared => "prepared",
        PublicationIntentState::IntentDurable => "intent-durable",
        PublicationIntentState::NewPointsAcknowledged => "new-points-acknowledged",
        PublicationIntentState::OldPointsClosedAcknowledged => "old-points-closed-acknowledged",
        PublicationIntentState::ReadbackVerified => "readback-verified",
        PublicationIntentState::ControlCommitted => "control-committed",
        PublicationIntentState::Reclaimable => "reclaimable",
        PublicationIntentState::Compensating => "compensating",
        PublicationIntentState::Aborted => "aborted",
        PublicationIntentState::InvalidationOnlyCommitted => "invalidation-only-committed",
        PublicationIntentState::PublicationBlocked => "publication-blocked",
    }
}

fn allocate_publication_ids<C: CancellationProbe>(
    ids: &SourceIdentityIdsV2<'_, C>,
    context: &OperationContext<C>,
) -> Result<(PublicationIntentId, PublicationReceiptId), AdapterError> {
    let intent_id = ids
        .fresh_publication_intent_id(context)
        .map_err(|_| AdapterError::ControlUnavailable)?;
    let receipt_id = ids
        .fresh_publication_receipt_id(context)
        .map_err(|_| AdapterError::ControlUnavailable)?;
    Ok((intent_id, receipt_id))
}

fn digest_parts(domain: &str, parts: &[&[u8]]) -> Result<Blake3Digest32, AdapterError> {
    let total = parts
        .iter()
        .try_fold(0_usize, |sum, part| {
            sum.checked_add(8)?.checked_add(part.len())
        })
        .ok_or(AdapterError::InvalidBinding)?;
    if total > MAX_DIGEST_INPUT {
        return Err(AdapterError::InvalidBinding);
    }
    let mut framed = Vec::with_capacity(total);
    for part in parts {
        framed.extend_from_slice(
            &u64::try_from(part.len())
                .map_err(|_| AdapterError::InvalidBinding)?
                .to_be_bytes(),
        );
        framed.extend_from_slice(part);
    }
    let domain = CanonicalDigestDomain::parse(domain).map_err(|_| AdapterError::InvalidBinding)?;
    let limit =
        DigestInputLimit::new(MAX_DIGEST_INPUT).map_err(|_| AdapterError::InvalidBinding)?;
    blake3_raw(&domain, &framed, limit).map_err(|_| AdapterError::InvalidBinding)
}

fn validate_abort_request_binding(
    binding: &PublicationEffectBinding,
    request: &AbortFinalizationRequest,
) -> Result<(), AdapterError> {
    let intent = &request.intent;
    if request.operation_id != binding.operation_id
        || intent.transaction_id() != &binding.transaction_id
        || intent.target_epoch() != binding.target_epoch
        || intent.old_manifest_digest() != binding.old_manifest_digest
        || intent.new_manifest_digest() != binding.new_manifest_digest
        || intent.guards() != binding.guards
        || request.prepared_input_digest != binding.prepared_input_digest
        || request.collection_generation_id != binding.scope.collection_generation_id
        || request.previous_visible_epoch != binding.previous_visible_epoch
        || request.expected_control_generation == 0
        || !abort_guards_are_current_or_newer(request.current_guards, binding.guards)
    {
        return Err(AdapterError::InvalidBinding);
    }
    Ok(())
}

fn abort_guards_are_current_or_newer(
    current: PublicationGuards,
    original: PublicationGuards,
) -> bool {
    current.owner_epoch >= original.owner_epoch
        && current.source_catalog_generation >= original.source_catalog_generation
        && current.membership_generation >= original.membership_generation
        && current.access_generation >= original.access_generation
        && current.shadow_generation >= original.shadow_generation
        && current.purge_generation >= original.purge_generation
        && current.profile_digest == original.profile_digest
}

fn abort_record_matches_intent(
    metadata: &PublicationAbortFinalizationMetadataV1,
    intent: &PublicationIntent,
) -> bool {
    intent.state == PublicationIntentState::Aborted
        && intent.publication_intent_id == metadata.publication_intent_id
        && intent.target_epoch == metadata.target_epoch
        && intent.owner_source_membership_access_guards == metadata.original_guards
        && intent.prepared_manifest_ref == metadata.new_manifest_reference
}

fn abort_scope_digest(
    scope: &search_publication::PublicationScope,
) -> Result<Blake3Digest32, AdapterError> {
    let source_length = scope.source.byte_length.to_be_bytes();
    digest_parts(
        ABORT_SCOPE_DOMAIN,
        &[
            scope.installation_incarnation_id.as_bytes(),
            scope.collection_generation_id.as_bytes(),
            scope.projection_membership_id.as_bytes(),
            scope.source_membership_id.as_bytes(),
            scope.source.source_namespace_id.as_bytes(),
            scope.source.source_id.as_bytes(),
            scope.source.revision_id.as_bytes(),
            scope.source.content_digest.as_bytes(),
            &source_length,
            scope.materialization_id.as_bytes(),
            scope.representation_id.as_bytes(),
            scope.unit_manifest_digest.as_bytes(),
            scope.projection_profile_set_id.as_str().as_bytes(),
            scope.projection_schema_id.as_str().as_bytes(),
            scope.schema_identity.as_bytes(),
            scope.access_partition_id.as_bytes(),
            scope.scoring_partition_id.as_bytes(),
        ],
    )
}

fn abort_resolution_metadata(
    resolution: &AbortedPublicationResolution,
) -> Result<(OpaqueId, AbortResolutionMetadataV1), AdapterError> {
    match resolution {
        AbortedPublicationResolution::Compensated {
            operation_id,
            restored_old_points,
        } => Ok((
            operation_id.clone(),
            AbortResolutionMetadataV1::Compensated {
                restored_old_points: *restored_old_points,
            },
        )),
        AbortedPublicationResolution::Excluded {
            operation_id,
            excluded_projection_memberships,
        } => {
            if excluded_projection_memberships
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(AdapterError::InvalidBinding);
            }
            let membership_count = u64::try_from(excluded_projection_memberships.len())
                .map_err(|_| AdapterError::InvalidBinding)?;
            let mut bytes = Vec::with_capacity(
                excluded_projection_memberships
                    .len()
                    .checked_mul(16)
                    .ok_or(AdapterError::InvalidBinding)?,
            );
            for membership in excluded_projection_memberships {
                bytes.extend_from_slice(membership.as_bytes());
            }
            let count = membership_count.to_be_bytes();
            let membership_digest = digest_parts(ABORT_MEMBERSHIPS_DOMAIN, &[&count, &bytes])?;
            Ok((
                operation_id.clone(),
                AbortResolutionMetadataV1::Excluded {
                    membership_count,
                    membership_digest,
                },
            ))
        }
    }
}

fn abort_resolution_digest(
    operation_id: &OpaqueId,
    resolution: &AbortResolutionMetadataV1,
) -> Result<Blake3Digest32, AdapterError> {
    let (tag, detail): (u8, Vec<u8>) = match resolution {
        AbortResolutionMetadataV1::Compensated {
            restored_old_points,
        } => (1, vec![u8::from(*restored_old_points)]),
        AbortResolutionMetadataV1::Excluded {
            membership_count,
            membership_digest,
        } => {
            let mut detail = membership_count.to_be_bytes().to_vec();
            detail.extend_from_slice(membership_digest.as_bytes());
            (2, detail)
        }
    };
    digest_parts(
        ABORT_MEMBERSHIPS_DOMAIN,
        &[operation_id.as_str().as_bytes(), &[tag], &detail],
    )
}

fn abort_request_digest(
    binding: &PublicationEffectBinding,
    request: &AbortFinalizationRequest,
    original: &PublicationIntentBindingMetadataV1,
    scope_digest: Blake3Digest32,
) -> Result<Blake3Digest32, AdapterError> {
    let (resolution_operation_id, resolution) = abort_resolution_metadata(&request.resolution)?;
    let resolution_digest = abort_resolution_digest(&resolution_operation_id, &resolution)?;
    let expected_generation = request.expected_control_generation.to_be_bytes();
    let previous_epoch = request.previous_visible_epoch.get().to_be_bytes();
    let target_epoch = request.intent.target_epoch().get().to_be_bytes();
    let intent_generation = request.intent.control_generation().to_be_bytes();
    let original_relative_deadline = original.original_relative_deadline_ms.to_be_bytes();
    let abort_relative_deadline = binding.relative_deadline_ms.to_be_bytes();
    let old_digest = optional_digest_bytes(request.intent.old_manifest_digest());
    let guards = guards_bytes(request.current_guards);
    let original_guards = guards_bytes(request.intent.guards());
    digest_parts(
        ABORT_REQUEST_DOMAIN,
        &[
            request.operation_id.as_str().as_bytes(),
            &expected_generation,
            request.collection_generation_id.as_bytes(),
            &previous_epoch,
            request.intent.transaction_id().as_str().as_bytes(),
            &target_epoch,
            &old_digest,
            request.intent.new_manifest_digest().as_bytes(),
            &original_guards,
            request.intent.persist_operation_id().as_str().as_bytes(),
            request.intent.request_id().as_bytes(),
            &original_relative_deadline,
            original.original_budget_ref.as_str().as_bytes(),
            &intent_generation,
            request.intent.control_state_digest().as_bytes(),
            request.prepared_input_digest.as_bytes(),
            binding.request_id.as_bytes(),
            &abort_relative_deadline,
            binding.budget_ref.as_str().as_bytes(),
            scope_digest.as_bytes(),
            &guards,
            resolution_operation_id.as_str().as_bytes(),
            resolution_digest.as_bytes(),
        ],
    )
}

fn optional_digest_bytes(digest: Option<Blake3Digest32>) -> Vec<u8> {
    digest.map_or_else(
        || vec![0],
        |digest| {
            let mut bytes = Vec::with_capacity(33);
            bytes.push(1);
            bytes.extend_from_slice(digest.as_bytes());
            bytes
        },
    )
}

fn abort_request_digest_from_metadata(
    metadata: &PublicationAbortFinalizationMetadataV1,
) -> Result<Blake3Digest32, AdapterError> {
    let resolution_digest =
        abort_resolution_digest(&metadata.resolution_operation_id, &metadata.resolution)?;
    let expected_generation = metadata.expected_control_generation.to_be_bytes();
    let previous_epoch = metadata.previous_visible_epoch.get().to_be_bytes();
    let target_epoch = metadata.target_epoch.get().to_be_bytes();
    let intent_generation = metadata.original_control_generation.to_be_bytes();
    let original_relative_deadline = metadata.original_relative_deadline_ms.to_be_bytes();
    let abort_relative_deadline = metadata.abort_relative_deadline_ms.to_be_bytes();
    let old_digest = optional_digest_bytes(metadata.old_manifest_digest);
    let guards = guards_bytes(metadata.current_guards);
    let original_guards = guards_bytes(metadata.original_guards);
    digest_parts(
        ABORT_REQUEST_DOMAIN,
        &[
            metadata.abort_operation_id.as_str().as_bytes(),
            &expected_generation,
            metadata.collection_generation_id.as_bytes(),
            &previous_epoch,
            metadata.transaction_id.as_str().as_bytes(),
            &target_epoch,
            &old_digest,
            metadata.new_manifest_digest.as_bytes(),
            &original_guards,
            metadata.original_persist_operation_id.as_str().as_bytes(),
            metadata.original_request_id.as_bytes(),
            &original_relative_deadline,
            metadata.original_budget_ref.as_str().as_bytes(),
            &intent_generation,
            metadata.original_control_state_digest.as_bytes(),
            metadata.prepared_input_digest.as_bytes(),
            metadata.abort_request_id.as_bytes(),
            &abort_relative_deadline,
            metadata.abort_budget_ref.as_str().as_bytes(),
            metadata.scope_digest.as_bytes(),
            &guards,
            metadata.resolution_operation_id.as_str().as_bytes(),
            resolution_digest.as_bytes(),
        ],
    )
}

fn abort_finalization_command_digest_from_metadata(
    binding: &PublicationEffectBinding,
    metadata: &PublicationAbortFinalizationMetadataV1,
) -> Result<Blake3Digest32, AdapterError> {
    let generation = metadata.expected_control_generation.to_be_bytes();
    digest_parts(
        COMMAND_DOMAIN,
        &[
            binding.prepared_input_digest.as_bytes(),
            binding.transaction_id.as_str().as_bytes(),
            metadata.abort_operation_id.as_str().as_bytes(),
            b"abort-finalization",
            &generation,
            metadata.abort_request_digest.as_bytes(),
        ],
    )
}
fn abort_finalization_command_digest(
    binding: &PublicationEffectBinding,
    request: &AbortFinalizationRequest,
    request_digest: Blake3Digest32,
) -> Result<Blake3Digest32, AdapterError> {
    let generation = request.expected_control_generation.to_be_bytes();
    digest_parts(
        COMMAND_DOMAIN,
        &[
            binding.prepared_input_digest.as_bytes(),
            binding.transaction_id.as_str().as_bytes(),
            request.operation_id.as_str().as_bytes(),
            b"abort-finalization",
            &generation,
            request_digest.as_bytes(),
        ],
    )
}
fn visibility_command_digest(
    binding: &PublicationEffectBinding,
    publication_receipt_id: PublicationReceiptId,
    readback_digest: Blake3Digest32,
) -> Result<Blake3Digest32, AdapterError> {
    visibility_command_digest_for_operation(
        binding,
        &binding.operation_id,
        publication_receipt_id,
        readback_digest,
    )
}

fn visibility_command_digest_for_operation(
    binding: &PublicationEffectBinding,
    visibility_operation: &OpaqueId,
    publication_receipt_id: PublicationReceiptId,
    readback_digest: Blake3Digest32,
) -> Result<Blake3Digest32, AdapterError> {
    digest_parts(
        COMMAND_DOMAIN,
        &[
            binding.prepared_input_digest.as_bytes(),
            visibility_operation.as_str().as_bytes(),
            publication_receipt_id.as_bytes(),
            b"visible-epoch",
            readback_digest.as_bytes(),
        ],
    )
}

/// Cross-phase evidence keeps every frozen input while using distinct actual phase identities.
fn same_transaction_inputs(a: &PublicationEffectBinding, b: &PublicationEffectBinding) -> bool {
    a.transaction_id == b.transaction_id
        && a.request_id == b.request_id
        && a.relative_deadline_ms == b.relative_deadline_ms
        && a.budget_ref == b.budget_ref
        && a.prepared_input_digest == b.prepared_input_digest
        && a.scope == b.scope
        && a.target_epoch == b.target_epoch
        && a.previous_visible_epoch == b.previous_visible_epoch
        && a.last_reserved_epoch == b.last_reserved_epoch
        && a.old_manifest_digest == b.old_manifest_digest
        && a.old_manifest == b.old_manifest
        && a.new_manifest_digest == b.new_manifest_digest
        && a.new_manifest == b.new_manifest
        && a.difference == b.difference
        && a.guards == b.guards
}

fn intent_digest(intent: &PublicationIntent) -> Result<Blake3Digest32, AdapterError> {
    let epoch = intent.target_epoch.get().to_be_bytes();
    let state = intent_state_label(intent.state);
    let guards = guards_bytes(intent.owner_source_membership_access_guards);
    digest_parts(
        CONTROL_STATE_DOMAIN,
        &[
            intent.publication_intent_id.as_bytes(),
            &epoch,
            intent.prepared_manifest_ref.as_str().as_bytes(),
            state.as_bytes(),
            &guards,
        ],
    )
}

fn visibility_digest(state: &PublicationVisibilityState) -> Result<Blake3Digest32, AdapterError> {
    let epoch = state.visible_epoch.get().to_be_bytes();
    let route = state.collection_generation_id.as_bytes();
    let guards = guards_bytes(state.guards);
    let receipt = state
        .last_receipt
        .map_or([0_u8; 16], |value| *value.as_bytes());
    digest_parts(
        CONTROL_STATE_DOMAIN,
        &[
            route,
            state.schema_identity_digest.as_bytes(),
            &epoch,
            &guards,
            &receipt,
        ],
    )
}

fn guards_bytes(guards: PublicationGuards) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 * 6 + 32);
    for value in [
        guards.owner_epoch.get(),
        guards.source_catalog_generation,
        guards.membership_generation,
        guards.access_generation,
        guards.shadow_generation,
        guards.purge_generation,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend_from_slice(guards.profile_digest.as_bytes());
    bytes
}

fn point_matches(
    expected: &PublicationPointExpectation,
    actual: &PublicationPointReadback,
) -> bool {
    expected.point_id() == actual.point_id
        && expected.identity() == &actual.identity
        && expected.payload() == &actual.payload
        && expected.payload().validate().is_ok()
        && actual.payload.validate().is_ok()
        && expected.payload().identity().ok().is_some()
        && expected.payload().identity().ok() == actual.payload.identity().ok()
        && vectors_match(expected.vectors(), &actual.vectors)
}

fn vectors_match(
    expected: &std::collections::BTreeMap<String, VectorValue>,
    actual: &std::collections::BTreeMap<String, VectorValue>,
) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|((left_name, left), (right_name, right))| {
                left_name == right_name && vector_match(left, right)
            })
}

fn vector_match(left: &VectorValue, right: &VectorValue) -> bool {
    let same_bits = |left: &[f32], right: &[f32]| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| left.to_bits() == right.to_bits())
    };
    match (left, right) {
        (VectorValue::Dense(left), VectorValue::Dense(right)) => same_bits(left, right),
        (
            VectorValue::Sparse {
                indices: left_indices,
                values: left_values,
            },
            VectorValue::Sparse {
                indices: right_indices,
                values: right_values,
            },
        ) => left_indices == right_indices && same_bits(left_values, right_values),
        _ => false,
    }
}

fn point_readback_digest(
    binding: &PublicationEffectBinding,
    points: &[PublicationPointReadback],
) -> Result<Blake3Digest32, AdapterError> {
    // The bridge returns every full point record and this owner compares every
    // payload/vector/bound above before a digest exists. The compact receipt
    // commits to the admitted full manifests, original request and exact
    // successful denominator; it does not replace point-by-point validation.
    let count = u64::try_from(points.len())
        .map_err(|_| AdapterError::InvalidBinding)?
        .to_be_bytes();
    let target_epoch = binding.target_epoch.get().to_be_bytes();
    digest_parts(
        READBACK_DOMAIN,
        &[
            binding.prepared_input_digest.as_bytes(),
            binding.new_manifest.canonical_bytes.as_slice(),
            binding
                .old_manifest
                .as_ref()
                .map_or(&[][..], |manifest| manifest.canonical_bytes.as_slice()),
            &target_epoch,
            &count,
        ],
    )
}

fn control_snapshot_digest<F, E>(
    snapshot: &std::sync::Arc<search_control_redb::ControlSnapshot>,
    limits: search_control_redb::JournalLimits,
    work: &F,
) -> Result<Blake3Digest32, AdapterError>
where
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    let digest_bytes = snapshot
        .records
        .len()
        .checked_mul(32)
        .ok_or(AdapterError::InvalidBinding)?;
    if digest_bytes > MAX_DIGEST_INPUT {
        return Err(AdapterError::InvalidBinding);
    }
    work(MigrationBootstrapWork::Allocate(digest_bytes))
        .map_err(|_| AdapterError::ControlUnavailable)?;
    let mut row_digests = Vec::with_capacity(digest_bytes);
    for (key, value) in &snapshot.records {
        work(MigrationBootstrapWork::Process(
            key.as_bytes()
                .len()
                .checked_add(value.as_bytes().len())
                .ok_or(AdapterError::InvalidBinding)?,
        ))
        .map_err(|_| AdapterError::ControlUnavailable)?;
        let row = digest_parts(SNAPSHOT_ROW_DOMAIN, &[key.as_bytes(), value.as_bytes()])?;
        row_digests.extend_from_slice(row.as_bytes());
    }
    let generation = snapshot.generation.to_be_bytes();
    let record_count = u64::try_from(snapshot.records.len())
        .map_err(|_| AdapterError::InvalidBinding)?
        .to_be_bytes();
    if row_digests.len() > limits.max_records.saturating_mul(32) {
        return Err(AdapterError::InvalidBinding);
    }
    digest_parts(
        SNAPSHOT_DOMAIN,
        &[
            snapshot.identity.installation_incarnation_id.as_bytes(),
            snapshot.identity.data_root_id.as_bytes(),
            &snapshot.identity.owner_epoch.get().to_be_bytes(),
            snapshot.identity.path_identity_digest.as_bytes(),
            snapshot.identity.schema_family_digest.as_bytes(),
            &snapshot.identity.schema_version.to_be_bytes(),
            &generation,
            &record_count,
            &row_digests,
        ],
    )
}

fn snapshot_process_id(
    snapshot: &std::sync::Arc<search_control_redb::ControlSnapshot>,
    digest: Blake3Digest32,
) -> Result<OpaqueId, AdapterError> {
    let address = (std::sync::Arc::as_ptr(snapshot) as usize).to_be_bytes();
    let id = digest_parts(SNAPSHOT_DOMAIN, &[&address, digest.as_bytes()])?;
    OpaqueId::new(format!("publication-snapshot-v1:{id}")).map_err(|_| AdapterError::InvalidBinding)
}
