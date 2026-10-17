//! C03 first-bind persistence. Upstream representation and partition truth stays external.

use super::{RegistryBatch, RegistryChange, RegistryOperation, SourceRegistry, batch};
use crate::error::{RegistryControlPort, RegistryError, cancelled_before_commit};
use search_contracts::{
    AccessPartitionId, OpaqueId, ProjectionMembership, ProjectionMembershipId, ReceiptRef,
    ScoringPartitionId, SourceMembershipId,
};
use search_ports::{CancellationProbe, OperationContext};
use std::collections::BTreeMap;

pub use search_source_records::projection_membership::{
    ProjectionMembershipBindRequest, ProjectionMembershipRecord, ProjectionPartitionSemantics,
};

/// Existing partition IDs whose complete canonical meaning matches one new C03 request.
///
/// Missing classes require Root's qualified entropy owner to allocate an ID
/// after original-operation recovery and before the C02 preparation record is
/// committed. This read result is metadata only and grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReusableProjectionPartitions {
    access: Option<AccessPartitionId>,
    scoring: Option<ScoringPartitionId>,
}

impl ReusableProjectionPartitions {
    /// Existing access class with byte-identical canonical meaning.
    #[must_use]
    pub const fn access(&self) -> Option<AccessPartitionId> {
        self.access
    }

    /// Existing scoring class with byte-identical canonical meaning.
    #[must_use]
    pub const fn scoring(&self) -> Option<ScoringPartitionId> {
        self.scoring
    }
}

/// Resolve shared partition classes by exact canonical meaning from retained C03 rows.
///
/// The caller resolves its original C03 operation before invoking this method
/// and performs any missing typed-ID draw only after this exact scan. Historical
/// rows without class semantics cannot prove equality and are skipped.
pub fn reusable_projection_partitions(
    registry: &SourceRegistry,
    semantics: &ProjectionPartitionSemantics,
) -> Result<ReusableProjectionPartitions, RegistryError> {
    let mut access = None;
    let mut scoring = None;
    for record in registry.projection_memberships.values() {
        let Some(existing) = &record.request().partition_semantics else {
            continue;
        };
        if existing.access() == semantics.access() {
            let id = record.binding().access_partition_id;
            if access.is_some_and(|prior| prior != id) {
                return Err(RegistryError::ProjectionMembershipConflict);
            }
            access = Some(id);
        }
        if existing.scoring() == semantics.scoring() {
            let id = record.binding().scoring_partition_id;
            if scoring.is_some_and(|prior| prior != id) {
                return Err(RegistryError::ProjectionMembershipConflict);
            }
            scoring = Some(id);
        }
    }
    Ok(ReusableProjectionPartitions { access, scoring })
}

/// Original complete request retained across an unknown outcome or retry.
#[derive(Clone, Debug)]
pub struct PreparedProjectionMembership {
    batch: RegistryBatch,
    receipt: ReceiptRef,
}

impl PreparedProjectionMembership {
    /// Full original operation, including actual upstream projection UUID.
    pub const fn batch(&self) -> &RegistryBatch {
        &self.batch
    }
}

/// Exact original durable outcome; neither access nor representation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembershipReceipt {
    /// Original C00 coordinates, never regenerated on replay.
    pub binding: ProjectionMembership,
    /// Registry revision of the original bind.
    pub registry_revision: u64,
    /// Original operation identity.
    pub operation_id: OpaqueId,
    /// Original content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether the exact original operation was acknowledged again.
    pub replayed: bool,
}

/// Prepare one first bind under exact current permanent-member owner fences.
///
/// No identity allocation or upstream qualification occurs here. Root must
/// supply those observations. Reuse this exact preparation after an unknown
/// outcome; fresh preparation is intentionally refused when the row exists.
pub fn prepare_projection_membership<C: CancellationProbe>(
    registry: &SourceRegistry,
    request: ProjectionMembershipBindRequest,
    expected_registry_revision: u64,
    operation: RegistryOperation,
    receipt: ReceiptRef,
    context: &OperationContext<C>,
) -> Result<PreparedProjectionMembership, RegistryError> {
    cancelled_before_commit(context)?;
    let prepared = PreparedProjectionMembership {
        batch: RegistryBatch {
            expected_registry_revision,
            operation,
            changes: vec![RegistryChange::BindProjectionMembership {
                request,
                receipt: receipt.clone(),
            }],
        },
        receipt,
    };
    batch::prepare(registry, &prepared.batch, &prepared.receipt)?;
    Ok(prepared)
}

/// Commit or replay the original preparation through the existing atomic port.
pub fn commit_projection_membership<C, P>(
    registry: &mut SourceRegistry,
    prepared: &PreparedProjectionMembership,
    port: &mut P,
    context: &OperationContext<C>,
) -> Result<ProjectionMembershipReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    execute(
        registry,
        prepared.batch.clone(),
        &prepared.receipt,
        port,
        context,
    )
}

