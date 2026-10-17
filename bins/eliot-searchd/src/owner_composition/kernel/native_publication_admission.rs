//! Normal publication admission over the one retained Root and actual owner products.
//!
//! This capture admits Prepare only. Restore must load the original durable
//! operation and its recovery-plane references; a resolved serving snapshot is
//! deliberately insufficient to manufacture that historical context.

use search_control_redb::{
    ControlSnapshotPublisher, MigrationBootstrapWork as Work, PersistentControlJournal,
    PreparationInputReadbackV1, PublicationIntentHead, PublicationProjectionSnapshot,
};
use search_projection_planner::{PlannedUnit, ProjectionManifest};
use search_publication::{
    AdmissionPurpose, AdmissionWork, ProjectionEvidence, PublicationAdmissionOwners,
    PublicationEffectPreparationOwners, PublicationError as Error, PublicationMembershipState,
    PublicationOwnerObservation, PublicationScope,
};
use search_source_registry::{SourceRegistry, api::read_projection_membership};

use super::super::read_child::fresh_root::FreshRootCancellation;
use super::{
    artifact_reader::NativeReopenedArtifactSet, command::original_port,
    retention_control::NativeRetentionControl, retention_source::RetentionSourceInput,
};
use crate::preparation_composition::{
    artifact_payloads::PreparedRawCodeArtifacts, native_artifact_store::NativeStoredArtifactSet,
};

/// Borrows genuine owner output. Neither variant can construct its inner proof.
#[derive(Clone, Copy)]
pub(super) enum NativePublicationProjection<'product, 'snapshot, 'registry> {
    Stored(&'product NativeStoredArtifactSet),
    Reopened(&'product NativeReopenedArtifactSet<'snapshot, 'registry>),
}

impl NativePublicationProjection<'_, '_, '_> {
    const fn prepared(&self) -> &PreparedRawCodeArtifacts {
        match self {
            Self::Stored(value) => value.prepared(),
            Self::Reopened(value) => value.prepared(),
        }
    }
}

/// Coherent actual metadata; private construction performs the original reads.
pub(super) struct NativePublicationSnapshot {
    projection: PublicationProjectionSnapshot,
    intent: PublicationIntentHead,
    preparation: PreparationInputReadbackV1,
    registry: SourceRegistry,
}

/// A borrowed admission adapter, without durable state or mutation authority.
pub(super) struct NativePublicationAdmission<
    'call,
    'view,
    'operation,
    'root,
    'product,
    'snapshot,
    'registry,
> {
    input: &'call RetentionSourceInput<'view, 'operation, 'root>,
    journal: &'call mut PersistentControlJournal,
    publisher: &'call mut ControlSnapshotPublisher,
    old: Option<NativePublicationProjection<'product, 'snapshot, 'registry>>,
}

