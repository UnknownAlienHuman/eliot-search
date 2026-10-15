//! Current permanent-v2 Source admission over the sole semantic registry.
use super::{SourceRegistry, batch};
use crate::error::{JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry};
use crate::source::AdmittedSourceReceipt;
use crate::source_ids::SourceIdentityIdPortV2;
use search_contracts::{
    Blake3Digest32, BoundedSet, NamespaceOwnershipStatus, NonZeroRevision, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerGeneration,
};
use search_ports::{CancellationProbe, OperationContext, PortErrorKind, PortFailure};
use search_source_admission::{AdmissionOutcome, policy_v2::CheckpointFailureV2};
use search_source_identity::{
    CreationPolicy, IdentityDraft, IdentityError, IdentityResolution, PriorIdentityCandidate,
    PriorIdentityCandidates, ResolutionPolicy, SourceBinding, derive_source_identity,
    resolve_identity,
};
use search_source_records::source_request_v2::{MemoryOriginals, source_receipt};
use search_source_records::{
    batch::{DecodedBatch, RegistryStateDelta, RegistryStateKey, RegistryStateRecord},
    identity_lookup_v2::{IdentityLookupV2Delta, SourceIdentityLookupV2},
    namespace::NamespaceCutoverState,
    provenance::PermanentSourceAdmissionV2,
    source::{RegisteredSource, SourceLifecycle},
    source_request_v2::{SourceAdmissionRequestV2, SourceNamespaceSelectionV2},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub use search_source_records::source_request_v2::SourceAdmissionV2Error;
#[derive(Clone)]
pub(super) struct IdentityCacheV2 {
    lookup: SourceIdentityLookupV2,
    namespaces: BTreeSet<SourceNamespaceId>,
}
impl std::fmt::Debug for IdentityCacheV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdentityCacheV2")
    }
}

/// A genuine semantic preparation; public decoded metadata cannot construct it.
#[derive(Clone)]
pub struct PreparedSourceAdmissionV2 {
    batch: batch::PreparedBatch,
    result: AdmittedSourceReceipt,
    lookup_delta: IdentityLookupV2Delta,
}
impl PreparedSourceAdmissionV2 {
    /// Exact normal-control transaction to persist with retained originals.
    pub const fn batch(&self) -> &batch::PreparedBatch {
        &self.batch
    }
    /// The proposed result is not committed until exact readback succeeds.
    pub const fn proposed_receipt(&self) -> &AdmittedSourceReceipt {
        &self.result
    }
}

/// Preparation either returns an original acknowledgment or one new transaction.
pub enum SourceAdmissionPreparationV2 {
    /// Exact immutable operation was already durably committed.
    Replayed(AdmittedSourceReceipt),
    /// Complete state and identity preparation, without a durable success claim.
    Prepared(PreparedSourceAdmissionV2),
}

use search_source_records::source_request_v2::source_admission_checkpoint as checkpoint;

