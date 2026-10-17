//! V2 retained read adapter. All calls stay inside the original C18/issuer lock.

use search_access::{AccessCheckpoint, AccessError};
use search_contracts::{ProjectionMembershipId, SourceMembershipId, SourceRevisionRef};
use search_control_redb::MigrationBootstrapWork as Work;
use search_revision_store::PersistedSelectedObjectMetadata;
use search_source_history::revision::SourceRevisionControlPort;

use super::super::super::read_child::{
    NativeRequestReadState,
    fresh_root::{FreshRootCancellation, FreshRootContext},
    with_guarded_read_operation,
};
use super::super::{NativeProcessRootCustody, retention_control::NativeRetentionControl};
use super::{RetainedRevisionReadState, open_retained_revision};
use crate::config_composition::EffectiveDaemonConfig;
use crate::provider_composition::CanonicalServingError as Error;
use crate::query_composition::native_v2::ReadTurn;

const fn unavailable() -> Error {
    Error::Backend("SOURCE_REVISION_UNAVAILABLE")
}

impl NativeProcessRootCustody {
    /// Read actual bytes under one retained cumulative request meter.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_evidence_retained_read<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        turn: &ReadTurn<'_, '_>,
        membership: SourceMembershipId,
        expected: &SourceRevisionRef,
        config: &EffectiveDaemonConfig,
        consume: impl FnOnce(&[u8], &PersistedSelectedObjectMetadata) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.with_evidence_retained_control(
            state,
            registry,
            turn,
            membership,
            expected.revision_id,
            Some(expected.byte_length),
            false,
            |native, control, _remaining, _snapshot| {
                let (plaintext, metadata) =
                    open_retained_revision(native, control, expected, config)
                        .map_err(|_| unavailable())?;
                consume(plaintext.as_bytes(), &metadata)
            },
        )
    }

    /// Protect a genuine current retained object for C28 without reading its body.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_evidence_retained_metadata<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        turn: &ReadTurn<'_, '_>,
        membership: SourceMembershipId,
        expected: &SourceRevisionRef,
        consume: impl FnOnce(&PersistedSelectedObjectMetadata) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.with_evidence_retained_control(
            state,
            registry,
            turn,
            membership,
            expected.revision_id,
            None,
            false,
            |native, control, _remaining, _snapshot| {
                let (metadata, reservation) = control
                    .load_retained_revision(expected, native.context())
                    .map_err(|_| unavailable())?;
                let result = consume(&metadata)?;
                let after = control
                    .load_retained_revision(expected, native.context())
                    .map_err(|_| unavailable())?;
                if after != (metadata, reservation) {
                    return Err(unavailable());
                }
                native.source_full_checkpoint().map_err(|_| unavailable())?;
                Ok(result)
            },
        )
    }

    /// Only a nominated in-population projection can select a full C07 reopen.
    /// The payload epochs are untrusted reconstruction parameters: the C07
    /// reader verifies every reconstructed member byte before returning it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reopen_evidence_projection(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        turn: &ReadTurn<'_, '_>,
        projection: ProjectionMembershipId,
        observation: &search_contracts::indexed::PointPayload,
        profiles: &super::super::native_preparation_factory::QualifiedNativeProfiles,
        memory_remaining: u64,
        config: &EffectiveDaemonConfig,
    ) -> Result<
        (
            crate::preparation_composition::artifact_payloads::PreparedRawCodeArtifacts,
            search_contracts::SourceRevision,
        ),
        Error,
    > {
        let row = turn
            .population
            .projection_memberships()
            .values()
            .find(|row| row.projection_membership_id == projection)
            .ok_or_else(unavailable)?;
        let membership = row.source_membership_id;
        let (_, identity) = registry
            .active_source_membership_v2(membership)
            .map_err(|_| unavailable())?;
        let search_contracts::SourceView::RetainedRevision(revision_id) =
            turn.population.snapshot_fence().source_view
        else {
            return Err(Error::Backend("V2_SOURCE_VIEW_UNSUPPORTED"));
        };
        // Resolve the expected revision from durable metadata, not the observed payload.
        self.with_evidence_retained_control(
            state,
            registry,
            turn,
            membership,
            revision_id,
            None,
            true,
            |native, control, remaining, snapshot| {
                let revision = control
                    .load_source_revision(
                        &identity.source_namespace_id,
                        &identity.source_id,
                        &revision_id,
                        native.context(),
                    )
                    .map_err(|_| unavailable())?
                    .ok_or_else(unavailable)?;
                let expected = SourceRevisionRef {
                    content_digest: revision.content_digest,
                    byte_length: revision.byte_length,
                    source_namespace_id: identity.source_namespace_id,
                    source_id: identity.source_id,
                    revision_id,
                };
                let search_provider_protocol::frame::EvidenceRecipeRequestV2::RetrieveEvidence(
                    request,
                ) = &turn.request.envelope().recipe_request
                else {
                    return Err(unavailable());
                };
                // Whole-revision decryption/reconstruction needs finite scratch. This
                // conservative reservation also bounds the retained preparation cache.
                if expected.byte_length.checked_mul(32).is_none_or(|bytes| {
                    bytes > request.budgets.max_memory_bytes.min(memory_remaining)
                }) {
                    return Err(Error::Backend("RESOURCE_EXHAUSTED"));
                }
                if *remaining == 0 {
                    return Err(Error::Backend("RESOURCE_EXHAUSTED"));
                }
                *remaining = remaining
                    .checked_sub(expected.byte_length)
                    .ok_or(Error::Backend("RESOURCE_EXHAUSTED"))?;
                let (plaintext, metadata) =
                    open_retained_revision(native, control, &expected, config)
                        .map_err(|_| unavailable())?;
                let snapshot = snapshot.ok_or_else(unavailable)?;
                let population = turn.population.population();
                if snapshot.visibility().collection_generation_id
                    != population.collection_generation_id()
                    || snapshot.visibility().visible_epoch != population.visible_epoch()
                    || snapshot.identity().installation_incarnation_id
                        != population.installation_incarnation_id()
                    || snapshot.initial_route_revision()
                        != Some(turn.population.snapshot_fence().collection_route_revision)
                {
                    return Err(Error::Access(AccessError::SnapshotStale));
                }
                profiles
                    .reopen_evidence_artifacts(
                        native,
                        control,
                        snapshot,
                        registry,
                        membership,
                        projection,
                        observation,
                        &revision,
                        &expected,
                        &plaintext,
                        &metadata,
                        config,
                    )
                    .map(|prepared| (prepared, revision))
            },
        )
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn with_evidence_retained_control<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        turn: &ReadTurn<'_, '_>,
        membership: SourceMembershipId,
        revision_id: search_contracts::SourceRevisionId,
        source_bytes: Option<u64>,
        read_projection: bool,
        consume: impl FnOnce(
            &FreshRootContext<'_, '_>,
            &NativeRetentionControl<'_, '_, FreshRootCancellation>,
            &mut u64,
            Option<&search_control_redb::PublicationProjectionSnapshot>,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        turn.check()?;
        if !turn.issued.template().source_read_permission {
            return Err(Error::Access(AccessError::ScopeUnauthorized));
        }
        if registry.revision() != turn.population.snapshot_fence().catalog_revision.get()
            || !turn
                .population
                .projection_memberships()
                .contains_key(&membership)
        {
            return Err(Error::Access(AccessError::SnapshotStale));
        }
        let (_, identity) = registry
            .active_source_membership_v2(membership)
            .map_err(|_| unavailable())?;
        if turn.population.snapshot_fence().source_view
            != search_contracts::SourceView::RetainedRevision(revision_id)
        {
            return Err(Error::Access(AccessError::ScopeUnauthorized));
        }
        if let Some(bytes) = source_bytes {
            state.charge_source_bytes(bytes)?;
        }
        let key = registry
            .membership_key_by_id(membership)
            .map_err(|_| unavailable())?;
        let member = registry.membership(key).map_err(|_| unavailable())?;
        let source = registry.sources().get(identity).ok_or_else(unavailable)?;
        let namespace = registry
            .namespaces()
            .get(&identity.source_namespace_id)
            .ok_or_else(unavailable)?;
        let root = registry
            .root(source.binding().root_binding_id())
            .map_err(|_| unavailable())?;
        turn.domain
            .with_live_checkpoint(turn.fence, AccessCheckpoint::BeforeSourceReadback, |_| ())
            .map_err(Error::Access)?;
        if state.native.is_none() {
            state.native = Some(
                NativeRequestReadState::from_guard(
                    &self.owner.owner,
                    turn.request.guard(),
                    turn.budget,
                )
                .map_err(|_| unavailable())?,
            );
        }
        let remaining = &mut state.remaining_source_bytes;
        let projection_snapshot = &mut state.projection_snapshot;
        let native_state = state.native.as_mut().ok_or_else(unavailable)?;
        let journal = self
            .owner
            .control
            .journal
            .as_mut()
            .ok_or_else(unavailable)?;
        let publisher = &mut self.owner.control.publisher;
        let result = with_guarded_read_operation(
            &mut self.owner.owner,
            turn.request.guard(),
            native_state,
            turn.budget,
            |operation| {
                let deadline = operation.migration_deadline();
                let native = operation.fresh_root_context().map_err(|_| unavailable())?;
                native.source_full_checkpoint().map_err(|_| unavailable())?;
                for unit in [
                    Work::Allocate(
                        4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes,
                    ),
                    Work::Process(4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes),
                ] {
                    native.work(unit).map_err(|_| unavailable())?;
                }
                let conditions = turn
                    .domain
                    .retained_source_read_conditions(
                        identity.source_namespace_id,
                        namespace.ownership.source_owner_generation,
                        journal,
                        turn.fence,
                    )
                    .map_err(|_| Error::Access(AccessError::SecurityFailClosed))?;
                let work = |unit| native.work(unit);
                if read_projection && projection_snapshot.is_none() {
                    *projection_snapshot = Some(
                        journal
                            .bind_control_port_with_original_context::<FreshRootCancellation, _>(
                                publisher, deadline, &work,
                            )
                            .map_err(|_| unavailable())?
                            .read_publication_projection_snapshot_with_original_context(
                                deadline, &work,
                            )
                            .map_err(|_| unavailable())?
                            .ok_or_else(unavailable)?,
                    );
                }
                let control = NativeRetentionControl::bind_for_serving(
                    journal,
                    publisher,
                    source,
                    namespace,
                    root,
                    member,
                    &conditions,
                    deadline,
                    &work,
                )
                .map_err(|_| unavailable())?;
                consume(&native, &control, remaining, projection_snapshot.as_ref())
            },
        )
        .map_err(|_| unavailable())?;
        let after = turn.check().and_then(|()| {
            turn.domain
                .with_live_checkpoint(turn.fence, AccessCheckpoint::BeforeSourceReadback, |_| ())
                .map_err(Error::Access)
        });
        let value = result?;
        after?;
        Ok(value)
    }
}
