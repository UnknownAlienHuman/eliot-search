//! Permanent typed SourceMembership preparation over the current source owner.
//!
//! The accepted contract carries its UUID membership ID and policy/profile
//! bindings. This module preserves that value and commits it through the
//! existing C02 registry batch port.

use super::{
    RegistryBatch, RegistryBatchLookup, RegistryChange, RegistryOperation, SourceRegistry,
    apply_admission_batch,
};
use crate::error::{RegistryControlPort, RegistryError, cancelled_before_commit};
use crate::membership::{
    MembershipCommand, MembershipKey, MembershipLifecycle, MembershipRecord,
    PermanentMembershipProvenanceV2,
};
use search_contracts::{
    Blake3Digest32, MembershipRevision, NonZeroRevision, OpaqueId, ReceiptRef, RootBindingId,
    SourceIdentity, SourceMembership, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};
use search_ports::{CancellationProbe, OperationContext};
use search_source_admission::{AdmissionError, AdmissionOutcome, policy_v2::CheckpointFailureV2};
use search_source_records::operation::{BatchItemStatus, RegistryBatchReceipt};
use search_source_records::source_request_v2::{SourceNamespaceSelectionV2, SourceOwnerScopeV2};
use std::collections::BTreeMap;

/// Preparation containing every exact input required for safe commit or replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedSourceMembershipV2 {
    batch: RegistryBatch,
    member: SourceMembership,
    receipt: ReceiptRef,
}

impl PreparedSourceMembershipV2 {
    /// Exact accepted member, including the caller's genuine UUID membership ID.
    pub const fn member(&self) -> &SourceMembership {
        &self.member
    }

    /// Complete immutable request retained for same-operation replay.
    pub const fn batch(&self) -> &RegistryBatch {
        &self.batch
    }
}

/// Content-free acknowledgment of a committed permanent SourceMembership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMembershipReceiptV2 {
    /// Exact accepted typed member used by the operation.
    pub member: SourceMembership,
    /// Registry revision after the original commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether exact original operation replay was acknowledged.
    pub replayed: bool,
}

