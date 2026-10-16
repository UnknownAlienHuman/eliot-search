//! Closed Native namespace creation before any Source body is acquired.
//!
//! The existing C03 namespace owner, UUID port, permanent-v2 metadata and v4
//! transaction are reused. Creation admits no Source, initializes no access
//! policy and grants no body/key use. The normal caller must next complete real
//! C18 genesis/publication/invalidation before Source acquisition.

use std::sync::Arc;
use search_contracts::{self as c, CanonicalValue as V};
use search_ports::{CancellationProbe, OperationContext};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_identity::FilesystemIdentityProfile;
use search_source_provenance::{OriginalInputsLayoutV2, RetainedPolicyV2};
use search_source_records::{namespace::NamespaceCutoverState, source_request_v2::{SourceOwnerScopeV2, source_admission_checkpoint as checkpoint}};
use super::{SourceRegistry, batch, source_v2::SourceAdmissionV2Error};
use crate::{error::{JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry}, source_ids::SourceIdentityIdPortV2};

/// Complete original metadata for the qualified Native baseline.
/// No Source path/body, UUID, timestamp or ready flag is caller supplied.
#[derive(Clone, Debug)]
pub struct NativeNamespaceRequestV2 {
    /// Expected semantic registry revision for a fresh transaction only.
    pub expected_registry_revision: u64,
    /// Actual already admitted Root and its exact policy predecessor.
    pub root_binding_id: c::RootBindingId,
    /// Exact Root predecessor, distinct from the global journal generation.
    pub expected_root_revision: c::NonZeroRevision,
    /// Actual qualified namespace owner, separate from invocation guards.
    pub owner: SourceOwnerScopeV2,
    /// Closed qualified Native identity profile.
    pub identity_profile: FilesystemIdentityProfile,
    /// Complete retained, compiled admission policy.
    pub policy: Arc<RetainedPolicyV2>,
    /// Original namespace-initialization operation, distinct from admission.
    pub operation_id: c::OpaqueId,
    /// Original content-free consumer receipt.
    pub receipt: c::ReceiptRef,
}

/// Owner-prepared creation, never a committed namespace or security authority.
#[derive(Clone, Debug)]
pub struct PreparedNativeNamespaceV2 { batch: batch::PreparedBatch }
impl PreparedNativeNamespaceV2 {
    /// Exact immutable command retained for uncertain-outcome recovery.
    pub const fn batch(&self) -> &batch::PreparedBatch { &self.batch }
}

/// Exact verified creation operation and original namespace metadata.
/// Historical replay alone is not current Native/security permission.
#[derive(Clone, Debug)]
pub struct CommittedNativeNamespaceV2 {
    namespace: NamespaceCutoverState,
    operation_id: c::OpaqueId,
    replayed: bool,
}
impl CommittedNativeNamespaceV2 {
    /// Original complete namespace, including retained Root/policy/owner binding.
    pub const fn namespace(&self) -> &NamespaceCutoverState { &self.namespace }
    /// Exact creation operation, whose delta proves namespace absence beforehand.
    pub const fn operation_id(&self) -> &c::OpaqueId { &self.operation_id }
    /// Whether this acknowledgment describes a historical creation.
    pub const fn replayed(&self) -> bool { self.replayed }
}

/// Exact replay or a fresh bounded owner preparation.
#[derive(Clone, Debug)]
pub enum NativeNamespacePreparationV2 {
    /// Original durable creation, with no entropy redrawn.
    Replayed(CommittedNativeNamespaceV2),
    /// Metadata only; security and Source access still remain closed.
    Prepared(PreparedNativeNamespaceV2),
}

