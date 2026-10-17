//! Normal C16 publication while the original held-source operation is live.

use search_control_redb::{ControlSnapshotPublisher, PersistentControlJournal};
use search_projection_planner::ProjectionBudget;
use search_publication::{ProjectionUpdate, PublicationCoordinator, PublicationProposal};
use search_qdrant_bridge::{CollectionRoute, real::BlockingRealQueryPlane};

use super::{
    publication_admission::{NativePublicationAdmission, NativePublicationProjection},
    retention_source::RetentionSourceInput,
};
use crate::{
    preparation_composition::native_artifact_store::NativeStoredArtifactSet,
    publication_composition::canonical_control::{
        CanonicalPublicationArtifacts, CanonicalPublicationPorts,
    },
    registry_control::SourceIdentityIdsV2,
};

/// All effects use this same runtime plane; this view creates no endpoint,
/// child, journal, owner epoch, current pointer or independent publication state.
pub(super) struct IndexedRetention<'a> {
    pub(super) plane: &'a mut BlockingRealQueryPlane,
    pub(super) route: CollectionRoute,
}

impl IndexedRetention<'_> {
    pub(super) fn publish_new(
        &mut self,
        input: &RetentionSourceInput<'_, '_, '_>,
        stored: &NativeStoredArtifactSet,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
    ) -> Result<search_contracts::Epoch, String> {
        input
            .native
            .source_full_checkpoint()
            .map_err(|error| error.to_string())?;
        input
            .security
            .checkpoint()
            .map_err(|error| error.to_string())?;
        if self.route.generation
            != stored
                .prepared()
                .scope()
                .membership
                .collection_generation_id
        {
            return Err("NATIVE_PUBLICATION_RUNTIME_ROUTE_MISMATCH".into());
        }
        let projection = NativePublicationProjection::Stored(stored);
        let mut coordinator = PublicationCoordinator::new(ProjectionBudget::BASELINE.max_points)
            .map_err(|error| error.code().to_owned())?;
        {
            // A prior pointer requires a genuine old C07 reopen. The admission
            // owner verifies absence for this first-publication path; it cannot
            // turn an unresolved/update operation into an empty old set.
            let (mut owners, snapshot) =
                NativePublicationAdmission::capture_for_prepare(input, journal, publisher, None)
                    .map_err(|error| error.code().to_owned())?;
            let proposal = PublicationProposal {
                transaction_id: input.command.operation.clone(),
                old: None,
                update: ProjectionUpdate::Publish(&projection),
            };
            coordinator
                .submit_with_effect_preparation(
                    &proposal,
                    &mut owners,
                    &snapshot,
                    ProjectionBudget::BASELINE,
                )
                .map_err(|error| error.code().to_owned())?;
        }
        // Release the admission's journal borrow before borrowing the real
        // effect/readback pair. No owner is reopened and no budget is renewed.
        let work = |unit| input.native.work(unit);
        let ids = SourceIdentityIdsV2::bind(input.deadline, &work);
        let ports = CanonicalPublicationPorts::bind(
            journal,
            publisher,
            &ids,
            self.plane,
            self.route.clone(),
            CanonicalPublicationArtifacts::from_stored(stored),
            input.native.context(),
            input.deadline,
            &work,
        )
        .map_err(|_| "NATIVE_PUBLICATION_PORT_BIND_REFUSED")?;
        let (mut effects, mut readback) = ports.split();
        let context = input.native.context();
        coordinator
            .persist_intent(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .stage_new_points(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .close_old_points(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .verify_readback(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .commit_visible_epoch(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .publish_control_snapshot(&mut effects, &mut readback, context)
            .map_err(|error| error.code().to_owned())?;
        coordinator
            .complete()
            .map_err(|error| error.code().to_owned())?;
        input
            .native
            .source_full_checkpoint()
            .map_err(|error| error.to_string())?;
        input
            .security
            .checkpoint()
            .map_err(|error| error.to_string())?;
        coordinator
            .visible_epoch()
            .ok_or_else(|| "NATIVE_PUBLICATION_VISIBLE_EPOCH_MISSING".to_owned())
    }
}