/// Resolves and validates all current owner fences for one permanent member.
#[allow(clippy::too_many_arguments)]
fn validate_member(
    registry: &SourceRegistry,
    member: &SourceMembership,
    root_binding_id: RootBindingId,
    expected_root_record_revision: NonZeroRevision,
    namespace: SourceNamespaceSelectionV2,
    owner: &SourceOwnerScopeV2,
    expected_source_revision: NonZeroRevision,
) -> Result<(SourceIdentity, PermanentMembershipProvenanceV2), RegistryError> {
    let (namespace_id, namespace_revision, owner_generation) = match namespace {
        SourceNamespaceSelectionV2::Existing {
            id,
            revision,
            generation,
        } => (id, revision, generation),
        SourceNamespaceSelectionV2::CreateNew => {
            return Err(RegistryError::NamespaceOwnershipConflict);
        }
    };
    let mut identities = registry.sources.keys().filter(|identity| {
        identity.source_namespace_id == namespace_id && identity.source_id == member.source_id
    });
    let identity = identities
        .next()
        .cloned()
        .ok_or(RegistryError::SourceNotFound)?;
    if identities.next().is_some() {
        return Err(RegistryError::AdmissionBindingMismatch);
    }
    let source = registry
        .sources
        .get(&identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle() != crate::source::SourceLifecycle::Active
        || source.source_revision() != expected_source_revision
        || source.binding().root_binding_id() != root_binding_id
    {
        return Err(RegistryError::SourceRevisionConflict);
    }
    let admission = source
        .permanent_admission()
        .ok_or(RegistryError::AdmissionReceiptMismatch)?;
    let decision = admission.original().receipt().decision();
    if decision.outcome() != AdmissionOutcome::Allow {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    let root = registry
        .roots
        .get(&root_binding_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    if root.status != crate::root::RootStatus::Active
        || root.record_revision != expected_root_record_revision
        || root.record_revision != admission.root_record_revision()
        || root.policy_revision != decision.policy_revision()
        || root.policy_fingerprint.as_bytes() != decision.policy_fingerprint().as_bytes()
        // Root registration is immutable across Native restarts. The C03
        // owner scope is normalized to the selected namespace's original
        // epoch, so an exact root may predate that namespace but cannot be
        // from a future owner epoch.
        || root.owner_epoch > owner.epoch
    {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    let namespace_state = registry
        .namespaces
        .get(&namespace_id)
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    let metadata = namespace_state
        .permanent_metadata()
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    let ownership = &namespace_state.ownership;
    if ownership.status != search_contracts::NamespaceOwnershipStatus::Active
        || namespace_state.fenced
        || namespace_state.preparation.is_some()
        || namespace_state.activation.is_some()
        || ownership.ownership_record_revision != namespace_revision
        || ownership.ownership_record_revision != admission.namespace_record_revision()
        || ownership.source_owner_generation != owner_generation
        || ownership.source_owner_generation != admission.owner_generation()
        || ownership.owner_system_id != owner.system_id
        || ownership.owner_installation_incarnation_id != owner.installation_incarnation
        || ownership.owner_epoch != owner.epoch
        || ownership.source_admission_policy_revision.get() != decision.policy_revision().get()
        || metadata.root_binding_id() != root_binding_id
        || metadata.policy_reference() != admission.policy_reference()
    {
        return Err(RegistryError::NamespaceOwnershipConflict);
    }
    if member.membership_revision.get() != 1 {
        return Err(RegistryError::MembershipRevisionConflict);
    }
    let provenance = PermanentMembershipProvenanceV2::capture(
        identity.clone(),
        expected_source_revision,
        root_binding_id,
        admission,
    );
    Ok((identity, provenance))
}

fn membership_key(
    member: &SourceMembership,
    source_identity: SourceIdentity,
) -> Result<MembershipKey, RegistryError> {
    let corpus_id =
        OpaqueId::new(member.corpus_id.to_string()).map_err(|_| RegistryError::SnapshotInvalid)?;
    Ok(MembershipKey {
        corpus_id,
        source_identity,
    })
}

/// Prepares a permanent membership from the accepted typed contract.
///
/// No ID is allocated here: `source_membership_id` is the exact UUID assigned
/// by the existing caller ID port and retained in the request. Preparation
/// has no durability effect.
#[allow(clippy::too_many_arguments)]
pub fn prepare_source_membership_v2<C: CancellationProbe>(
    registry: &SourceRegistry,
    member: &SourceMembership,
    root_binding_id: RootBindingId,
    expected_root_record_revision: NonZeroRevision,
    namespace: SourceNamespaceSelectionV2,
    owner: &SourceOwnerScopeV2,
    expected_source_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    context: &OperationContext<C>,
) -> Result<PreparedSourceMembershipV2, RegistryError> {
    cancelled_before_commit(context)?;
    let (source_namespace_id, expected_namespace_record_revision, expected_owner_generation) =
        match namespace {
            SourceNamespaceSelectionV2::Existing {
                id,
                revision,
                generation,
            } => (id, revision, generation),
            SourceNamespaceSelectionV2::CreateNew => {
                return Err(RegistryError::NamespaceOwnershipConflict);
            }
        };
    let (source_identity, _provenance) = validate_member(
        registry,
        member,
        root_binding_id,
        expected_root_record_revision,
        namespace,
        owner,
        expected_source_revision,
    )?;
    let source = registry
        .sources
        .get(&source_identity)
        .ok_or(RegistryError::SourceNotFound)?;
    let admission = source
        .permanent_admission()
        .ok_or(RegistryError::AdmissionReceiptMismatch)?;
    let metadata = registry
        .namespaces
        .get(&source_namespace_id)
        .and_then(|state| state.permanent_metadata())
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    let mut probe = || {
        context
            .preflight()
            .map_err(|_| CheckpointFailureV2::Cancelled)
    };
    admission
        .original()
        .verify_current(metadata.policy().compiled(), &mut probe)
        .map_err(|error| match error {
            AdmissionError::Cancelled => RegistryError::CancelledBeforeCommit,
            AdmissionError::BudgetExhausted => RegistryError::DurabilityExhausted,
            AdmissionError::ReceiptStale => RegistryError::AdmissionReceiptStale,
            AdmissionError::AdmissionDenied
            | AdmissionError::AdmissionReviewRequired
            | AdmissionError::SensitiveSourceDenied
            | AdmissionError::ReceiptMismatch => RegistryError::AdmissionReceiptMismatch,
            _ => RegistryError::AdmissionReceiptStale,
        })?;
    let key = membership_key(member, source_identity)?;
    if registry.memberships.contains_key(&key)
        || registry
            .reverse_memberships
            .contains_key(&member.source_membership_id)
    {
        return Err(RegistryError::MembershipConflict);
    }
    let batch = RegistryBatch {
        expected_registry_revision: registry.revision,
        operation: RegistryOperation::new(operation_id.clone(), mutation_digest),
        changes: vec![RegistryChange::BindPermanentMembershipV2 {
            member: member.clone(),
            source_namespace_id,
            root_binding_id,
            owner: owner.clone(),
            source_identity: key.source_identity,
            expected_source_revision,
            expected_root_record_revision,
            expected_namespace_record_revision,
            expected_owner_generation,
            receipt: receipt.clone(),
        }],
    };
    batch.validate(registry.limits)?;
    super::batch::request_fingerprint(&batch)?;
    Ok(PreparedSourceMembershipV2 {
        batch,
        member: member.clone(),
        receipt: receipt.clone(),
    })
}

/// Prepares a permanent lifecycle change while carrying forward the exact typed member.
///
/// Activation is refused here because restoring effective access requires a
/// fresh root/namespace/policy authorization input, which the lifecycle command
/// does not accept.
pub fn prepare_source_membership_transition_v2<C: CancellationProbe>(
    registry: &SourceRegistry,
    membership_id: SourceMembershipId,
    expected_membership_revision: MembershipRevision,
    command: MembershipCommand,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    context: &OperationContext<C>,
) -> Result<PreparedSourceMembershipV2, RegistryError> {
    cancelled_before_commit(context)?;
    if command == MembershipCommand::Activate {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    let record = registry.permanent_membership(membership_id)?;
    if record.member().membership_revision != expected_membership_revision {
        return Err(RegistryError::MembershipRevisionConflict);
    }
    let mut resulting_member = record.member().clone();
    resulting_member.membership_revision = expected_membership_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let batch = RegistryBatch {
        expected_registry_revision: registry.revision,
        operation: RegistryOperation::new(operation_id.clone(), mutation_digest),
        changes: vec![RegistryChange::TransitionPermanentMembershipV2 {
            membership_id,
            expected_membership_revision,
            command,
            receipt: receipt.clone(),
        }],
    };
    batch.validate(registry.limits)?;
    super::batch::request_fingerprint(&batch)?;
    Ok(PreparedSourceMembershipV2 {
        batch,
        member: resulting_member,
        receipt: receipt.clone(),
    })
}

/// Resolves an exact previously committed public bind before fresh collision checks.
///
/// The complete original V5 request is compared first, then replay is delegated
/// to the same C02 batch owner. Historical replay returns the original receipt
/// and never reinstalls an older lifecycle state.
#[allow(clippy::too_many_arguments)]
pub fn replay_committed_source_membership_bind_v2<C, P>(
    registry: &mut SourceRegistry,
    member: &SourceMembership,
    root_binding_id: RootBindingId,
    expected_root_record_revision: NonZeroRevision,
    namespace: SourceNamespaceSelectionV2,
    owner: &SourceOwnerScopeV2,
    expected_source_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<Option<SourceMembershipReceiptV2>, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    match control_port
        .load_batch(operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    {
        RegistryBatchLookup::Absent { revision } if revision == registry.revision => Ok(None),
        RegistryBatchLookup::Absent { .. } => Err(RegistryError::RegistryRevisionConflict),
        RegistryBatchLookup::Committed(readback) => {
            readback.verify_operation()?;
            let record = readback.record();
            if record.entry().operation_id != *operation_id
                || record.entry().mutation_digest != mutation_digest
            {
                return Err(RegistryError::OperationConflict);
            }
            let batch = record
                .replay_batch()
                .ok_or(RegistryError::OperationConflict)?;
            let [
                RegistryChange::BindPermanentMembershipV2 {
                    member: original_member,
                    source_namespace_id,
                    root_binding_id: original_root_id,
                    owner: original_owner,
                    source_identity,
                    expected_source_revision: original_source_revision,
                    expected_root_record_revision: original_root_revision,
                    expected_namespace_record_revision,
                    expected_owner_generation,
                    receipt: original_receipt,
                },
            ] = batch.changes.as_slice()
            else {
                return Err(RegistryError::OperationConflict);
            };
            let namespace_matches = match &namespace {
                SourceNamespaceSelectionV2::Existing {
                    id,
                    revision,
                    generation,
                } => {
                    id == source_namespace_id
                        && revision == expected_namespace_record_revision
                        && generation == expected_owner_generation
                }
                SourceNamespaceSelectionV2::CreateNew => false,
            };
            if original_member != member
                || *original_root_id != root_binding_id
                || original_owner != owner
                || *original_source_revision != expected_source_revision
                || *original_root_revision != expected_root_record_revision
                || !namespace_matches
                || original_receipt != receipt
                || record.entry().receipt != *receipt
                || source_identity.source_namespace_id != *source_namespace_id
                || source_identity.source_id != member.source_id
                || !registry.sources.contains_key(source_identity)
            {
                return Err(RegistryError::OperationConflict);
            }
            let replay_member = original_member.clone();
            let replay_receipt = original_receipt.clone();
            let prepared = PreparedSourceMembershipV2 {
                batch,
                member: replay_member,
                receipt: replay_receipt,
            };
            let replay = commit_source_membership_v2(registry, &prepared, control_port, context)?;
            if !replay.replayed {
                return Err(RegistryError::MutationOutcomeUnknown);
            }
            Ok(Some(replay))
        }
    }
}

/// Commits the exact prepared request through C02 and returns its original member.
///
/// The underlying batch port resolves committed/absent/unknown outcomes. A
/// caller retry must reuse this preparation, preserving operation ID, member
/// UUID, expected revisions and every full input field.
pub fn commit_source_membership_v2<C, P>(
    registry: &mut SourceRegistry,
    prepared: &PreparedSourceMembershipV2,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceMembershipReceiptV2, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    let result: RegistryBatchReceipt = apply_admission_batch(
        registry,
        prepared.batch.clone(),
        &prepared.receipt,
        control_port,
        context,
    )?;
    let item = result
        .items
        .first()
        .ok_or(RegistryError::MutationOutcomeUnknown)?;
    if item.status != BatchItemStatus::Committed {
        return Err(item.reason.unwrap_or(RegistryError::MutationOutcomeUnknown));
    }
    Ok(SourceMembershipReceiptV2 {
        member: prepared.member.clone(),
        registry_revision: result.after_revision,
        operation_id: result.operation.operation_id().clone(),
        receipt: result.receipt,
        replayed: result.replayed,
    })
}

/// Derives the exact internal state key for one accepted member.
pub(super) fn state_key(
    member: &SourceMembership,
    source_identity: SourceIdentity,
) -> Result<MembershipKey, RegistryError> {
    membership_key(member, source_identity)
}

/// Stages one permanent bind after rechecking exact current owner fences.
#[allow(clippy::too_many_arguments)]
pub(super) fn stage_bind(
    registry: &SourceRegistry,
    member: &SourceMembership,
    source_namespace_id: SourceNamespaceId,
    root_binding_id: RootBindingId,
    owner: &SourceOwnerScopeV2,
    source_identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    expected_root_record_revision: NonZeroRevision,
    expected_namespace_record_revision: NonZeroRevision,
    expected_owner_generation: SourceOwnerGeneration,
    registry_revision: u64,
    receipt: &ReceiptRef,
) -> Result<(MembershipKey, MembershipRecord), RegistryError> {
    let selection = SourceNamespaceSelectionV2::Existing {
        id: source_namespace_id,
        revision: expected_namespace_record_revision,
        generation: expected_owner_generation,
    };
    let (resolved_identity, provenance) = validate_member(
        registry,
        member,
        root_binding_id,
        expected_root_record_revision,
        selection,
        owner,
        expected_source_revision,
    )?;
    if &resolved_identity != source_identity {
        return Err(RegistryError::AdmissionBindingMismatch);
    }
    let key = membership_key(member, resolved_identity)?;
    if key.source_identity.source_namespace_id != source_namespace_id
        || registry.memberships.contains_key(&key)
        || registry
            .reverse_memberships
            .contains_key(&member.source_membership_id)
    {
        return Err(RegistryError::MembershipConflict);
    }
    let generation = NonZeroRevision::new(member.membership_revision.get())
        .map_err(|_| RegistryError::MembershipRevisionConflict)?;
    let record = MembershipRecord::new_permanent_v2(
        key.clone(),
        generation,
        member.clone(),
        provenance,
        MembershipLifecycle::Active,
        registry_revision,
        receipt.clone(),
    )?;
    Ok((key, record))
}

/// Stages one lifecycle transition, preserving the complete typed member.
pub(super) fn stage_transition(
    record: &MembershipRecord,
    expected_membership_revision: MembershipRevision,
    command: MembershipCommand,
    registry_revision: u64,
    receipt: &ReceiptRef,
) -> Result<MembershipRecord, RegistryError> {
    let lifecycle = match (record.lifecycle(), command) {
        (MembershipLifecycle::Active, MembershipCommand::Restrict) => {
            MembershipLifecycle::Restricted
        }
        (
            MembershipLifecycle::Active | MembershipLifecycle::Restricted,
            MembershipCommand::Suspend,
        ) => MembershipLifecycle::Suspended,
        (
            MembershipLifecycle::Active
            | MembershipLifecycle::Restricted
            | MembershipLifecycle::Suspended,
            MembershipCommand::Retire,
        ) => MembershipLifecycle::Retired,
        (MembershipLifecycle::Retired, MembershipCommand::Remove) => MembershipLifecycle::Removed,
        // Reactivation changes effective access and requires fresh owner/policy guards.
        (_, MembershipCommand::Activate) => return Err(RegistryError::AdmissionReceiptStale),
        _ => return Err(RegistryError::MembershipRevisionConflict),
    };
    if record
        .permanent_v2()
        .is_none_or(|v2| v2.member().membership_revision != expected_membership_revision)
    {
        return Err(RegistryError::MembershipRevisionConflict);
    }
    let next = expected_membership_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    record.with_permanent_v2_lifecycle(next, lifecycle, registry_revision, receipt.clone())
}

/// Resolves one existing permanent membership ID to its current internal key.
pub(super) fn key_for_id(
    reverse: &BTreeMap<SourceMembershipId, MembershipKey>,
    membership_id: SourceMembershipId,
) -> Result<MembershipKey, RegistryError> {
    reverse
        .get(&membership_id)
        .cloned()
        .ok_or(RegistryError::MembershipNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::NamespaceCutoverState;
    use crate::error::{
        DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, RegistryControlPort, RegistryPortError,
        registry_port_error,
    };
    use crate::root::{RegisterRootRequest, RootRecord, RootStatus};
    use crate::source::{RegisteredSource, SourceLifecycle};
    use search_contracts::{
        AccessPolicyBindingId, CorpusId, InstallationIncarnationId, MembershipRole,
        OpaqueCanonicalBytes, OpaqueRef, OwnerEpoch, PolicyRevision, ProfileId,
        ResidencyPolicyBindingId, SourceId, SourceIdentityKind, SourceMembershipId,
        SourceNamespaceOwnership, WorkspaceId,
    };
    use search_ports::{
        FakeCancellation, MutationIdentity, OperationContext, Port, PortErrorKind, PortReceipt,
        PortRetryability,
    };
    use search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES;
    use search_source_admission::original_inputs_v2::reconstitute_observation_v2;
    use search_source_admission::policy_v2::{
        DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2, RuleInputV2,
    };
    use search_source_admission::{
        AcceptedCapabilities, BASELINE_PROFILE, RuleEffect, SectionPlatform, SourceClass,
        compiled_defaults, validate_section,
    };
    use search_source_identity::{
        CanonicalRelativePath, CaseBehavior, DEFAULT_IDENTITY_LIMITS, FilesystemIdentityProfile,
        LinkBehavior, ReparseBehavior, SourceBinding, SourceObservation, StableFieldPolicy,
        UnicodeBehavior,
    };
    use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2};
    use std::sync::Arc;

    struct Fixture {
        registry: SourceRegistry,
        member: SourceMembership,
        root_id: RootBindingId,
        namespace: SourceNamespaceId,
        owner: SourceOwnerScopeV2,
        namespace_selection: SourceNamespaceSelectionV2,
        source_revision: NonZeroRevision,
        port: crate::error::InMemoryRegistryJournal<FakeCancellation>,
        context: OperationContext<FakeCancellation>,
    }

    struct LoseOneAcknowledgment {
        inner: InMemoryRegistryJournal<FakeCancellation>,
        lose_next_commit_ack: bool,
    }

    impl Port for LoseOneAcknowledgment {
        type Error = RegistryPortError;
        type Cancellation = FakeCancellation;
    }

    impl RegistryControlPort for LoseOneAcknowledgment {
        fn commit_batch(
            &mut self,
            batch: &super::super::PreparedBatch,
            context: &OperationContext<Self::Cancellation>,
            mutation: &MutationIdentity,
        ) -> Result<super::super::RegistryBatchReadback, Self::Error> {
            let readback = self.inner.commit_batch(batch, context, mutation)?;
            if self.lose_next_commit_ack {
                self.lose_next_commit_ack = false;
                return Err(registry_port_error(
                    PortErrorKind::OutcomeUnknown,
                    PortRetryability::AfterReadback,
                    RegistryError::MutationOutcomeUnknown,
                    Some(mutation.operation_id.clone()),
                ));
            }
            Ok(readback)
        }

        fn load_batch(
            &self,
            operation_id: &OpaqueId,
            context: &OperationContext<Self::Cancellation>,
        ) -> Result<super::super::RegistryBatchLookup, Self::Error> {
            self.inner.load_batch(operation_id, context)
        }

        fn persist_entry(
            &mut self,
            entry: &crate::error::RegistryJournalEntry,
            context: &OperationContext<Self::Cancellation>,
            mutation: &MutationIdentity,
        ) -> Result<PortReceipt, Self::Error> {
            self.inner.persist_entry(entry, context, mutation)
        }

        fn load_entry(
            &self,
            operation_id: &OpaqueId,
            context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<crate::error::RegistryJournalEntry>, Self::Error> {
            self.inner.load_entry(operation_id, context)
        }
    }

    fn rev(value: u64) -> NonZeroRevision {
        NonZeroRevision::new(value).expect("revision")
    }

    fn opaque(value: &str) -> OpaqueId {
        OpaqueId::new(value).expect("opaque")
    }

    fn receipt(value: &str) -> ReceiptRef {
        ReceiptRef::new(value).expect("receipt")
    }

    fn digest(value: u8) -> Blake3Digest32 {
        Blake3Digest32::from_bytes([value; 32])
    }

    fn context() -> OperationContext<FakeCancellation> {
        OperationContext::new(
            search_contracts::RequestId::from_bytes([40; 16]),
            1_000,
            FakeCancellation::new(false),
            OpaqueRef::new("budget:membership-v2-test").expect("budget ref"),
        )
        .expect("context")
    }

    fn original(effect: RuleEffect) -> OriginalAdmissionV2 {
        let platform = SectionPlatform {
            os: "windows".into(),
            arch: "x64".into(),
        };
        let capabilities = AcceptedCapabilities {
            profiles: vec![BASELINE_PROFILE.into()],
        };
        let section =
            validate_section(&compiled_defaults(), &platform, &capabilities).expect("section");
        let compiled = search_source_admission::policy_v2::compile_policy_v2(
            &PolicyInputV2 {
                schema_version: POLICY_SCHEMA_VERSION,
                revision: 7,
                section,
                rules: vec![RuleInputV2 {
                    id: "membership-v2-test".into(),
                    precedence: 0,
                    source_class: SourceClass::Regular,
                    predicate: PredicateV2::Always,
                    effect,
                    unknown_fields: Vec::new(),
                }],
                unknown_fields: Vec::new(),
            },
            DEFAULT_COMPILE_LIMITS,
            &mut || Ok(()),
        )
        .expect("compiled policy");
        let policy = Arc::new(
            RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(()))
                .expect("retained policy"),
        );
        let observation = reconstitute_observation_v2(
            &search_source_admission::UnvalidatedObservationInput {
                locator_class: "normalized-file".into(),
                location_class: "local-fixed".into(),
                source_kind: "file".into(),
                source_class: "regular".into(),
                byte_size: Some(10),
                is_generated: Some(false),
                is_vendor: Some(false),
                is_binary: Some(false),
                is_system: Some(false),
                sensitivity: Some("public".into()),
                detector_id: Some("detector:membership-v2-test".into()),
                profile_id: Some(BASELINE_PROFILE.into()),
                unavailable_fields: Vec::new(),
                unknown_fields: Vec::new(),
            },
            Some(b"src/lib.rs"),
            policy.compiled(),
            MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(()),
        )
        .expect("normalized observation");
        OriginalAdmissionV2::retain(policy, &observation, &mut || Ok(())).expect("original")
    }

    fn make_fixture(effect: RuleEffect) -> Fixture {
        make_fixture_with_epochs(
            effect,
            OwnerEpoch::new(3).expect("root owner epoch"),
            OwnerEpoch::new(3).expect("namespace owner epoch"),
        )
    }

    fn make_fixture_with_epochs(
        effect: RuleEffect,
        root_epoch: OwnerEpoch,
        namespace_epoch: OwnerEpoch,
    ) -> Fixture {
        let original = original(effect);
        let decision = original.receipt().decision();
        let root_id = RootBindingId::from_bytes([41; 16]);
        let namespace = SourceNamespaceId::from_bytes([42; 16]);
        let source_id = SourceId::from_bytes([43; 16]);
        let source_identity = SourceIdentity {
            source_namespace_id: namespace,
            source_id,
            identity_kind: SourceIdentityKind::NtfsFile,
            stable_identity_components: OpaqueCanonicalBytes::from_validated(
                b"stable:membership-v2-source".to_vec(),
            )
            .expect("identity components"),
        };
        let owner = SourceOwnerScopeV2 {
            system_id: opaque("system:membership-v2"),
            installation_incarnation: InstallationIncarnationId::from_bytes([44; 16]),
            epoch: namespace_epoch,
        };
        let mut ownership = SourceNamespaceOwnership {
            source_namespace_id: namespace,
            owner_system_id: owner.system_id.clone(),
            owner_installation_incarnation_id: owner.installation_incarnation,
            owner_epoch: owner.epoch,
            ownership_record_revision: rev(1),
            source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
            source_admission_policy_revision: PolicyRevision::new(7),
            status: search_contracts::NamespaceOwnershipStatus::Active,
            cutover_receipt_ref: None,
        };
        ownership.source_owner_generation =
            search_domain::source_ownership_v2::source_owner_generation_v2(&ownership)
                .expect("owner generation");
        let generation = ownership.source_owner_generation;
        let policy_reference =
            OriginalInputsLayoutV2::policy_reference(original.policy(), &mut || Ok(()))
                .expect("policy reference");
        let profile = FilesystemIdentityProfile::new(
            rev(1),
            digest(45),
            CaseBehavior::InsensitiveAscii,
            UnicodeBehavior::PreNormalizedNfc,
            StableFieldPolicy::Required,
            StableFieldPolicy::Required,
            LinkBehavior::StablePhysicalIdentity,
            ReparseBehavior::FinalTargetIdentity,
        )
        .expect("identity profile");
        let namespace_state = NamespaceCutoverState::new_v2_metadata(
            ownership,
            root_id,
            profile,
            Arc::clone(original.policy()),
            policy_reference,
            1,
            receipt("receipt:namespace-v2"),
            &mut || Ok(()),
        )
        .expect("permanent namespace");
        let root = RootRecord {
            root_binding_id: root_id,
            canonical_root_digest: digest(46),
            policy_fingerprint: Blake3Digest32::from_stored_bytes(
                *decision.policy_fingerprint().as_bytes(),
            ),
            policy_revision: decision.policy_revision(),
            owner_epoch: root_epoch,
            record_revision: rev(1),
            registry_revision: 1,
            status: RootStatus::Active,
            last_receipt: receipt("receipt:root-v2"),
        };
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        let mut port = InMemoryRegistryJournal::new(100).expect("journal");
        let ctx = context();
        let root_request = RegisterRootRequest {
            root_binding_id: root_id,
            canonical_root_digest: root.canonical_root_digest,
            policy_fingerprint: root.policy_fingerprint,
            policy_revision: root.policy_revision,
            owner_epoch: root_epoch,
            owner_fence_receipt: receipt("receipt:owner-fence-v2"),
            policy_receipt: receipt("receipt:policy-v2"),
        };
        crate::api::register_root(
            &mut registry,
            &root_request,
            &opaque("operation:root-v2"),
            digest(47),
            &receipt("receipt:root-register-v2"),
            &mut port,
            &ctx,
        )
        .expect("persist root");
        let binding = SourceBinding::new(
            source_identity.clone(),
            SourceObservation {
                root_binding_id: root_id,
                relative_path: CanonicalRelativePath::new("src/lib.rs", DEFAULT_IDENTITY_LIMITS)
                    .expect("relative path"),
                stable_file_identity_digest: Some(digest(48)),
                content_digest: digest(49),
                content_bytes: 10,
                observation_receipt: receipt("receipt:observation-v2"),
            },
            rev(1),
            rev(1),
        );
        let permanent = search_source_records::provenance::PermanentSourceAdmissionV2::new(
            original,
            rev(1),
            rev(1),
            generation,
            &mut || Ok(()),
        )
        .expect("permanent source evidence");
        let source = RegisteredSource::new_permanent(
            binding,
            permanent,
            SourceLifecycle::Active,
            rev(1),
            1,
            receipt("receipt:source-v2"),
        )
        .expect("permanent source");
        registry.sources.insert(source_identity, source);
        registry.namespaces.insert(namespace, namespace_state);
        let member = SourceMembership {
            source_membership_id: SourceMembershipId::from_bytes([50; 16]),
            corpus_id: CorpusId::from_bytes([51; 16]),
            source_id,
            workspace_id: WorkspaceId::from_bytes([52; 16]),
            role: MembershipRole::Documentation,
            preparation_profile_id: ProfileId::new("profile:prepared-text-v2")
                .expect("preparation profile"),
            access_policy_binding_id: AccessPolicyBindingId::from_bytes([53; 16]),
            retention_policy_id: ProfileId::new("profile:retention-v2").expect("retention"),
            residency_policy_binding_id: ResidencyPolicyBindingId::from_bytes([54; 16]),
            membership_revision: MembershipRevision::new(1),
        };
        Fixture {
            registry,
            member,
            root_id,
            namespace,
            owner,
            namespace_selection: SourceNamespaceSelectionV2::Existing {
                id: namespace,
                revision: rev(1),
                generation,
            },
            source_revision: rev(1),
            port,
            context: ctx,
        }
    }

    fn prepare_result(
        fixture: &Fixture,
        operation_name: &str,
        member: &SourceMembership,
    ) -> Result<PreparedSourceMembershipV2, RegistryError> {
        prepare_source_membership_v2(
            &fixture.registry,
            member,
            fixture.root_id,
            rev(1),
            fixture.namespace_selection,
            &fixture.owner,
            fixture.source_revision,
            &opaque(operation_name),
            digest(55),
            &receipt("receipt:membership-v2"),
            &fixture.context,
        )
    }

    fn prepare(
        fixture: &Fixture,
        operation_name: &str,
        member: &SourceMembership,
    ) -> PreparedSourceMembershipV2 {
        prepare_result(fixture, operation_name, member).expect("prepare member")
    }

    #[test]
    fn permanent_source_bind_replay_lookup_and_conflicting_payload_are_exact() {
        let mut fixture = make_fixture(RuleEffect::Allow);
        let source = fixture
            .registry
            .sources
            .values()
            .next()
            .expect("permanent source");
        assert!(source.legacy_admission().is_none());
        assert_eq!(
            source.admission_outcome(),
            AdmissionOutcome::Allow,
            "the bind uses the real permanent admission path"
        );
        let original = prepare(&fixture, "operation:membership-v2", &fixture.member);
        let committed = commit_source_membership_v2(
            &mut fixture.registry,
            &original,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("commit membership");
        assert!(!committed.replayed);
        assert_eq!(committed.member, fixture.member);
        assert_eq!(
            fixture
                .registry
                .permanent_membership(fixture.member.source_membership_id)
                .expect("record")
                .member(),
            &fixture.member
        );
        let (member, identity) = fixture
            .registry
            .active_source_membership_v2(fixture.member.source_membership_id)
            .expect("retained reader lookup");
        assert_eq!(member, &fixture.member);
        assert_eq!(identity.source_namespace_id, fixture.namespace);
        assert_eq!(identity.source_id, fixture.member.source_id);
        let snapshot = crate::api::registry_snapshot(&fixture.registry);
        let validated =
            crate::snapshot::validate_registry_snapshot(&snapshot, DEFAULT_REGISTRY_LIMITS)
                .expect("typed membership snapshot");
        assert_eq!(
            crate::snapshot::snapshot_digest_v5(&snapshot),
            Err(RegistryError::SnapshotInvalid),
            "the old snapshot profile cannot omit typed member fields"
        );
        assert_eq!(
            crate::snapshot::snapshot_digest_v6(validated.snapshot()),
            crate::snapshot::snapshot_digest_v6(&snapshot)
        );
        assert_ne!(
            fixture
                .registry
                .permanent_membership(fixture.member.source_membership_id)
                .expect("record")
                .member()
                .source_membership_id,
            crate::membership::derive_membership_id(
                fixture
                    .registry
                    .membership_key_by_id(fixture.member.source_membership_id)
                    .expect("typed member key")
            ),
            "the new contract UUID is not replaced by the frozen legacy FNV ID"
        );

        let replay = commit_source_membership_v2(
            &mut fixture.registry,
            &original,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("exact operation replay");
        assert!(replay.replayed);
        assert_eq!(replay.member, fixture.member);
        assert_eq!(replay.operation_id, committed.operation_id);
        assert_eq!(replay.receipt, committed.receipt);

        let mut conflicting_member = fixture.member.clone();
        conflicting_member.access_policy_binding_id = AccessPolicyBindingId::from_bytes([99; 16]);
        let conflicting = prepare(
            &make_fixture(RuleEffect::Allow),
            "operation:membership-v2",
            &conflicting_member,
        );
        assert_eq!(
            commit_source_membership_v2(
                &mut fixture.registry,
                &conflicting,
                &mut fixture.port,
                &fixture.context,
            ),
            Err(RegistryError::OperationConflict)
        );
    }

    #[test]
    fn historical_root_epoch_accepts_later_namespace_and_rejects_future_or_foreign_owner() {
        let mut historical_root = make_fixture_with_epochs(
            RuleEffect::Allow,
            OwnerEpoch::new(1).expect("historical root epoch"),
            OwnerEpoch::new(4).expect("namespace epoch"),
        );
        // The native caller has already proved epoch 5 and preserves the
        // existing namespace's epoch 4 in the C03 SourceOwnerScopeV2.
        assert!(
            historical_root.owner.epoch < OwnerEpoch::new(5).expect("current native owner epoch")
        );
        let prepared = prepare(
            &historical_root,
            "operation:historical-root-epoch",
            &historical_root.member,
        );
        commit_source_membership_v2(
            &mut historical_root.registry,
            &prepared,
            &mut historical_root.port,
            &historical_root.context,
        )
        .expect("root from an earlier Native epoch remains valid");
        let (member, identity) = historical_root
            .registry
            .active_source_membership_v2(historical_root.member.source_membership_id)
            .expect("retained lookup accepts the historical root");
        assert_eq!(member, &historical_root.member);
        assert_eq!(identity.source_namespace_id, historical_root.namespace);
        assert_eq!(
            historical_root
                .registry
                .root(historical_root.root_id)
                .expect("root")
                .owner_epoch,
            OwnerEpoch::new(1).expect("historical root epoch")
        );
        assert_eq!(
            historical_root
                .registry
                .namespaces()
                .get(&historical_root.namespace)
                .expect("namespace")
                .ownership
                .owner_epoch,
            OwnerEpoch::new(4).expect("namespace epoch")
        );

        let future_root = make_fixture_with_epochs(
            RuleEffect::Allow,
            OwnerEpoch::new(5).expect("future root epoch"),
            OwnerEpoch::new(4).expect("namespace epoch"),
        );
        assert_eq!(
            prepare_result(
                &future_root,
                "operation:future-root-epoch",
                &future_root.member
            )
            .unwrap_err(),
            RegistryError::AdmissionReceiptStale
        );

        let future_namespace_scope = make_fixture_with_epochs(
            RuleEffect::Allow,
            OwnerEpoch::new(1).expect("historical root epoch"),
            OwnerEpoch::new(4).expect("namespace epoch"),
        );
        let future_scope = SourceOwnerScopeV2 {
            epoch: OwnerEpoch::new(5).expect("future namespace scope epoch"),
            ..future_namespace_scope.owner.clone()
        };
        assert_eq!(
            prepare_source_membership_v2(
                &future_namespace_scope.registry,
                &future_namespace_scope.member,
                future_namespace_scope.root_id,
                rev(1),
                future_namespace_scope.namespace_selection,
                &future_scope,
                future_namespace_scope.source_revision,
                &opaque("operation:future-namespace-scope"),
                digest(55),
                &receipt("receipt:future-namespace-scope"),
                &future_namespace_scope.context,
            )
            .unwrap_err(),
            RegistryError::NamespaceOwnershipConflict
        );

        let foreign_owner_fixture = make_fixture_with_epochs(
            RuleEffect::Allow,
            OwnerEpoch::new(1).expect("historical root epoch"),
            OwnerEpoch::new(4).expect("namespace epoch"),
        );
        let foreign_owner = SourceOwnerScopeV2 {
            system_id: opaque("system:foreign-membership-v2"),
            installation_incarnation: InstallationIncarnationId::from_bytes([99; 16]),
            epoch: foreign_owner_fixture.owner.epoch,
        };
        assert_eq!(
            prepare_source_membership_v2(
                &foreign_owner_fixture.registry,
                &foreign_owner_fixture.member,
                foreign_owner_fixture.root_id,
                rev(1),
                foreign_owner_fixture.namespace_selection,
                &foreign_owner,
                foreign_owner_fixture.source_revision,
                &opaque("operation:foreign-owner-membership"),
                digest(55),
                &receipt("receipt:foreign-owner-membership"),
                &foreign_owner_fixture.context,
            )
            .unwrap_err(),
            RegistryError::NamespaceOwnershipConflict
        );
    }

    #[test]
    fn public_bind_replays_c02_after_restart_without_reactivating_membership() {
        let mut fixture = make_fixture(RuleEffect::Allow);
        let operation_id = opaque("operation:public-membership-bind");
        let bind_receipt = receipt("receipt:public-membership-bind");
        let first = crate::api::bind_source_membership_v2(
            &mut fixture.registry,
            &fixture.member,
            fixture.root_id,
            rev(1),
            fixture.namespace_selection,
            &fixture.owner,
            fixture.source_revision,
            &operation_id,
            digest(55),
            &bind_receipt,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("public bind");
        assert!(!first.replayed);

        // Reopen the retained owner state without its process-local operation
        // cache; exact replay must be established through the C02 port.
        let mut restarted = fixture.registry.clone();
        restarted.operations.clear();
        let repeated = crate::api::bind_source_membership_v2(
            &mut restarted,
            &fixture.member,
            fixture.root_id,
            rev(1),
            fixture.namespace_selection,
            &fixture.owner,
            fixture.source_revision,
            &operation_id,
            digest(55),
            &bind_receipt,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("public bind replay after restart");
        assert!(repeated.replayed);
        assert_eq!(repeated.member, first.member);
        assert_eq!(repeated.registry_revision, first.registry_revision);
        assert_eq!(repeated.operation_id, first.operation_id);
        assert_eq!(repeated.receipt, first.receipt);

        let restricted = crate::api::transition_source_membership_v2(
            &mut restarted,
            fixture.member.source_membership_id,
            MembershipRevision::new(1),
            MembershipCommand::Restrict,
            &opaque("operation:public-membership-restrict"),
            digest(56),
            &receipt("receipt:public-membership-restrict"),
            &mut fixture.port,
            &fixture.context,
        )
        .expect("restrict membership");
        assert_eq!(
            restricted.member.membership_revision,
            MembershipRevision::new(2)
        );
        assert_eq!(
            restarted
                .permanent_membership(fixture.member.source_membership_id)
                .expect("restricted member")
                .lifecycle(),
            MembershipLifecycle::Restricted
        );

        let mut restarted_after_restrict = restarted.clone();
        restarted_after_restrict.operations.clear();
        let before_replay = crate::api::registry_snapshot(&restarted_after_restrict);
        let repeated_after_restrict = crate::api::bind_source_membership_v2(
            &mut restarted_after_restrict,
            &fixture.member,
            fixture.root_id,
            rev(1),
            fixture.namespace_selection,
            &fixture.owner,
            fixture.source_revision,
            &operation_id,
            digest(55),
            &bind_receipt,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("historical bind replay after lifecycle transition");
        assert!(repeated_after_restrict.replayed);
        assert_eq!(repeated_after_restrict.member, first.member);
        assert_eq!(
            crate::api::registry_snapshot(&restarted_after_restrict),
            before_replay,
            "replay does not restore the original active state"
        );
        let retained = restarted_after_restrict
            .permanent_membership(fixture.member.source_membership_id)
            .expect("retained restricted member");
        assert_eq!(retained.lifecycle(), MembershipLifecycle::Restricted);
        assert_eq!(
            retained.member().membership_revision,
            MembershipRevision::new(2)
        );

        let mut changed_intent = fixture.member.clone();
        changed_intent.access_policy_binding_id = AccessPolicyBindingId::from_bytes([99; 16]);
        assert_eq!(
            crate::api::bind_source_membership_v2(
                &mut restarted_after_restrict,
                &changed_intent,
                fixture.root_id,
                rev(1),
                fixture.namespace_selection,
                &fixture.owner,
                fixture.source_revision,
                &operation_id,
                digest(55),
                &bind_receipt,
                &mut fixture.port,
                &fixture.context,
            ),
            Err(RegistryError::OperationConflict)
        );
        assert_eq!(
            crate::api::registry_snapshot(&restarted_after_restrict),
            before_replay
        );
    }

    #[test]
    fn unknown_commit_recovers_through_the_existing_batch_readback() {
        let mut fixture = make_fixture(RuleEffect::Allow);
        let prepared = prepare(&fixture, "operation:membership-unknown", &fixture.member);
        let mut port = LoseOneAcknowledgment {
            inner: fixture.port.clone(),
            lose_next_commit_ack: true,
        };
        assert_eq!(
            commit_source_membership_v2(
                &mut fixture.registry,
                &prepared,
                &mut port,
                &fixture.context,
            ),
            Err(RegistryError::MutationOutcomeUnknown)
        );
        assert_eq!(fixture.registry.revision, 1);
        match port
            .load_batch(prepared.batch.operation.operation_id(), &fixture.context)
            .expect("C02 operation readback")
        {
            super::super::RegistryBatchLookup::Committed(readback) => {
                assert_eq!(
                    readback.record().replay_batch(),
                    Some(prepared.batch().clone()),
                    "V5 retains the original member UUID and every expected owner fence"
                );
            }
            other => panic!("expected committed V5 operation, got {other:?}"),
        }
        let recovered = commit_source_membership_v2(
            &mut fixture.registry,
            &prepared,
            &mut port,
            &fixture.context,
        )
        .expect("exact C02 readback recovers the operation");
        assert!(recovered.replayed);
        assert_eq!(recovered.member, fixture.member);
        assert_eq!(fixture.registry.revision, 2);
        assert_eq!(
            fixture
                .registry
                .permanent_membership(fixture.member.source_membership_id)
                .expect("restored member")
                .member(),
            &fixture.member
        );
    }

    #[test]
    fn deny_review_stale_owner_source_policy_and_collision_refuse_before_commit() {
        for effect in [RuleEffect::Deny, RuleEffect::Review] {
            let fixture = make_fixture(effect);
            assert_eq!(
                prepare_source_membership_v2(
                    &fixture.registry,
                    &fixture.member,
                    fixture.root_id,
                    rev(1),
                    fixture.namespace_selection,
                    &fixture.owner,
                    fixture.source_revision,
                    &opaque("operation:denied-member"),
                    digest(56),
                    &receipt("receipt:denied-member"),
                    &fixture.context,
                ),
                Err(RegistryError::AdmissionReceiptMismatch)
            );
        }

        let mut stale_root = make_fixture(RuleEffect::Allow);
        stale_root
            .registry
            .roots
            .get_mut(&stale_root.root_id)
            .expect("root")
            .record_revision = rev(2);
        assert_eq!(
            prepare_result(&stale_root, "operation:stale-root", &stale_root.member).unwrap_err(),
            RegistryError::AdmissionReceiptStale
        );

        let mut stale_policy = make_fixture(RuleEffect::Allow);
        stale_policy
            .registry
            .roots
            .get_mut(&stale_policy.root_id)
            .expect("root")
            .policy_fingerprint = digest(91);
        assert_eq!(
            prepare_result(
                &stale_policy,
                "operation:stale-policy",
                &stale_policy.member,
            )
            .unwrap_err(),
            RegistryError::AdmissionReceiptStale
        );

        let mut stale_namespace = make_fixture(RuleEffect::Allow);
        stale_namespace
            .registry
            .namespaces
            .get_mut(&stale_namespace.namespace)
            .expect("namespace")
            .ownership
            .ownership_record_revision = rev(2);
        assert_eq!(
            prepare_result(
                &stale_namespace,
                "operation:stale-namespace",
                &stale_namespace.member,
            )
            .unwrap_err(),
            RegistryError::NamespaceOwnershipConflict
        );

        let stale_source = make_fixture(RuleEffect::Allow);
        assert_eq!(
            prepare_source_membership_v2(
                &stale_source.registry,
                &stale_source.member,
                stale_source.root_id,
                rev(1),
                stale_source.namespace_selection,
                &stale_source.owner,
                rev(2),
                &opaque("operation:stale-source"),
                digest(57),
                &receipt("receipt:stale-source"),
                &stale_source.context,
            )
            .unwrap_err(),
            RegistryError::SourceRevisionConflict
        );

        let mut collision = make_fixture(RuleEffect::Allow);
        collision.registry.reverse_memberships.insert(
            collision.member.source_membership_id,
            MembershipKey {
                corpus_id: opaque("foreign-corpus"),
                source_identity: SourceIdentity {
                    source_namespace_id: collision.namespace,
                    source_id: SourceId::from_bytes([97; 16]),
                    identity_kind: SourceIdentityKind::NtfsFile,
                    stable_identity_components: OpaqueCanonicalBytes::from_validated(
                        b"foreign".to_vec(),
                    )
                    .expect("components"),
                },
            },
        );
        assert_eq!(
            prepare_result(&collision, "operation:member-collision", &collision.member)
                .unwrap_err(),
            RegistryError::MembershipConflict
        );
    }

    #[test]
    fn lifecycle_preserves_typed_policy_and_old_transition_refuses_it() {
        let mut fixture = make_fixture(RuleEffect::Allow);
        let prepared = prepare(&fixture, "operation:membership-create", &fixture.member);
        commit_source_membership_v2(
            &mut fixture.registry,
            &prepared,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("bind");
        let transitioned = crate::api::transition_source_membership_v2(
            &mut fixture.registry,
            fixture.member.source_membership_id,
            fixture.member.membership_revision,
            MembershipCommand::Restrict,
            &opaque("operation:membership-restrict"),
            digest(58),
            &receipt("receipt:membership-restrict"),
            &mut fixture.port,
            &fixture.context,
        )
        .expect("restrict");
        let mut expected_member = fixture.member.clone();
        expected_member.membership_revision = MembershipRevision::new(2);
        assert_eq!(transitioned.member, expected_member);
        let record = fixture
            .registry
            .permanent_membership(fixture.member.source_membership_id)
            .expect("retained record");
        assert_eq!(record.member(), &expected_member);
        assert_eq!(
            record.member().access_policy_binding_id,
            fixture.member.access_policy_binding_id
        );
        assert_eq!(
            record.member().residency_policy_binding_id,
            fixture.member.residency_policy_binding_id
        );
        assert_eq!(
            record.member().retention_policy_id,
            fixture.member.retention_policy_id
        );
        assert_eq!(record.lifecycle(), MembershipLifecycle::Restricted);
        assert_eq!(
            fixture
                .registry
                .active_source_membership_v2(fixture.member.source_membership_id),
            Err(RegistryError::MembershipNotFound)
        );
        assert_eq!(
            crate::api::prepare_source_membership_transition_v2(
                &fixture.registry,
                fixture.member.source_membership_id,
                MembershipRevision::new(2),
                MembershipCommand::Activate,
                &opaque("operation:membership-reactivate"),
                digest(59),
                &receipt("receipt:membership-reactivate"),
                &fixture.context,
            ),
            Err(RegistryError::AdmissionReceiptStale)
        );
    }

    #[test]
    fn retained_lookup_refuses_legacy_membership() {
        let mut fixture = make_fixture(RuleEffect::Allow);
        let key = MembershipKey {
            corpus_id: opaque("legacy-corpus-for-lookup"),
            source_identity: fixture
                .registry
                .sources
                .keys()
                .next()
                .expect("source")
                .clone(),
        };
        let membership_id = crate::membership::derive_membership_id(&key);
        let record = MembershipRecord::new(
            key.clone(),
            membership_id,
            rev(1),
            rev(1),
            MembershipLifecycle::Active,
            fixture.registry.revision,
            receipt("receipt:legacy-membership"),
        );
        fixture.registry.memberships.insert(key.clone(), record);
        fixture
            .registry
            .reverse_memberships
            .insert(membership_id, key);
        assert_eq!(
            fixture.registry.active_source_membership_v2(membership_id),
            Err(RegistryError::AdmissionReceiptMismatch)
        );
    }

    #[test]
    fn c03_v7_partition_history_restores_and_exact_replay_keeps_class_ids() {
        use crate::projection_membership::ProjectionPartitionSemantics;
        use crate::recovery::{
            RegistryChange, RegistryOperation, RegistryRestoreOperation, RegistryRestoreState,
            MAX_REGISTRY_RESTORE_BYTES,
        };
        use search_contracts as c;
        use std::collections::BTreeMap;

        fn canonical(value: &str) -> Vec<u8> {
            c::to_canonical_cbor(&c::CanonicalValue::Text(
                c::CanonicalText::new(value).expect("text"),
            ))
            .expect("canonical bytes")
            .into_vec()
        }

        fn semantics() -> ProjectionPartitionSemantics {
            ProjectionPartitionSemantics::new(
                canonical("access:shared-read-v1"),
                canonical("scoring:raw-code-v1"),
            )
            .expect("full class meanings")
        }

        fn request(
            registry: &SourceRegistry,
            member: &SourceMembership,
            projection: u8,
            representation: u8,
            access_partition: u8,
            scoring_partition: u8,
            semantics: Option<ProjectionPartitionSemantics>,
        ) -> crate::projection_membership::ProjectionMembershipBindRequest {
            crate::projection_membership::ProjectionMembershipBindRequest {
                binding: c::ProjectionMembership {
                    projection_membership_id: c::ProjectionMembershipId::from_bytes([projection; 16]),
                    source_membership_id: member.source_membership_id,
                    representation_id: c::RepresentationId::from_bytes([representation; 16]),
                    access_partition_id: c::AccessPartitionId::from_bytes([access_partition; 16]),
                    scoring_partition_id: c::ScoringPartitionId::from_bytes([scoring_partition; 16]),
                    projection_schema_id: ProfileId::new("profile:projection-v7-test")
                        .expect("projection schema"),
                },
                expected_membership_revision: member.membership_revision,
                expected_provenance: registry
                    .permanent_membership(member.source_membership_id)
                    .expect("permanent member")
                    .provenance()
                    .clone(),
                partition_semantics: semantics,
            }
        }

        fn persist_member(
            fixture: &Fixture,
            registry: &mut SourceRegistry,
            port: &mut InMemoryRegistryJournal<FakeCancellation>,
            suffix: u8,
        ) -> SourceMembership {
            let mut member = fixture.member.clone();
            member.source_membership_id = SourceMembershipId::from_bytes([suffix; 16]);
            member.corpus_id = CorpusId::from_bytes([suffix.wrapping_add(1); 16]);
            member.workspace_id = WorkspaceId::from_bytes([suffix.wrapping_add(2); 16]);
            let prepared = prepare_source_membership_v2(
                registry,
                &member,
                fixture.root_id,
                rev(1),
                fixture.namespace_selection,
                &fixture.owner,
                fixture.source_revision,
                &opaque(&format!("operation:partition-class-member:{suffix}")),
                digest(suffix),
                &receipt(&format!("receipt:partition-class-member:{suffix}")),
                &fixture.context,
            )
            .expect("prepare class consistency member");
            commit_source_membership_v2(registry, &prepared, port, &fixture.context)
                .expect("persist class consistency member");
            member
        }

        let mut fixture = make_fixture(RuleEffect::Allow);
        let first_member = prepare(&fixture, "operation:membership-v7-first", &fixture.member);
        commit_source_membership_v2(
            &mut fixture.registry,
            &first_member,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("persist first permanent membership");

        let mut second_member = fixture.member.clone();
        second_member.source_membership_id = SourceMembershipId::from_bytes([60; 16]);
        second_member.corpus_id = CorpusId::from_bytes([61; 16]);
        second_member.workspace_id = WorkspaceId::from_bytes([62; 16]);
        let second_prepared = prepare_source_membership_v2(
            &fixture.registry,
            &second_member,
            fixture.root_id,
            rev(1),
            fixture.namespace_selection,
            &fixture.owner,
            fixture.source_revision,
            &opaque("operation:membership-v7-second"),
            digest(71),
            &receipt("receipt:membership-v7-second"),
            &fixture.context,
        )
        .expect("prepare second member");
        commit_source_membership_v2(
            &mut fixture.registry,
            &second_prepared,
            &mut fixture.port,
            &fixture.context,
        )
        .expect("persist second permanent membership");

        // The ordinary C03 adapter receives the already-qualified C08/C09/C11
        // metadata and existing permanent member rows. Start a fresh C03 journal
        // at revision zero so the rows below are the complete persisted v7 chain.
        let mut c03_registry = fixture.registry.clone();
        c03_registry.revision = 0;
        c03_registry.operations.clear();
        c03_registry.state_encoding_profiles.clear();
        let mut c03_port = InMemoryRegistryJournal::new(100).expect("fresh C03 journal");

        let shared_meaning = semantics();
        let first_request = request(
            &c03_registry,
            &fixture.member,
            70,
            80,
            68,
            69,
            Some(shared_meaning.clone()),
        );
        let first_operation_id = opaque("operation:projection-v7-first");
        let first_receipt = receipt("receipt:projection-v7-first");
        let first = crate::projection_membership::bind_projection_membership(
            &mut c03_registry,
            first_request.clone(),
            0,
            RegistryOperation::new(first_operation_id.clone(), digest(70)),
            &first_receipt,
            &mut c03_port,
            &fixture.context,
        )
        .expect("actual C03 v7 first bind");
        assert!(!first.replayed);

        let second_request = request(
            &c03_registry,
            &second_member,
            72,
            82,
            68,
            69,
            Some(shared_meaning.clone()),
        );
        let second_operation_id = opaque("operation:projection-v7-second");
        let second_receipt = receipt("receipt:projection-v7-second");
        let second = crate::projection_membership::bind_projection_membership(
            &mut c03_registry,
            second_request.clone(),
            1,
            RegistryOperation::new(second_operation_id.clone(), digest(72)),
            &second_receipt,
            &mut c03_port,
            &fixture.context,
        )
        .expect("actual C03 v7 shared-class bind");
        assert!(!second.replayed);

        let mut history = c03_registry
            .operations
            .keys()
            .filter_map(|id| {
                c03_registry
                    .committed_operation(id)
                    .map(|operation| (id.clone(), operation.clone()))
            })
            .collect::<Vec<_>>();
        assert_eq!(history.len(), c03_registry.operations.len());
        history.sort_by_key(|(_, operation)| operation.registry_revision_after());
        assert!(history.iter().all(|(_, operation)| {
            operation.encoding_profile() == "eliot.registry-batch.v7"
                && operation.state_encoding_profile() == "eliot.registry-state.v7"
        }));

        let mut latest = BTreeMap::new();
        for (_, operation) in &history {
            for delta in operation.deltas() {
                latest.insert(
                    delta.key().clone(),
                    (
                        delta.after().clone(),
                        operation.state_encoding_profile(),
                    ),
                );
            }
        }
        let state_bytes = latest
            .into_iter()
            .map(|(key, (state, profile))| {
                (
                    key.encode().expect("state key"),
                    state.encode_with_profile(profile).expect("persisted state"),
                )
            })
            .collect::<Vec<_>>();
        let states = state_bytes
            .iter()
            .map(|(key, value)| RegistryRestoreState { key, value })
            .collect::<Vec<_>>();
        let operation_bytes = history
            .iter()
            .map(|(id, operation)| {
                (id.clone(), operation.encode().expect("persisted operation"))
            })
            .collect::<Vec<_>>();
        let operations = operation_bytes
            .iter()
            .map(|(id, value)| RegistryRestoreOperation {
                operation_id: id,
                value,
            })
            .collect::<Vec<_>>();

        let parts = search_source_history::registry_restore::restore_registry_state(
            2,
            &states,
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES,
        )
        .expect("fresh v7 history decode")
        .into_parts();
        assert_eq!(
            parts.operations,
            history.iter().map(|(_, operation)| operation.clone()).collect::<Vec<_>>()
        );

        // Rebuild the derived C03 binding map from the records proven equal to
        // the freshly decoded persisted operation history.
        let mut restored = c03_registry.clone();
        restored.projection_memberships.clear();
        restored.reverse_projection_memberships.clear();
        restored.state_encoding_profiles = parts.state_encoding_profiles;
        crate::recovery::projection_membership::restore_bindings(&mut restored)
            .expect("fresh C03 binding-index restore");

        let reusable =
            crate::projection_membership::reusable_projection_partitions(&restored, &shared_meaning)
                .expect("restored shared classes");
        assert_eq!(reusable.access(), Some(c::AccessPartitionId::from_bytes([68; 16])));
        assert_eq!(reusable.scoring(), Some(c::ScoringPartitionId::from_bytes([69; 16])));
        for (member, expected) in [
            (&fixture.member, &first_request),
            (&second_member, &second_request),
        ] {
            let record = restored
                .projection_membership_record(member.source_membership_id)
                .expect("restored C03 binding");
            assert_eq!(record.binding(), &expected.binding);
            assert_eq!(
                record.request().partition_semantics.as_ref(),
                Some(&shared_meaning)
            );
        }

        let original = restored
            .committed_operation(&first_operation_id)
            .expect("restored original C03 operation")
            .replay_batch()
            .expect("exact replay batch");
        let [RegistryChange::BindProjectionMembership {
            request: original_request,
            receipt: original_receipt,
        }] = original.changes.as_slice()
        else {
            panic!("one original C03 bind");
        };
        let replay = crate::projection_membership::bind_projection_membership(
            &mut restored,
            original_request.clone(),
            original.expected_registry_revision,
            original.operation.clone(),
            original_receipt,
            &mut c03_port,
            &fixture.context,
        )
        .expect("same original C03 operation after restart");
        assert!(replay.replayed);
        assert_eq!(replay.binding, first.binding);
        assert_eq!(restored.revision(), c03_registry.revision());
        let third_member = persist_member(&fixture, &mut c03_registry, &mut c03_port, 63);
        let refused_legacy_request = request(
            &c03_registry,
            &third_member,
            73,
            83,
            68,
            69,
            None,
        );
        let refused_legacy_id = opaque("operation:legacy-none-reuses-v7-class");
        let refused_legacy_revision = c03_registry.revision();
        assert_eq!(
            crate::projection_membership::bind_projection_membership(
                &mut c03_registry,
                refused_legacy_request,
                refused_legacy_revision,
                RegistryOperation::new(refused_legacy_id.clone(), digest(73)),
                &receipt("receipt:legacy-none-reuses-v7-class"),
                &mut c03_port,
                &fixture.context,
            ),
            Err(RegistryError::ProjectionMembershipConflict)
        );
        assert_eq!(c03_registry.revision(), refused_legacy_revision);
        assert!(c03_registry.committed_operation(&refused_legacy_id).is_none());
        assert!(matches!(
            c03_port
                .load_batch(&refused_legacy_id, &fixture.context)
                .expect("read refused v6 operation"),
            crate::recovery::RegistryBatchLookup::Absent { revision }
                if revision == refused_legacy_revision
        ));

        let legacy_first = request(&c03_registry, &third_member, 74, 84, 88, 89, None);
        let legacy_first_id = opaque("operation:legacy-none-first");
        let legacy_first_receipt = receipt("receipt:legacy-none-first");
        let legacy_first_revision = c03_registry.revision();
        let legacy_first_result = crate::projection_membership::bind_projection_membership(
            &mut c03_registry,
            legacy_first,
            legacy_first_revision,
            RegistryOperation::new(legacy_first_id.clone(), digest(74)),
            &legacy_first_receipt,
            &mut c03_port,
            &fixture.context,
        )
        .expect("legacy v6 class assignment");
        assert!(!legacy_first_result.replayed);
        assert_eq!(
            c03_registry
                .committed_operation(&legacy_first_id)
                .expect("legacy committed history")
                .encoding_profile(),
            "eliot.registry-batch.v6"
        );

        let fourth_member = persist_member(&fixture, &mut c03_registry, &mut c03_port, 66);
        let legacy_second = request(&c03_registry, &fourth_member, 75, 85, 88, 89, None);
        let legacy_second_receipt = receipt("receipt:legacy-none-second");
        let legacy_second_revision = c03_registry.revision();
        let legacy_second_result = crate::projection_membership::bind_projection_membership(
            &mut c03_registry,
            legacy_second,
            legacy_second_revision,
            RegistryOperation::new(
                opaque("operation:legacy-none-second"),
                digest(75),
            ),
            &legacy_second_receipt,
            &mut c03_port,
            &fixture.context,
        )
        .expect("v6 None/None class reuse remains compatible");
        assert!(!legacy_second_result.replayed);

        let fifth_member = persist_member(&fixture, &mut c03_registry, &mut c03_port, 69);
        let new_meaning = ProjectionPartitionSemantics::new(
            canonical("access:new-v2"),
            canonical("scoring:new-v2"),
        )
        .expect("new v7 meaning");
        let refused_v7_request = request(
            &c03_registry,
            &fifth_member,
            76,
            86,
            88,
            89,
            Some(new_meaning),
        );
        let refused_v7_id = opaque("operation:v7-some-reuses-v6-class");
        let refused_v7_revision = c03_registry.revision();
        assert_eq!(
            crate::projection_membership::bind_projection_membership(
                &mut c03_registry,
                refused_v7_request,
                refused_v7_revision,
                RegistryOperation::new(refused_v7_id.clone(), digest(76)),
                &receipt("receipt:v7-some-reuses-v6-class"),
                &mut c03_port,
                &fixture.context,
            ),
            Err(RegistryError::ProjectionMembershipConflict)
        );
        assert_eq!(c03_registry.revision(), refused_v7_revision);
        assert!(c03_registry.committed_operation(&refused_v7_id).is_none());
        assert!(matches!(
            c03_port
                .load_batch(&refused_v7_id, &fixture.context)
                .expect("read refused v7 operation"),
            crate::recovery::RegistryBatchLookup::Absent { revision }
                if revision == refused_v7_revision
        ));
    }
}
