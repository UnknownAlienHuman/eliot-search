//! Read-only reconstruction using the same qualified profiles as native preparation.

use search_contracts::{self as c, indexed::PointPayload};
use search_control_redb::{MigrationBootstrapWork as Work, PublicationProjectionSnapshot};
use search_materializer::api::{
    AcceptedProfiles, DEFAULT_MATERIALIZATION_BUDGET, MaterializationRequest, SourceEncoding,
    SourceKind, validate_materialization_request,
};
use search_projection_planner::{
    LineageObservation, PlanningScope, ProjectionBudget, ProjectionMembershipBinding,
};
use search_unitizer::{UnitizationBudget, V3SourceBinding};

use super::super::super::read_child::fresh_root::{FreshRootCancellation, FreshRootContext};
use super::super::{
    artifact_reader::reopen_prepared_artifact_set_v1, retention_control::NativeRetentionControl,
};
use super::{
    CodeIdentifierLimits, PointIdentityLimits, QualifiedNativeProfiles,
    RawCodeArtifactProducerInput, SparseLimits,
};
use crate::provider_composition::CanonicalServingError as Error;

const fn unavailable() -> Error {
    Error::Backend("NATIVE_PREPARED_ARTIFACT_RECONSTRUCTION_REFUSED")
}

impl QualifiedNativeProfiles {
    /// No new profile qualification, IDs, publication or relative request budget.
    /// Untrusted payload epochs select a reconstruction; only exact comparison
    /// with all committed C07 bodies can accept that reconstruction.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(in super::super) fn reopen_evidence_artifacts(
        &self,
        native: &FreshRootContext<'_, '_>,
        control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
        snapshot: &PublicationProjectionSnapshot,
        registry: &search_source_registry::SourceRegistry,
        membership: c::SourceMembershipId,
        projection: c::ProjectionMembershipId,
        observed: &PointPayload,
        revision: &c::SourceRevision,
        expected: &c::SourceRevisionRef,
        plaintext: &search_revision_crypto::SelectedPlaintextV2,
        metadata: &search_revision_store::PersistedSelectedObjectMetadata,
        config: &crate::config_composition::EffectiveDaemonConfig,
    ) -> Result<crate::preparation_composition::artifact_payloads::PreparedRawCodeArtifacts, Error>
    {
        native.source_full_checkpoint().map_err(|_| unavailable())?;
        let record = snapshot
            .prepared_artifact_sets()
            .get(&projection)
            .ok_or_else(unavailable)?;
        let set = record.metadata();
        let current = registry
            .projection_membership_record(membership)
            .map_err(|_| unavailable())?;
        let member = current.binding();
        let visibility = snapshot.visibility();
        let (schema, projection_schema_id) = self
            .collection_schema(
                snapshot.identity().installation_incarnation_id,
                visibility.collection_generation_id,
            )
            .map_err(|_| unavailable())?;
        if set.source_revision != *expected
            || member.representation_id != set.representation_id
            || schema.identity().map_err(|_| unavailable())?.as_digest()
                != &visibility.schema_identity_digest
            || observed.projection_membership_id != projection
            || observed.repository_lineage_id.is_some()
            || observed.valid_until_epoch_exclusive.is_some()
            || observed.valid_from_epoch > visibility.visible_epoch
        {
            return Err(unavailable());
        }
        let scope = PlanningScope {
            membership: ProjectionMembershipBinding {
                projection_membership_id: projection,
                source_membership_id: member.source_membership_id,
                source_id: expected.source_id,
                source_revision_id: expected.revision_id,
                representation_id: set.representation_id,
                projection_schema_id,
                collection_generation_id: *schema.collection_generation_id(),
                installation_incarnation_id: *schema.installation_incarnation_id(),
                projection_profile_set_id: schema.projection_profile_set_id().clone(),
                access_partition_id: member.access_partition_id,
                scoring_partition_id: member.scoring_partition_id,
            },
            valid_from_epoch: observed.valid_from_epoch,
            valid_until_epoch_exclusive: None,
            lineage: LineageObservation {
                repository_lineage_id: None,
            },
            identity_limits: PointIdentityLimits::default(),
        };
        let binding = V3SourceBinding {
            source_namespace_id: expected.source_namespace_id,
            source_id: expected.source_id,
            source_revision_id: expected.revision_id,
            representation_id: set.representation_id,
            materialization_id: set.materialization_id,
        };
        let source_name =
            c::OpaqueId::new(expected.source_id.to_string()).map_err(|_| unavailable())?;
        let request = MaterializationRequest {
            source_id: source_name,
            revision: c::NonZeroRevision::new(revision.occurrence_sequence)
                .map_err(|_| unavailable())?,
            residency: metadata.storage_object_id().clone(),
            content_digest: expected.content_digest,
            byte_count: expected.byte_length,
            declared_kind: SourceKind::Code,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: self.materializer.id(),
            operation_id: set.operation_id.clone(),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        let materialization_budget = DEFAULT_MATERIALIZATION_BUDGET;
        let validated = validate_materialization_request(
            &request,
            &AcceptedProfiles::new(vec![self.materializer.clone()]),
            &materialization_budget,
        )
        .map_err(|_| unavailable())?;
        let unitization_budget = UnitizationBudget {
            max_steps: self.unitizer.descriptor().max_steps,
            max_encoded_bytes: self.unitizer.descriptor().max_manifest_bytes,
            deadline: native.original_deadline(),
            cancelled: native.original_cancellation_flag(),
        };
        let language = c::BoundedName::new("code").map_err(|_| unavailable())?;
        let prepaid = plaintext
            .as_bytes()
            .len()
            .checked_mul(32)
            .ok_or_else(unavailable)?;
        for unit in [Work::Allocate(prepaid), Work::Process(prepaid)] {
            native.work(unit).map_err(|_| unavailable())?;
        }
        let producer = RawCodeArtifactProducerInput {
            binding: &binding,
            request: &validated,
            materializer: &self.materializer,
            unitizer: &self.unitizer,
            materialization_budget,
            unitization_budget: &unitization_budget,
            scope: &scope,
            schema: &schema,
            lexical_profile: &self.lexical,
            language: &language,
            lexical_limits: CodeIdentifierLimits::HARD,
            sparse_limits: SparseLimits::BASELINE,
            projection_budget: ProjectionBudget::BASELINE,
            max_input_bytes: usize::try_from(materialization_budget.max_input_bytes)
                .map_err(|_| unavailable())?
                .min(self.unitizer.descriptor().limits.max_input_bytes),
            max_total_vector_values: usize::try_from(c::indexed::MAX_STORED_VECTOR_VALUES)
                .map_err(|_| unavailable())?,
            max_artifact_bytes: usize::try_from(materialization_budget.max_output_bytes)
                .map_err(|_| unavailable())?,
        };
        reopen_prepared_artifact_set_v1(
            native, control, snapshot, registry, projection, expected, plaintext, metadata,
            &producer, config,
        )
        .map(crate::owner_composition::NativeReopenedArtifactSet::into_prepared)
        .map_err(|_| unavailable())
    }
}