/// Bind or replay exact caller input, resolving the operation before collision checks.
///
/// The caller must retain the original expected revision, operation and actual
/// UUID. Different IDs or coordinates under the same operation refuse. A new
/// operation cannot overwrite an existing binding: rebind requires a future
/// explicit expected-old transition and publication contract.
pub fn bind_projection_membership<C, P>(
    registry: &mut SourceRegistry,
    request: ProjectionMembershipBindRequest,
    expected_registry_revision: u64,
    operation: RegistryOperation,
    receipt: &ReceiptRef,
    port: &mut P,
    context: &OperationContext<C>,
) -> Result<ProjectionMembershipReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    execute(
        registry,
        RegistryBatch {
            expected_registry_revision,
            operation,
            changes: vec![RegistryChange::BindProjectionMembership {
                request,
                receipt: receipt.clone(),
            }],
        },
        receipt,
        port,
        context,
    )
}

fn execute<C, P>(
    registry: &mut SourceRegistry,
    request: RegistryBatch,
    receipt: &ReceiptRef,
    port: &mut P,
    context: &OperationContext<C>,
) -> Result<ProjectionMembershipReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    let [
        RegistryChange::BindProjectionMembership {
            request: input,
            receipt: original_receipt,
        },
    ] = request.changes.as_slice()
    else {
        return Err(RegistryError::SnapshotInvalid);
    };
    if original_receipt != receipt {
        return Err(RegistryError::OperationConflict);
    }
    let binding = input.binding.clone();
    cancelled_before_commit(context)?;
    if let batch::RegistryBatchLookup::Committed(observed) = port
        .load_batch(request.operation.operation_id(), context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    {
        observed.verify_operation()?;
        if observed.record().replay_batch().as_ref() != Some(&request)
            || observed.record().entry().receipt != *receipt
        {
            return Err(RegistryError::OperationConflict);
        }
    }
    let result = super::apply_admission_batch(registry, request, receipt, port, context)?;
    let [item] = result.items.as_slice() else {
        return Err(RegistryError::MutationOutcomeUnknown);
    };
    if item.status != search_source_records::operation::BatchItemStatus::Committed {
        return Err(item.reason.unwrap_or(RegistryError::MutationOutcomeUnknown));
    }
    Ok(ProjectionMembershipReceipt {
        binding,
        registry_revision: result.after_revision,
        operation_id: result.operation.operation_id().clone(),
        receipt: result.receipt,
        replayed: result.replayed,
    })
}

