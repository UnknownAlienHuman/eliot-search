//! Startup/visibility-refresh metadata reads under the one retained native Root.

use std::collections::{BTreeMap, BTreeSet};

use search_access::{AccessError, RequestSecurityFence};
use search_contracts::{
    CollectionRoute, MAX_COLLECTION_ITEMS, QuerySnapshotFence, SourceIdentity, SourceMembershipId,
    indexed::CollectionSchema,
};
use search_control_redb::{
    ControlSnapshotPublisher, MigrationBootstrapWork as Work, PersistentControlJournal,
    PublicationProjectionSnapshot,
};
use search_source_registry::{
    SourceRegistry, api::read_projection_membership, error::RegistryError,
};

use super::super::read_child::{
    direct_operation,
    fresh_root::{FreshRootCancellation, FreshRootContext},
};
use super::{
    NativeProcessRootCustody, artifact_reader::read_published_projection_metadata_v1,
    retention_control::NativeRetentionControl,
};
use crate::{
    access_composition::{NativeSecurityDomain, NativeSecurityError},
    config_composition::EffectiveDaemonConfig,
    query_serving_composition::canonical_population::CanonicalPopulationSnapshot,
};

/// Borrowed current owner data, not a permission or caller-issued verification.
pub struct NativePopulationInput<'a> {
    pub(crate) registry: &'a SourceRegistry,
    pub(crate) domain: &'a NativeSecurityDomain,
    pub(crate) sources: &'a BTreeMap<SourceMembershipId, SourceIdentity>,
    pub(crate) access_generation: u64,
    pub(crate) schema: CollectionSchema,
    pub(crate) route: CollectionRoute,
    pub(crate) fence: QuerySnapshotFence,
    pub(crate) config: &'a EffectiveDaemonConfig,
}

impl NativeProcessRootCustody {
    /// Reads metadata during the original startup operation. Publication refresh
    /// uses `capture_in_operation` with its own already-admitted native context.
    pub(crate) fn capture_query_population(
        &mut self,
        input: NativePopulationInput<'_>,
    ) -> Result<CanonicalPopulationSnapshot, AccessError> {
        let identity = self.journal_identity;
        let owner = &mut self.owner;
        let mut operation =
            direct_operation(&mut owner.owner).map_err(|_| AccessError::SecurityFailClosed)?;
        let native = operation
            .fresh_root_context()
            .map_err(|_| AccessError::SecurityFailClosed)?;
        let journal = owner
            .control
            .journal
            .as_mut()
            .ok_or(AccessError::SnapshotStale)?;
        let publisher = &mut owner.control.publisher;
        if journal.identity() != identity {
            return Err(AccessError::OwnerMismatch);
        }
        capture_in_operation(&native, journal, publisher, input)
    }
}