/// Prepares a Source under actual current Root/policy/namespace/predecessor fences.
/// No ID is drawn until every refusal that can be checked without an ID passes.
pub fn prepare_source_admission_v2<C, P>(
    registry: &mut SourceRegistry,
    request: &SourceAdmissionRequestV2,
    port: &mut P,
    context: &OperationContext<C>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<SourceAdmissionPreparationV2, SourceAdmissionV2Error<P::Error>>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C> + SourceIdentityIdPortV2<Cancellation = C>,
{
    context
        .preflight()
        .map_err(|_| RegistryError::CancelledBeforeCommit)?;
    checkpoint(probe)?;
    let decision = request.original.receipt().decision();
    if decision.outcome() != AdmissionOutcome::Allow {
        return Err(SourceAdmissionV2Error::Decision(decision.outcome()));
    }
    let empty = PriorIdentityCandidates::new(Vec::new())?;
    let policy = ResolutionPolicy::new(
        CreationPolicy::PermitExactNewIdentity,
        3,
        context.cancellation().is_cancelled(),
    )?;
    let IdentityResolution::CreateNew(draft) =
        resolve_identity(&request.identity_observation, &empty, policy)?
    else {
        return Err(IdentityError::SourceIdentityInsufficientEvidence.into());
    };
    let fingerprint = request.fingerprint(&draft.stable_identity_components, probe)?;
    match port
        .load_batch(&request.operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    {
        crate::recovery::RegistryBatchLookup::Committed(observed) => {
            let record = observed.record();
            if record.encoding_profile() != "eliot.registry-batch.v4"
                || record.entry().operation_id != request.operation_id
                || record.entry().mutation_digest != fingerprint
                || record.request_fingerprint() != fingerprint
            {
                return Err(RegistryError::OperationConflict.into());
            }
            observed.verify_operation()?;
            return Ok(SourceAdmissionPreparationV2::Replayed(source_receipt(
                record, true,
            )?));
        }
        crate::recovery::RegistryBatchLookup::Absent { revision }
            if revision != registry.revision =>
        {
            return Err(RegistryError::RegistryRevisionConflict.into());
        }
        crate::recovery::RegistryBatchLookup::Absent { .. } => {}
    }
    if registry.pending_batch.is_some() {
        return Err(RegistryError::MutationOutcomeUnknown.into());
    }
    if request.expected_registry_revision != registry.revision {
        return Err(RegistryError::RegistryRevisionConflict.into());
    }
    if request
        .owner
        .installation_incarnation
        .as_bytes()
        .iter()
        .all(|value| *value == 0)
    {
        return Err(RegistryError::NamespaceOwnershipConflict.into());
    }
    let root = registry
        .roots
        .get(&request.root_binding_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    if root.status != crate::root::RootStatus::Active
        || root.record_revision != request.expected_root_revision
    {
        return Err(RegistryError::RootPolicyGenerationMismatch.into());
    }
    if root.policy_revision != decision.policy_revision()
        || root.policy_fingerprint.as_bytes() != decision.policy_fingerprint().as_bytes()
    {
        return Err(RegistryError::AdmissionReceiptStale.into());
    }
    let identity = request.identity_observation.as_inner();
    let binding = &request.binding_observation;
    if identity.candidate_catalog_revision.get() != registry.revision
        || identity.path_key.root_binding_id() != request.root_binding_id
        || binding.root_binding_id != request.root_binding_id
        || identity.path_key.as_str() != binding.relative_path.as_str()
        || identity.profile_revision != request.identity_profile.revision()
        || identity.profile_schema_digest != request.identity_profile.schema_digest()
        || identity.path_key.profile_revision() != request.identity_profile.revision()
        || identity.path_key.profile_schema_digest() != request.identity_profile.schema_digest()
        || request.original.observation().metadata().byte_size() != Some(binding.content_bytes)
        || request
            .original
            .observation()
            .relative_locator()
            .map(search_source_admission::observation_v2::AdmissionRelativeLocator::as_bytes)
            != Some(binding.relative_path.as_str().as_bytes())
        || identity
            .byte_length_hint
            .is_some_and(|v| v != binding.content_bytes)
        || identity
            .content_digest_hint
            .is_some_and(|v| v != binding.content_digest)
    {
        return Err(RegistryError::AdmissionBindingMismatch.into());
    }
    if registry.operations.len() >= registry.limits.max_operations {
        return Err(RegistryError::CapacityExceeded.into());
    }
    if registry.identity_cache_v2.is_none() {
        registry.identity_cache_v2 = Some(IdentityCacheV2 {
            lookup: SourceIdentityLookupV2::rebuild(&registry.sources, &registry.namespaces)?,
            namespaces: registry
                .sources
                .keys()
                .map(|v| v.source_namespace_id)
                .chain(registry.namespaces.keys().copied())
                .collect(),
        });
    }
    let after = registry
        .revision
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let (namespace_id, existing_namespace) = match request.namespace {
        SourceNamespaceSelectionV2::Existing {
            id,
            revision,
            generation,
        } => {
            let namespace = registry
                .namespaces
                .get(&id)
                .ok_or(RegistryError::NamespaceOwnershipConflict)?;
            let metadata = namespace
                .permanent_metadata()
                .ok_or(RegistryError::CutoverRequired)?;
            if namespace.ownership.status != NamespaceOwnershipStatus::Active
                || namespace.fenced
                || namespace.preparation.is_some()
                || namespace.activation.is_some()
                || namespace.ownership.ownership_record_revision != revision
                || namespace.ownership.source_owner_generation != generation
                || namespace.ownership.owner_system_id != request.owner.system_id
                || namespace.ownership.owner_installation_incarnation_id
                    != request.owner.installation_incarnation
                || namespace.ownership.owner_epoch != request.owner.epoch
                || metadata.root_binding_id() != request.root_binding_id
                || metadata.identity_profile() != request.identity_profile
            {
                return Err(RegistryError::NamespaceOwnershipConflict.into());
            }
            request
                .original
                .verify_current(metadata.policy().compiled(), probe)?;
            (Some(id), Some(namespace.clone()))
        }
        SourceNamespaceSelectionV2::CreateNew => {
            if request.expected_source_revision.is_some()
                || identity.claimed_source_id.is_some()
                || registry.namespaces.len() >= registry.limits.max_namespaces
            {
                return Err(RegistryError::NamespaceOwnershipConflict.into());
            }
            (None, None)
        }
    };
    checkpoint(probe)?;
    prepare_identity(
        registry,
        request,
        port,
        context,
        probe,
        *draft,
        fingerprint,
        after,
        namespace_id,
        existing_namespace,
    )
}

fn prepare_identity<C, P>(
    registry: &SourceRegistry,
    request: &SourceAdmissionRequestV2,
    port: &mut P,
    context: &OperationContext<C>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    draft: IdentityDraft,
    fingerprint: Blake3Digest32,
    after: u64,
    namespace_id: Option<SourceNamespaceId>,
    namespace: Option<NamespaceCutoverState>,
) -> Result<SourceAdmissionPreparationV2, SourceAdmissionV2Error<P::Error>>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C> + SourceIdentityIdPortV2<Cancellation = C>,
{
    let cache = registry
        .identity_cache_v2
        .as_ref()
        .ok_or(RegistryError::SnapshotInvalid)?;
    let material = namespace_id
        .and_then(|id| {
            cache.lookup.lookup_material(
                id,
                draft.stable_key.identity_kind(),
                &draft.stable_identity_components,
            )
        })
        .cloned();
    let path = cache
        .lookup
        .lookup_path(&request.identity_observation.as_inner().path_key);
    if path.is_some() && path != material.as_ref() {
        return Err(RegistryError::SourceBindingConflict.into());
    }
    if request
        .identity_observation
        .as_inner()
        .claimed_source_id
        .is_some_and(|id| {
            material
                .as_ref()
                .is_none_or(|source| source.source_id != id)
        })
    {
        return Err(RegistryError::SourceBindingConflict.into());
    }
    let previous = material
        .as_ref()
        .map(|id| {
            registry
                .sources
                .get(id)
                .ok_or(RegistryError::SourceNotFound)
        })
        .transpose()?;
    let mut prior = Vec::new();
    if let Some(previous) = previous {
        if previous.lifecycle() != SourceLifecycle::Active {
            return Err(RegistryError::SourceRetired.into());
        }
        if previous.permanent_admission().is_none() {
            return Err(RegistryError::CutoverRequired.into());
        }
        if request.expected_source_revision != Some(previous.source_revision()) {
            return Err(RegistryError::SourceRevisionConflict.into());
        }
        let mut paths = Vec::new();
        if previous.binding().relative_path() == &request.binding_observation.relative_path {
            paths.push(request.identity_observation.as_inner().path_key.clone());
        }
        prior.push(PriorIdentityCandidate {
            identity: previous.identity().clone(),
            stable_key: draft.stable_key,
            active_paths: BoundedSet::from_items(paths)
                .map_err(|_| RegistryError::CapacityExceeded)?,
            closed_paths: BoundedSet::empty(),
        });
    } else if request.expected_source_revision.is_some() {
        return Err(RegistryError::SourceNotFound.into());
    }
    if previous.is_none() && registry.sources.len() >= registry.limits.max_sources {
        return Err(RegistryError::CapacityExceeded.into());
    }
    let prior = PriorIdentityCandidates::new(prior)?;
    let resolution = resolve_identity(
        &request.identity_observation,
        &prior,
        ResolutionPolicy::new(
            CreationPolicy::PermitExactNewIdentity,
            3,
            context.cancellation().is_cancelled(),
        )?,
    )?;
    let next_source_revision = previous
        .map_or_else(
            || NonZeroRevision::new(1),
            |v| v.source_revision().checked_next(),
        )
        .map_err(|_| RegistryError::ContractExhausted)?;
    let next_binding_revision = previous
        .map_or_else(
            || NonZeroRevision::new(1),
            |v| v.binding().binding_revision().checked_next(),
        )
        .map_err(|_| RegistryError::ContractExhausted)?;
    let next_observation_revision = previous
        .map_or_else(
            || NonZeroRevision::new(1),
            |v| v.binding().observation_revision().checked_next(),
        )
        .map_err(|_| RegistryError::ContractExhausted)?;
    checkpoint(probe)?;
    let created_namespace = namespace_id.is_none();
    let namespace_id = if let Some(id) = namespace_id {
        id
    } else {
        let id = port
            .fresh_source_namespace_id(context)
            .map_err(SourceAdmissionV2Error::Allocation)?;
        if !fresh_uuid(id.as_bytes()) || cache.namespaces.contains(&id) {
            return Err(RegistryError::NamespaceOwnershipConflict.into());
        }
        id
    };
    let assigned = match resolution {
        IdentityResolution::MatchExisting { source_id, .. } => {
            let id = material.as_ref().ok_or(RegistryError::SourceNotFound)?;
            if id.source_id != source_id {
                return Err(RegistryError::SourceBindingConflict.into());
            }
            id.clone()
        }
        IdentityResolution::CreateNew(draft) => {
            let id = port
                .fresh_source_id(context)
                .map_err(SourceAdmissionV2Error::Allocation)?;
            if !fresh_uuid(id.as_bytes()) || cache.lookup.lookup_id(namespace_id, id).is_some() {
                return Err(RegistryError::SourceAlreadyAdmittedConflict.into());
            }
            derive_source_identity(*draft, namespace_id, id)?
        }
        _ => return Err(IdentityError::SourceIdentityInsufficientEvidence.into()),
    };
    checkpoint(probe)?;
    let namespace = if let Some(namespace) = namespace {
        namespace
    } else {
        let mut owner = SourceNamespaceOwnership {
            source_namespace_id: namespace_id,
            owner_system_id: request.owner.system_id.clone(),
            owner_installation_incarnation_id: request.owner.installation_incarnation,
            owner_epoch: request.owner.epoch,
            ownership_record_revision: NonZeroRevision::new(1)
                .map_err(|_| RegistryError::ContractExhausted)?,
            source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
            source_admission_policy_revision: search_contracts::PolicyRevision::new(
                request.original.policy().compiled().revision().get(),
            ),
            status: NamespaceOwnershipStatus::Active,
            cutover_receipt_ref: None,
        };
        owner.source_owner_generation =
            search_domain::source_ownership_v2::source_owner_generation_v2(&owner)
                .map_err(|_| RegistryError::NamespaceOwnershipConflict)?;
        NamespaceCutoverState::new_v2_metadata(
            owner,
            request.root_binding_id,
            request.identity_profile,
            Arc::clone(request.original.policy()),
            search_source_provenance::OriginalInputsLayoutV2::policy_reference(
                request.original.policy(),
                probe,
            )?,
            after,
            request.receipt.clone(),
            probe,
        )
        .map_err(|v| match v {
            search_source_records::namespace::NamespaceMetadataError::Registry(v) => {
                SourceAdmissionV2Error::Registry(v)
            }
            search_source_records::namespace::NamespaceMetadataError::Admission(v) => {
                SourceAdmissionV2Error::Admission(v)
            }
        })?
    };
    let bound = SourceBinding::new(
        assigned.clone(),
        request.binding_observation.clone(),
        next_binding_revision,
        next_observation_revision,
    );
    let evidence = PermanentSourceAdmissionV2::new(
        request.original.clone(),
        request.expected_root_revision,
        namespace.ownership.ownership_record_revision,
        namespace.ownership.source_owner_generation,
        probe,
    )?;
    let source = RegisteredSource::new_permanent(
        bound,
        evidence,
        SourceLifecycle::Active,
        next_source_revision,
        after,
        request.receipt.clone(),
    )?;
    let scope = BTreeMap::from([(namespace_id, namespace.clone())]);
    let lookup_delta = cache
        .lookup
        .prepare_change(previous, Some(&source), &scope)?;
    let mut deltas = vec![RegistryStateDelta::new(
        RegistryStateKey::Source(assigned),
        previous.cloned().map(RegistryStateRecord::Source),
        RegistryStateRecord::Source(source),
    )?];
    if created_namespace {
        deltas.push(RegistryStateDelta::new(
            RegistryStateKey::Namespace(namespace_id),
            None,
            RegistryStateRecord::Namespace(namespace),
        )?);
    }
    deltas.sort_by(|a, b| a.key().cmp(b.key()));
    let mut original_values = vec![request.original.clone()];
    if let Some(previous) = previous {
        original_values.push(
            previous
                .permanent_admission()
                .ok_or(RegistryError::CutoverRequired)?
                .original()
                .clone(),
        );
    }
    let mut originals = MemoryOriginals::new(original_values);
    let record = DecodedBatch::from_observed_parts_v4(
        RegistryJournalEntry::new(
            request.operation_id.clone(),
            fingerprint,
            registry.revision,
            after,
            JournalEntryKind::BatchApply,
            request.receipt.clone(),
        ),
        fingerprint,
        deltas.len(),
        deltas,
        registry.limits,
        &mut originals,
        probe,
    )?;
    let result = source_receipt(&record, false)?;
    let prepared = batch::prepare_v4_record(registry, record)?;
    checkpoint(probe)?;
    Ok(SourceAdmissionPreparationV2::Prepared(
        PreparedSourceAdmissionV2 {
            batch: prepared,
            result,
            lookup_delta,
        },
    ))
}

fn fresh_uuid(value: &[u8; 16]) -> bool {
    value[6] & 0xf0 == 0x40 && value[8] & 0xc0 == 0x80
}

/// Commits one already prepared source without redrawing identity or changing inputs.
pub fn commit_source_admission_v2<C, P>(
    registry: &mut SourceRegistry,
    prepared: &PreparedSourceAdmissionV2,
    control: &mut P,
    context: &OperationContext<C>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmittedSourceReceipt, SourceAdmissionV2Error<P::Error>>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    context
        .preflight()
        .map_err(|_| RegistryError::CancelledBeforeCommit)?;
    checkpoint(probe)?;
    if registry.pending_batch.is_some() {
        return Err(RegistryError::MutationOutcomeUnknown.into());
    }
    if registry.revision != prepared.batch.expected_registry_revision() {
        return Err(RegistryError::RegistryRevisionConflict.into());
    }
    batch::validate_install(registry, prepared.batch.record())?;
    let Some(mut cache) = registry.identity_cache_v2.take() else {
        return Err(RegistryError::SnapshotInvalid.into());
    };
    let record = prepared.batch.record();
    let mutation = crate::error::registry_mutation(&record.entry().operation_id);
    let observed = match control.commit_batch(&prepared.batch, context, &mutation) {
        Ok(observed) => observed,
        Err(error) => {
            registry.identity_cache_v2 = Some(cache);
            if !matches!(
                error.kind(),
                PortErrorKind::CancelledBeforeSideEffect
                    | PortErrorKind::CompareAndSwapRejected
                    | PortErrorKind::StaleGeneration
                    | PortErrorKind::Conflict
                    | PortErrorKind::InvalidInput
                    | PortErrorKind::Unauthorized
                    | PortErrorKind::ResourceExhausted
                    | PortErrorKind::InvalidContext
                    | PortErrorKind::DeadlineBeforeStart
            ) {
                registry.pending_batch = Some((
                    record.entry().operation_id.clone(),
                    record.request_fingerprint(),
                ));
            }
            return Err(SourceAdmissionV2Error::Control(error));
        }
    };
    if observed.record() != record
        || observed.verify().is_err()
        || checkpoint::<P::Error>(probe).is_err()
    {
        registry.identity_cache_v2 = Some(cache);
        registry.pending_batch = Some((
            record.entry().operation_id.clone(),
            record.request_fingerprint(),
        ));
        return Err(RegistryError::MutationOutcomeUnknown.into());
    }
    cache.lookup.install(prepared.lookup_delta.clone());
    cache
        .namespaces
        .insert(prepared.result.source_identity.source_namespace_id);
    batch::install_prepared(registry, &prepared.batch);
    registry.identity_cache_v2 = Some(cache);
    Ok(prepared.result.clone())
}

pub use search_source_records::source_request_v2::SourceAdmissionRecoveryV2;
/// Resolves a possible commit by exact readback, never by redrawing an identity.
pub fn recover_source_admission_v2<C, P>(
    registry: &mut SourceRegistry,
    prepared: &PreparedSourceAdmissionV2,
    control: &P,
    context: &OperationContext<C>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<SourceAdmissionRecoveryV2, SourceAdmissionV2Error<P::Error>>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    checkpoint(probe)?;
    let record = prepared.batch.record();
    let pending = (
        record.entry().operation_id.clone(),
        record.request_fingerprint(),
    );
    if registry
        .pending_batch
        .as_ref()
        .is_some_and(|existing| existing != &pending)
    {
        return Err(RegistryError::MutationOutcomeUnknown.into());
    }
    let observed = control
        .load_batch(&record.entry().operation_id, context)
        .map_err(SourceAdmissionV2Error::Control)?;
    match observed {
        crate::recovery::RegistryBatchLookup::Absent { revision }
            if revision == record.expected_registry_revision() && registry.revision == revision =>
        {
            checkpoint(probe)?;
            registry.pending_batch = None;
            Ok(SourceAdmissionRecoveryV2::RetrySamePrepared)
        }
        crate::recovery::RegistryBatchLookup::Committed(readback)
            if readback.record() == record && readback.verify().is_ok() =>
        {
            checkpoint(probe)?;
            batch::validate_install(registry, record)?;
            let mut cache = registry.identity_cache_v2.take();
            if let Some(cache) = &mut cache {
                cache.lookup.install(prepared.lookup_delta.clone());
                cache
                    .namespaces
                    .insert(prepared.result.source_identity.source_namespace_id);
            }
            batch::install_prepared(registry, &prepared.batch);
            registry.identity_cache_v2 = cache;
            let mut receipt = prepared.result.clone();
            receipt.replayed = true;
            Ok(SourceAdmissionRecoveryV2::Committed(receipt))
        }
        _ => Err(RegistryError::MutationOutcomeUnknown.into()),
    }
}