impl<'call, 'view, 'operation, 'root, 'product, 'snapshot, 'registry>
    NativePublicationAdmission<'call, 'view, 'operation, 'root, 'product, 'snapshot, 'registry>
{
    pub(super) fn capture_for_prepare(
        input: &'call RetentionSourceInput<'view, 'operation, 'root>,
        journal: &'call mut PersistentControlJournal,
        publisher: &'call mut ControlSnapshotPublisher,
        old: Option<NativePublicationProjection<'product, 'snapshot, 'registry>>,
    ) -> Result<(Self, NativePublicationSnapshot), Error> {
        checkpoint(input)?;
        let work = |unit| input.native.work(unit);
        let projection = journal
            .bind_control_port::<FreshRootCancellation>(publisher)
            .map_err(|_| Error::ControlConflict)?
            .read_publication_projection_snapshot_with_original_context(input.deadline, &work)
            .map_err(|_| Error::ControlConflict)?
            .ok_or(Error::ControlConflict)?;
        let registry = original_port(journal, publisher, input.deadline, &work)
            .map_err(|_| Error::GuardMismatch)?
            .restore(input.native.context())
            .map_err(|_| Error::GuardMismatch)?
            .into_registry();
        let port = journal
            .bind_control_port::<FreshRootCancellation>(publisher)
            .map_err(|_| Error::ControlConflict)?;
        let intent = port
            .read_publication_intent_with_original_context(input.deadline, &work)
            .map_err(|_| Error::ControlConflict)?;
        let preparation = port
            .read_preparation_input_with_original_context(
                &input.command.operation,
                input.deadline,
                &work,
            )
            .map_err(|_| Error::ControlConflict)?
            .ok_or(Error::InvalidPreparedPublication)?;
        if projection.identity() != intent.identity
            || projection.generation() != intent.generation
            || preparation.identity() != projection.identity()
            || preparation.generation() != projection.generation()
        {
            return Err(Error::ControlConflict);
        }
        let snapshot = NativePublicationSnapshot {
            projection,
            intent,
            preparation,
            registry,
        };
        let mut owner = Self {
            input,
            journal,
            publisher,
            old,
        };
        owner.verify_live(&snapshot)?;
        Ok((owner, snapshot))
    }

    fn verify_live(&mut self, snapshot: &NativePublicationSnapshot) -> Result<(), Error> {
        checkpoint(self.input)?;
        let facts = self
            .input
            .native
            .revision_dek_root_facts()
            .map_err(|_| Error::GuardMismatch)?;
        let identity = snapshot.projection.identity();
        if identity.data_root_id != facts.data_root
            || identity.installation_incarnation_id != facts.incarnation
            || identity.owner_epoch != facts.creator_epoch
        {
            return Err(Error::GuardMismatch);
        }
        let work = |unit| self.input.native.work(unit);
        let control = NativeRetentionControl::<FreshRootCancellation>::bind(
            self.journal,
            self.publisher,
            self.input.source,
            self.input.namespace,
            self.input.root,
            self.input.security,
            self.input.deadline,
            &work,
        )
        .map_err(|_| Error::GuardMismatch)?;
        let (actual_identity, generation) = control
            .projection_metadata_checkpoint()
            .map_err(|_| Error::GuardMismatch)?;
        if actual_identity != identity || generation != snapshot.projection.generation() {
            return Err(Error::GuardMismatch);
        }
        checkpoint(self.input)
    }
}

fn checkpoint(input: &RetentionSourceInput<'_, '_, '_>) -> Result<(), Error> {
    if input.deadline != input.native.original_deadline() {
        return Err(Error::OperationContextInvalid);
    }
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| Error::OperationContextInvalid)?;
    input
        .security
        .checkpoint()
        .map_err(|_| Error::GuardMismatch)?;
    if let Some(current) = input.current_source {
        current().map_err(|_| Error::GuardMismatch)?;
    }
    Ok(())
}