/// Prepares a namespace under the actual Root/policy/owner before Source I/O.
/// Exact operation lookup precedes UUID allocation; missing old security is not
/// inferred or initialized by this function.
pub fn prepare_native_namespace_v2<C, P>(registry: &mut SourceRegistry,
    request: &NativeNamespaceRequestV2, port: &mut P, context: &OperationContext<C>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<NativeNamespacePreparationV2, SourceAdmissionV2Error<P::Error>>
where C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C> + SourceIdentityIdPortV2<Cancellation = C>,
{
    context.preflight().map_err(|_| RegistryError::CancelledBeforeCommit)?;
    checkpoint(probe)?;
    // This additive profile is the existing Native NTFS baseline, not a new
    // generic allocator or permission to admit arbitrary identity profiles.
    if request.identity_profile != search_source_identity::ntfs_source_profile_v1()?
        || request.owner.installation_incarnation.as_bytes() == &[0; 16] {
        return Err(RegistryError::NamespaceOwnershipConflict.into());
    }
    let policy_reference = OriginalInputsLayoutV2::policy_reference(&request.policy, probe)?;
    let fingerprint = fingerprint(request, &policy_reference)?;
    match port.load_batch(&request.operation_id, context).map_err(|_| RegistryError::MutationOutcomeUnknown)? {
        super::RegistryBatchLookup::Committed(observed) => {
            observed.verify_operation()?;
            let record = observed.record();
            if record.request_fingerprint() != fingerprint || record.entry().mutation_digest != fingerprint
                || record.entry().operation_id != request.operation_id {
                return Err(RegistryError::OperationConflict.into());
            }
            return Ok(NativeNamespacePreparationV2::Replayed(creation(record, true)?));
        }
        super::RegistryBatchLookup::Absent { revision } if revision != registry.revision => {
            return Err(RegistryError::RegistryRevisionConflict.into());
        }
        super::RegistryBatchLookup::Absent { .. } => {}
    }
    if registry.pending_batch.is_some() { return Err(RegistryError::MutationOutcomeUnknown.into()); }
    if request.expected_registry_revision != registry.revision { return Err(RegistryError::RegistryRevisionConflict.into()); }
    let root = registry.roots.get(&request.root_binding_id).ok_or(RegistryError::RootNotRegistered)?;
    if root.status != crate::root::RootStatus::Active || root.record_revision != request.expected_root_revision
        || root.policy_revision != request.policy.compiled().revision()
        || root.policy_fingerprint.as_bytes() != request.policy.fingerprint().as_bytes() {
        return Err(RegistryError::RootPolicyGenerationMismatch.into());
    }
    if registry.namespaces.len() >= registry.limits.max_namespaces
        || registry.namespaces.values().any(|namespace| namespace.permanent_metadata().is_some_and(|metadata|
            metadata.root_binding_id() == request.root_binding_id && metadata.identity_profile() == request.identity_profile)) {
        return Err(RegistryError::NamespaceOwnershipConflict.into());
    }
    let after = registry.revision.checked_add(1).ok_or(RegistryError::RegistryRevisionOverflow)?;
    let namespace_id = port.fresh_source_namespace_id(context).map_err(SourceAdmissionV2Error::Allocation)?;
    checkpoint(probe)?;
    if namespace_id.as_bytes()[6] & 0xf0 != 0x40 || namespace_id.as_bytes()[8] & 0xc0 != 0x80
        || registry.namespaces.contains_key(&namespace_id)
        || registry.sources.keys().any(|identity| identity.source_namespace_id == namespace_id) {
        return Err(RegistryError::NamespaceOwnershipConflict.into());
    }
    let mut owner = c::SourceNamespaceOwnership {
        source_namespace_id: namespace_id, owner_system_id: request.owner.system_id.clone(),
        owner_installation_incarnation_id: request.owner.installation_incarnation, owner_epoch: request.owner.epoch,
        ownership_record_revision: c::NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?,
        source_owner_generation: c::SourceOwnerGeneration::from_bytes([0; 32]),
        source_admission_policy_revision: c::PolicyRevision::new(request.policy.compiled().revision().get()),
        status: c::NamespaceOwnershipStatus::Active, cutover_receipt_ref: None,
    };
    owner.source_owner_generation = search_domain::source_ownership_v2::source_owner_generation_v2(&owner)
        .map_err(|_| RegistryError::NamespaceOwnershipConflict)?;
    let namespace = NamespaceCutoverState::new_v2_metadata(owner, request.root_binding_id,
        request.identity_profile, Arc::clone(&request.policy), policy_reference.clone(),
        after, request.receipt.clone(), probe).map_err(|error| match error {
            search_source_records::namespace::NamespaceMetadataError::Registry(error) => SourceAdmissionV2Error::Registry(error),
            search_source_records::namespace::NamespaceMetadataError::Admission(error) => SourceAdmissionV2Error::Admission(error),
        })?;
    let mut originals = NamespacePolicyInputs { reference: policy_reference, policy: Arc::clone(&request.policy) };
    let record = batch::DecodedBatch::from_observed_parts_v4(RegistryJournalEntry::new(request.operation_id.clone(),
        fingerprint, registry.revision, after, JournalEntryKind::BatchApply, request.receipt.clone()), fingerprint,
        1, vec![batch::RegistryStateDelta::new(batch::RegistryStateKey::Namespace(namespace_id), None,
            batch::RegistryStateRecord::Namespace(namespace))?], registry.limits, &mut originals, probe)?;
    let prepared = batch::prepare_v4_record(registry, record)?;
    Ok(NativeNamespacePreparationV2::Prepared(PreparedNativeNamespaceV2 { batch: prepared }))
}

/// Commits/reconciles exactly the original preparation without redrawing an ID.
/// Security genesis and Source-body permission are deliberately separate owners.
pub fn commit_native_namespace_v2<C, P>(registry: &mut SourceRegistry, prepared: &PreparedNativeNamespaceV2,
    port: &mut P, context: &OperationContext<C>, probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<CommittedNativeNamespaceV2, SourceAdmissionV2Error<P::Error>>
where C: CancellationProbe, P: RegistryControlPort<Cancellation = C>,
{
    context.preflight().map_err(|_| RegistryError::CancelledBeforeCommit)?;
    checkpoint(probe)?;
    let operation = &prepared.batch.record().entry().operation_id;
    match port.load_batch(operation, context).map_err(|_| RegistryError::MutationOutcomeUnknown)? {
        super::RegistryBatchLookup::Committed(observed) => {
            if observed.record() != prepared.batch.record() { return Err(RegistryError::OperationConflict.into()); }
            observed.verify()?;
            batch::install(registry, observed.record())?;
            registry.identity_cache_v2 = None;
            Ok(creation(observed.record(), true)?)
        }
        super::RegistryBatchLookup::Absent { revision } => {
            if revision != registry.revision || registry.operations.contains_key(operation) {
                return Err(RegistryError::MutationOutcomeUnknown.into());
            }
            if registry.pending_batch.as_ref().is_some_and(|(id, fingerprint)| id == operation
                && *fingerprint == prepared.batch.record().request_fingerprint()) {
                registry.pending_batch = None;
            }
            super::source_v2::commit_prepared_v4(registry, &prepared.batch, port, context, probe)?;
            registry.identity_cache_v2 = None;
            Ok(creation(prepared.batch.record(), false)?)
        }
    }
}

fn creation(record: &batch::DecodedBatch, replayed: bool) -> Result<CommittedNativeNamespaceV2, RegistryError> {
    let [delta] = record.deltas() else { return Err(RegistryError::OperationConflict); };
    let batch::RegistryStateRecord::Namespace(namespace) = delta.after() else { return Err(RegistryError::OperationConflict); };
    if record.encoding_profile() != "eliot.registry-batch.v4" || delta.before().is_some()
        || record.entry().kind != JournalEntryKind::BatchApply
        || delta.key() != &batch::RegistryStateKey::Namespace(namespace.ownership.source_namespace_id)
        || namespace.permanent_metadata().is_none_or(|metadata| metadata.registry_revision() != record.entry().after_revision)
        || namespace.ownership.ownership_record_revision.get() != 1
        || namespace.ownership.status != c::NamespaceOwnershipStatus::Active
        || namespace.fenced || namespace.preparation.is_some() || namespace.activation.is_some() {
        return Err(RegistryError::OperationConflict);
    }
    Ok(CommittedNativeNamespaceV2 { namespace: namespace.clone(), operation_id: record.entry().operation_id.clone(), replayed })
}

fn fingerprint<E>(request: &NativeNamespaceRequestV2, policy: &[u8])
    -> Result<c::Blake3Digest32, SourceAdmissionV2Error<E>> {
    let binary = |bytes: &[u8]| c::BoundedBytes::new(bytes.to_vec()).map(V::Bytes).map_err(|_| RegistryError::ContractExhausted);
    let text = |value: &str| c::CanonicalText::new(value).map(V::Text).map_err(|_| RegistryError::ContractExhausted);
    let value = V::Array(c::BoundedList::new(vec![V::U64(2), binary(request.root_binding_id.as_bytes())?,
        V::U64(request.expected_root_revision.get()), text(request.owner.system_id.as_str())?,
        binary(request.owner.installation_incarnation.as_bytes())?, V::U64(request.owner.epoch.get()),
        V::U64(request.identity_profile.revision().get()), binary(request.identity_profile.schema_digest().as_bytes())?,
        binary(policy)?, text(request.receipt.as_str())?]).map_err(|_| RegistryError::ContractExhausted)?);
    c::blake3_canonical(&c::CanonicalDigestDomain::parse("eliot/cbor/native-namespace-initialization/v2")
        .map_err(|_| RegistryError::ContractExhausted)?, &value,
        c::DigestInputLimit::new(64 * 1024).map_err(|_| RegistryError::ContractExhausted)?)
        .map_err(|_| RegistryError::ContractExhausted.into())
}

// A namespace carries a real retained policy reference but no Source Original.
// Resolve only that exact complete policy, never fabricate admission evidence.
struct NamespacePolicyInputs { reference: Vec<u8>, policy: Arc<RetainedPolicyV2> }
impl search_source_records::batch::OriginalInputResolver for NamespacePolicyInputs {
    fn resolve_policy(&mut self, reference: &[u8], probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>)
        -> Result<Arc<RetainedPolicyV2>, search_source_records::batch::BatchCodecV4Error> {
        let actual = OriginalInputsLayoutV2::policy_reference(&self.policy, probe)?;
        if actual != self.reference || reference != actual {
            return Err(search_source_admission::AdmissionError::ReceiptMismatch.into());
        }
        Ok(Arc::clone(&self.policy))
    }
    fn resolve_admission(&mut self, _: &[u8], _: Arc<RetainedPolicyV2>,
        _: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>)
        -> Result<search_source_provenance::OriginalAdmissionV2, search_source_records::batch::BatchCodecV4Error> {
        Err(search_source_admission::AdmissionError::ReceiptMismatch.into())
    }
}

#[cfg(test)]
#[path = "source_namespace_v2/tests.rs"]
mod tests;