/// Read exact binding metadata after current C03 owner-fence validation.
///
/// This does not validate representation/partition meaning, a grant, a route,
/// an epoch or live deny state. C13/C18 and their upstream owners must do so.
pub fn read_projection_membership(
    registry: &SourceRegistry,
    membership: SourceMembershipId,
    expected_registry_revision: u64,
) -> Result<&ProjectionMembershipRecord, RegistryError> {
    if registry.pending_batch.is_some() {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    if registry.revision != expected_registry_revision {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    let record = registry.projection_membership_record(membership)?;
    validate_target(registry, record.request())?;
    Ok(record)
}

fn validate_target(
    registry: &SourceRegistry,
    request: &ProjectionMembershipBindRequest,
) -> Result<(), RegistryError> {
    request.validate_shape()?;
    let id = request.binding.source_membership_id;
    let (member, _) = registry.active_source_membership_v2(id)?;
    let permanent = registry.permanent_membership(id)?;
    if member.membership_revision != request.expected_membership_revision
        || permanent.provenance() != &request.expected_provenance
    {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    Ok(())
}

pub(super) fn stage_bind(
    registry: &SourceRegistry,
    request: &ProjectionMembershipBindRequest,
    after_revision: u64,
    receipt: &ReceiptRef,
) -> Result<ProjectionMembershipRecord, RegistryError> {
    validate_target(registry, request)?;
    if registry
        .projection_memberships
        .contains_key(&request.binding.source_membership_id)
    {
        return Err(RegistryError::ProjectionMembershipConflict);
    }
    if registry
        .reverse_projection_memberships
        .contains_key(&request.binding.projection_membership_id)
    {
        return Err(RegistryError::ProjectionMembershipIdCollision);
    }
    if registry.projection_memberships.len() >= registry.limits.max_memberships {
        return Err(RegistryError::CapacityExceeded);
    }
    if let Some(semantics) = &request.partition_semantics {
        for existing in registry.projection_memberships.values() {
            let old = existing.request().partition_semantics.as_ref();
            let binding = existing.binding();
            let access_same_id = binding.access_partition_id == request.binding.access_partition_id;
            let scoring_same_id =
                binding.scoring_partition_id == request.binding.scoring_partition_id;
            match old {
                Some(old) => {
                    if (old.access() == semantics.access()) != access_same_id
                        || (old.scoring() == semantics.scoring()) != scoring_same_id
                    {
                        return Err(RegistryError::ProjectionMembershipConflict);
                    }
                }
                None if access_same_id || scoring_same_id => {
                    return Err(RegistryError::ProjectionMembershipConflict);
                }
                None => {}
            }
        }
    }
    ProjectionMembershipRecord::from_observed_parts(
        request.clone(),
        after_revision,
        receipt.clone(),
    )
}

pub(super) fn prepare_batch(
    registry: &SourceRegistry,
    request: &RegistryBatch,
    receipt: &ReceiptRef,
) -> Result<batch::PreparedBatch, RegistryError> {
    request.validate(registry.limits)?;
    if registry.revision != request.expected_registry_revision {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    let [
        RegistryChange::BindProjectionMembership {
            request: input,
            receipt: original_receipt,
        },
    ] = request.changes.as_slice()
    else {
        return Err(RegistryError::BatchSizeInvalid);
    };
    if receipt != original_receipt {
        return Err(RegistryError::OperationConflict);
    }
    let after = registry
        .revision
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let state = batch::RegistryStateRecord::ProjectionMembership(stage_bind(
        registry, input, after, receipt,
    )?);
    let entry = crate::RegistryJournalEntry::new(
        request.operation.operation_id().clone(),
        request.operation.mutation_digest(),
        registry.revision,
        after,
        crate::JournalEntryKind::BatchApply,
        receipt.clone(),
    );
    let fingerprint = batch::request_fingerprint(request)?;
    let delta = batch::RegistryStateDelta::new(state.key(), None, state)?;
    let record = if input.partition_semantics.is_some() {
        batch::DecodedBatch::from_observed_parts_v7(
            entry,
            fingerprint,
            vec![delta],
            request.changes.clone(),
            registry.limits,
        )?
    } else {
        batch::DecodedBatch::from_observed_parts_v6(
            entry,
            fingerprint,
            vec![delta],
            request.changes.clone(),
            registry.limits,
        )?
    };
    if input.partition_semantics.is_some() {
        batch::prepare_v7_record(registry, record)
    } else {
        batch::prepare_v6_record(registry, record)
    }
}

pub(super) fn validate_install(
    registry: &SourceRegistry,
    record: &batch::DecodedBatch,
) -> Result<(), RegistryError> {
    let mut staged = BTreeMap::new();
    for delta in record.deltas() {
        if let batch::RegistryStateRecord::ProjectionMembership(binding) = delta.after() {
            let id = binding.binding();
            if delta.before().is_some()
                || registry
                    .reverse_projection_memberships
                    .get(&id.projection_membership_id)
                    .is_some_and(|member| *member != id.source_membership_id)
                || staged
                    .insert(id.projection_membership_id, id.source_membership_id)
                    .is_some_and(|member| member != id.source_membership_id)
            {
                return Err(RegistryError::ProjectionMembershipIdCollision);
            }
            if registry.revision == record.expected_registry_revision() {
                let expected = stage_bind(
                    registry,
                    binding.request(),
                    record.registry_revision_after(),
                    binding.receipt(),
                )?;
                if &expected != binding {
                    return Err(RegistryError::SnapshotInvalid);
                }
            }
        }
    }
    Ok(())
}

/// Rebuild indexes only from already validated complete immutable history.
pub(super) fn restore_bindings(registry: &mut SourceRegistry) -> Result<(), RegistryError> {
    let mut bindings = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    for operation in registry.operations.values() {
        let super::RetainedRegistryOperation::Committed(operation) = operation else {
            continue;
        };
        for delta in operation.deltas() {
            if let batch::RegistryStateRecord::ProjectionMembership(record) = delta.after() {
                let b = record.binding();
                let member = registry.permanent_membership(b.source_membership_id)?;
                if delta.before().is_some()
                    || record.registry_revision() != operation.registry_revision_after()
                    || member.member().membership_revision.get()
                        < record.request().expected_membership_revision.get()
                    || member.provenance() != &record.request().expected_provenance
                    || bindings
                        .insert(b.source_membership_id, record.clone())
                        .is_some()
                    || reverse
                        .insert(b.projection_membership_id, b.source_membership_id)
                        .is_some()
                {
                    return Err(RegistryError::SnapshotInvalid);
                }
            }
        }
    }
    if bindings.len() > registry.limits.max_memberships {
        return Err(RegistryError::CapacityExceeded);
    }
    registry.projection_memberships = bindings;
    registry.reverse_projection_memberships = reverse;
    Ok(())
}

impl SourceRegistry {
    /// Retained exact coordinates, including stale/inactive bindings; not live authority.
    pub fn projection_membership_record(
        &self,
        id: SourceMembershipId,
    ) -> Result<&ProjectionMembershipRecord, RegistryError> {
        self.projection_memberships
            .get(&id)
            .ok_or(RegistryError::ProjectionMembershipNotFound)
    }

    /// Reverse lookup of the original projection UUID, without identity coercion.
    pub fn projection_source_membership(
        &self,
        id: ProjectionMembershipId,
    ) -> Result<SourceMembershipId, RegistryError> {
        self.reverse_projection_memberships
            .get(&id)
            .copied()
            .ok_or(RegistryError::ProjectionMembershipNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{error::DEFAULT_REGISTRY_LIMITS, membership::PermanentMembershipProvenanceV2};
    use search_contracts as c;

    fn canonical(label: &str) -> Vec<u8> {
        c::to_canonical_cbor(&c::CanonicalValue::Text(c::CanonicalText::new(label).unwrap()))
            .unwrap()
            .into_vec()
    }

    fn semantics(access: &str, scoring: &str) -> ProjectionPartitionSemantics {
        ProjectionPartitionSemantics::new(canonical(access), canonical(scoring)).unwrap()
    }

    fn record(
        source: u8,
        projection: u8,
        representation: u8,
        access_partition: u8,
        scoring_partition: u8,
        semantics: ProjectionPartitionSemantics,
    ) -> ProjectionMembershipRecord {
        let namespace = c::SourceNamespaceId::from_bytes([1; 16]);
        let identity = c::SourceIdentity {
            source_namespace_id: namespace,
            source_id: c::SourceId::from_bytes([source; 16]),
            identity_kind: c::SourceIdentityKind::NtfsFile,
            stable_identity_components: c::OpaqueCanonicalBytes::from_validated(vec![source])
                .unwrap(),
        };
        let revision = c::NonZeroRevision::new(1).unwrap();
        let provenance = PermanentMembershipProvenanceV2::from_stored_parts(
            identity.clone(),
            revision,
            c::RootBindingId::from_bytes([3; 16]),
            revision,
            namespace,
            revision,
            c::SourceOwnerGeneration::from_bytes([4; 32]),
            b"policy".to_vec(),
            b"admission".to_vec(),
            revision,
        )
        .unwrap();
        let source_membership_id = c::SourceMembershipId::from_bytes([source; 16]);
        let request = ProjectionMembershipBindRequest {
            binding: c::ProjectionMembership {
                projection_membership_id: c::ProjectionMembershipId::from_bytes([projection; 16]),
                source_membership_id,
                representation_id: c::RepresentationId::from_bytes([representation; 16]),
                access_partition_id: c::AccessPartitionId::from_bytes([access_partition; 16]),
                scoring_partition_id: c::ScoringPartitionId::from_bytes([scoring_partition; 16]),
                projection_schema_id: c::ProfileId::new("test-projection-schema").unwrap(),
            },
            expected_membership_revision: c::MembershipRevision::new(1),
            expected_provenance: provenance,
            partition_semantics: Some(semantics),
        };
        ProjectionMembershipRecord::from_observed_parts(
            request,
            1,
            c::ReceiptRef::new(format!("receipt:{source}")).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn partition_reuse_is_full_semantic_and_conflicting_ids_refuse() {
        let baseline = semantics("access:read-v1", "scoring:code-v1");
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).unwrap();
        for (source, projection) in [(11, 21), (12, 22)] {
            let record = record(source, projection, source + 20, 7, 8, baseline.clone());
            registry
                .projection_memberships
                .insert(record.binding().source_membership_id, record);
        }

        let reused = reusable_projection_partitions(&registry, &baseline).unwrap();
        assert_eq!(reused.access(), Some(c::AccessPartitionId::from_bytes([7; 16])));
        assert_eq!(reused.scoring(), Some(c::ScoringPartitionId::from_bytes([8; 16])));

        let changed_access = semantics("access:deny-v2", "scoring:code-v1");
        let changed = reusable_projection_partitions(&registry, &changed_access).unwrap();
        assert_eq!(changed.access(), None);
        assert_eq!(changed.scoring(), Some(c::ScoringPartitionId::from_bytes([8; 16])));

        let mut conflicting = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).unwrap();
        for (source, projection, access_partition) in [(11, 21, 7), (12, 22, 9)] {
            let record = record(source, projection, source + 20, access_partition, 8, baseline.clone());
            conflicting
                .projection_memberships
                .insert(record.binding().source_membership_id, record);
        }
        assert!(matches!(
            reusable_projection_partitions(&conflicting, &baseline),
            Err(RegistryError::ProjectionMembershipConflict)
        ));
    }
}