/// Uses the actual startup/publication invocation, never a replacement allowance
/// or the expired startup request for a later visibility refresh.
#[allow(clippy::too_many_lines)] // Preserve the single checked scope and final complete readback.
pub(super) fn capture_in_operation(
    native: &FreshRootContext<'_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    input: NativePopulationInput<'_>,
) -> Result<CanonicalPopulationSnapshot, AccessError> {
    if input.sources.len() > MAX_COLLECTION_ITEMS
        || input.registry.revision() != input.fence.catalog_revision.get()
    {
        return Err(AccessError::SnapshotStale);
    }
    native
        .source_full_checkpoint()
        .map_err(|_| AccessError::SecurityFailClosed)?;
    let facts = native
        .revision_dek_root_facts()
        .map_err(|_| AccessError::OwnerMismatch)?;
    let identity = journal.identity();
    if identity.data_root_id != facts.data_root
        || identity.installation_incarnation_id != facts.incarnation
        || identity.owner_epoch != facts.creator_epoch
    {
        return Err(AccessError::OwnerMismatch);
    }
    let deadline = native.original_deadline();
    let work = |unit| native.work(unit);
    let snapshot: PublicationProjectionSnapshot = journal
        .bind_control_port::<FreshRootCancellation>(publisher)
        .map_err(|_| AccessError::SnapshotStale)?
        .read_publication_projection_snapshot_with_original_context(deadline, &work)
        .map_err(|_| AccessError::SnapshotStale)?
        .ok_or(AccessError::SnapshotStale)?;
    if snapshot.identity() != identity {
        return Err(AccessError::OwnerMismatch);
    }
    if snapshot.visibility().collection_generation_id != input.route.collection_generation_id
        || snapshot.visibility().schema_identity_digest != input.route.schema_identity_digest
        || snapshot.visibility().visible_epoch != input.route.committed_visible_epoch
        || snapshot.initial_route_revision() != Some(input.fence.collection_route_revision)
    {
        return Err(AccessError::RouteMismatch);
    }
    let capacity = input
        .sources
        .len()
        .checked_mul(1024)
        .ok_or(AccessError::EligibilityPopulationTooLarge)?;
    for unit in [Work::Allocate(capacity), Work::Process(capacity)] {
        native
            .work(unit)
            .map_err(|_| AccessError::RetrievalLegBudgetExceeded)?;
    }
    let mut reads = Vec::with_capacity(input.sources.len());
    for (membership_id, identity) in input.sources {
        native
            .source_full_checkpoint()
            .map_err(|_| AccessError::SecurityFailClosed)?;
        let (_, current) = input
            .registry
            .active_source_membership_v2(*membership_id)
            .map_err(|_| AccessError::ScopeUnauthorized)?;
        if current != identity {
            return Err(AccessError::SnapshotStale);
        }
        let projection = match read_projection_membership(
            input.registry,
            *membership_id,
            input.registry.revision(),
        ) {
            Ok(projection) => projection,
            Err(RegistryError::ProjectionMembershipNotFound) => continue,
            Err(_) => return Err(AccessError::SnapshotStale),
        };
        let projection_id = projection.binding().projection_membership_id;
        let Some(record) = snapshot.prepared_artifact_sets().get(&projection_id) else {
            continue;
        };
        let expected = record.metadata().source_revision;
        if expected.source_namespace_id != identity.source_namespace_id
            || expected.source_id != identity.source_id
        {
            return Err(AccessError::SnapshotStale);
        }
        let source = input
            .registry
            .sources()
            .get(identity)
            .ok_or(AccessError::ScopeUnknown)?;
        let namespace = input
            .registry
            .namespaces()
            .get(&identity.source_namespace_id)
            .ok_or(AccessError::NamespaceUnknown)?;
        let root = input
            .registry
            .root(source.binding().root_binding_id())
            .map_err(|_| AccessError::ScopeUnknown)?;
        let key = input
            .registry
            .membership_key_by_id(*membership_id)
            .map_err(|_| AccessError::ScopeUnknown)?;
        let member = input
            .registry
            .membership(key)
            .map_err(|_| AccessError::ScopeUnknown)?;
        native
            .work(Work::Allocate(
                4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes,
            ))
            .map_err(|_| AccessError::RetrievalLegBudgetExceeded)?;
        native
            .work(Work::Process(
                4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes,
            ))
            .map_err(|_| AccessError::RetrievalLegBudgetExceeded)?;
        let fence = RequestSecurityFence {
            planned_generation: input.access_generation,
            memberships: BTreeSet::from([*membership_id]),
        };
        let conditions = match input.domain.retained_source_read_conditions(
            identity.source_namespace_id,
            namespace.ownership.source_owner_generation,
            journal,
            &fence,
        ) {
            Ok(conditions) => conditions,
            Err(NativeSecurityError::Access(
                AccessError::LiveRevocation | AccessError::LivePurge | AccessError::LiveShadowed,
            )) => continue,
            Err(NativeSecurityError::Access(error)) => return Err(error),
            Err(_) => return Err(AccessError::SecurityFailClosed),
        };
        let control = NativeRetentionControl::bind_for_projection_metadata(
            native,
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
        .map_err(|_| AccessError::SnapshotStale)?;
        let (metadata, _) = control
            .load_retained_revision(&expected, native.context())
            .map_err(|_| AccessError::SnapshotStale)?;
        reads.push(
            read_published_projection_metadata_v1(
            native,
                &control,
                &snapshot,
                input.registry,
                projection_id,
                &expected,
                &metadata,
                input.config,
            )
            .map_err(|_| AccessError::SnapshotStale)?,
        );
    }
    // Pay for the copied bounded reference maps and selected metadata before capture.
    let metadata_bytes = snapshot
        .manifests()
        .len()
        .checked_add(snapshot.shadows().len())
        .and_then(|count| count.checked_add(reads.len()))
        .and_then(|count| {
            count.checked_mul(3 * search_control_redb::JournalLimits::BASELINE.max_value_bytes)
        })
        .ok_or(AccessError::EligibilityPopulationTooLarge)?;
    for unit in [
        Work::Allocate(metadata_bytes),
        Work::Process(metadata_bytes),
    ] {
        native
            .work(unit)
            .map_err(|_| AccessError::RetrievalLegBudgetExceeded)?;
    }
    let population = CanonicalPopulationSnapshot::capture(
        &snapshot,
        input.schema,
        input.route,
        input.fence,
        &reads,
    )?;
    let after = journal
        .bind_control_port::<FreshRootCancellation>(publisher)
        .map_err(|_| AccessError::SnapshotStale)?
        .read_publication_projection_snapshot_with_original_context(deadline, &work)
        .map_err(|_| AccessError::SnapshotStale)?
        .ok_or(AccessError::SnapshotStale)?;
    if snapshot != after {
        return Err(AccessError::SnapshotStale);
    }
    native
        .source_full_checkpoint()
        .map_err(|_| AccessError::SecurityFailClosed)?;
    Ok(population)
}