impl<'product, 'snapshot, 'registry> PublicationAdmissionOwners
    for NativePublicationAdmission<'_, '_, '_, '_, 'product, 'snapshot, 'registry>
{
    type Projection = NativePublicationProjection<'product, 'snapshot, 'registry>;
    type Snapshot = NativePublicationSnapshot;

    fn projection<'a>(&self, token: &'a Self::Projection) -> Result<ProjectionEvidence<'a>, Error> {
        let prepared = token.prepared();
        let source = match token {
            NativePublicationProjection::Stored(value) => {
                &value.c02_candidate().metadata().source_revision
            }
            NativePublicationProjection::Reopened(value) => {
                &value.record().metadata().source_revision
            }
        };
        Ok(ProjectionEvidence {
            source,
            units: prepared.units(),
            plan: prepared.plan(),
            planning_scope: prepared.scope(),
            schema: prepared.schema(),
        })
    }

    fn observe(
        &mut self,
        snapshot: &Self::Snapshot,
        transaction: &search_contracts::OpaqueId,
        scope: &PublicationScope,
        purpose: AdmissionPurpose,
    ) -> Result<PublicationOwnerObservation, Error> {
        if purpose != AdmissionPurpose::Prepare {
            return Err(Error::RecoveryBlocked);
        }
        self.verify_live(snapshot)?;
        let metadata = snapshot.preparation.record().metadata();
        let visibility = snapshot.projection.visibility();
        let binding = &metadata.projection_membership;
        let (member, source) = snapshot
            .registry
            .active_source_membership_v2(scope.source_membership_id)
            .map_err(|_| Error::GuardMismatch)?;
        let projection = read_projection_membership(
            &snapshot.registry,
            scope.source_membership_id,
            snapshot.registry.revision(),
        )
        .map_err(|_| Error::GuardMismatch)?;
        if transaction != &self.input.command.operation
            || metadata.original_operation_id != *transaction
            || member != &metadata.source_membership
            || source != self.input.source.identity()
            || projection.binding() != binding
            || scope.source != metadata.source_revision
            || scope.source.source_namespace_id != source.source_namespace_id
            || scope.source.source_id != source.source_id
            || scope.source.content_digest != self.input.source.binding().content_digest()
            || scope.source.byte_length != self.input.source.binding().content_bytes()
            || scope.materialization_id != metadata.materialization.materialization_id
            || scope.representation_id != metadata.representation.representation_id
            || scope.unit_manifest_digest != metadata.representation.unit_manifest_digest
            || scope.projection_membership_id != binding.projection_membership_id
            || scope.source_membership_id != binding.source_membership_id
            || scope.representation_id != binding.representation_id
            || scope.projection_schema_id != binding.projection_schema_id
            || scope.access_partition_id != binding.access_partition_id
            || scope.scoring_partition_id != binding.scoring_partition_id
            || scope.installation_incarnation_id
                != snapshot.projection.identity().installation_incarnation_id
            || scope.collection_generation_id != visibility.collection_generation_id
            || scope.schema_identity != visibility.schema_identity_digest
            || &scope.projection_profile_set_id
                != metadata.collection_schema.projection_profile_set_id()
            || metadata
                .collection_schema
                .identity()
                .map_err(|_| Error::GuardMismatch)?
                .as_digest()
                != &scope.schema_identity
            || metadata.policy_canonical != self.input.security.policy_body()
        {
            return Err(Error::GuardMismatch);
        }
        let floor = snapshot
            .intent
            .intent
            .as_ref()
            .map_or(visibility.visible_epoch, |intent| intent.target_epoch);
        let target = floor.checked_next().map_err(|_| Error::ContractExhausted)?;
        self.verify_live(snapshot)?;
        Ok(PublicationOwnerObservation {
            original_guards: visibility.guards,
            current_guards: visibility.guards,
            previous_visible_epoch: visibility.visible_epoch,
            current_visible_epoch: visibility.visible_epoch,
            last_reserved_epoch: floor,
            target_epoch: target,
            membership_state: PublicationMembershipState::Active,
        })
    }

    fn verify_current_manifest(
        &mut self,
        snapshot: &Self::Snapshot,
        scope: &PublicationScope,
        expected: Option<&ProjectionManifest>,
    ) -> Result<(), Error> {
        self.verify_live(snapshot)?;
        let current = snapshot
            .projection
            .manifests()
            .get(&scope.projection_membership_id);
        match (expected, current, self.old) {
            (None, None, None) => Ok(()),
            (Some(expected), Some(reference), Some(NativePublicationProjection::Reopened(old)))
                if old.generation() == snapshot.projection.generation()
                    && old.snapshot() == &snapshot.projection
                    && old.record().reference() == reference
                    && old.membership() == scope.projection_membership_id
                    && &old.prepared().plan().manifest == expected =>
            {
                Ok(())
            }
            _ => Err(Error::InvalidPreparedPublication),
        }
    }

    fn work(&mut self, snapshot: &Self::Snapshot, work: AdmissionWork) -> Result<(), Error> {
        let unit = match work {
            AdmissionWork::Checkpoint => Work::Checkpoint,
            AdmissionWork::Allocate(bytes) => Work::Allocate(bytes),
            AdmissionWork::Process(bytes) => Work::Process(bytes),
        };
        self.input
            .native
            .work(unit)
            .map_err(|_| Error::BudgetExceeded)?;
        if matches!(work, AdmissionWork::Checkpoint) {
            self.verify_live(snapshot)?;
        }
        Ok(())
    }
}

impl PublicationEffectPreparationOwners for NativePublicationAdmission<'_, '_, '_, '_, '_, '_, '_> {
    fn planned_vectors<'a>(
        &self,
        token: &'a Self::Projection,
    ) -> Result<Option<&'a [PlannedUnit]>, Error> {
        Ok(Some(token.prepared().vectors()))
    }
}
