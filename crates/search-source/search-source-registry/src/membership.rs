//! Source/corpus membership with verified admission-receipt gating.
//!
//! One source may hold multiple explicit memberships; each membership is keyed
//! by the exact `(corpus, source)` pair and carries a stable reverse
//! [`search_contracts::SourceMembershipId`]. Membership creation requires an
//! admitted source plus the exact current verified [`AdmissionGrant`]; it can
//! never weaken admission.

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity, SourceMembershipId,
};
use search_ports::{CancellationProbe, MutationIdentity, OperationContext};
use search_source_admission::{AdmissionOutcome, AdmissionReceipt};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};
use crate::source::{RegisteredSource, SourceLifecycle};

pub use search_source_records::membership::{
    BindMembershipRequest, MembershipCommand, MembershipKey, MembershipLifecycle,
    MembershipObligation, MembershipPolicies, MembershipRecord, MembershipTransitionReceipt,
    NewMembership, SourceMembershipReceipt, derive_membership_id,
};

/// Binds one explicit membership under the exact current verified admission.
///
/// The caller supplies the current [`AdmissionReceipt`] bound to the source; a
/// stale, mismatched or missing grant is rejected before any state changes.
#[allow(clippy::too_many_arguments)]
pub fn bind_membership<C, P>(
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    reverse_index: &mut BTreeMap<SourceMembershipId, MembershipKey>,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    request: &BindMembershipRequest,
    admission_receipt: &AdmissionReceipt,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceMembershipReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    limits.validate()?;
    if next_registry_revision
        != expected_registry_revision
            .checked_add(1)
            .ok_or(RegistryError::RegistryRevisionOverflow)?
    {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::MembershipBind,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = memberships
            .get(&request.key)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(SourceMembershipReceipt {
            key: request.key.clone(),
            membership_id: record.membership_id(),
            membership_revision: record.membership_revision(),
            registry_revision: record.registry_revision(),
            operation_id: operation_id.clone(),
            receipt: record.last_receipt().clone(),
            replayed: true,
        });
    }
    let source = sources
        .get(&request.key.source_identity)
        .ok_or(RegistryError::SourceNotAdmitted)?;
    if source.lifecycle() != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if admission_receipt.outcome() != AdmissionOutcome::Allow {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    if admission_receipt.policy_revision() != source.admission().policy_revision()
        || admission_receipt.policy_fingerprint().as_bytes()
            != source.admission().policy_fingerprint().as_bytes()
    {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    if admission_receipt.observation_digest().as_bytes()
        != source.admission().observation_digest().as_bytes()
    {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    if memberships.contains_key(&request.key) {
        return Err(RegistryError::MembershipConflict);
    }
    if let Some(active) = active_generations.get(&request.key.corpus_id)
        && *active != request.generation
    {
        return Err(RegistryError::MembershipGenerationMismatch);
    }
    if memberships.len() >= limits.max_memberships {
        return Err(RegistryError::CapacityExceeded);
    }
    let membership_id = derive_membership_id(&request.key);
    if let Some(existing) = reverse_index.get(&membership_id)
        && existing != &request.key
    {
        return Err(RegistryError::MembershipConflict);
    }
    let membership_revision =
        NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    let replacement = MembershipRecord::new(
        request.key.clone(),
        membership_id,
        request.generation,
        membership_revision,
        MembershipLifecycle::Active,
        next_registry_revision,
        receipt.clone(),
    );
    persist(control_port, &entry, context, &mutation)?;
    memberships.insert(request.key.clone(), replacement);
    reverse_index.insert(membership_id, request.key.clone());
    active_generations
        .entry(request.key.corpus_id.clone())
        .or_insert(request.generation);
    Ok(SourceMembershipReceipt {
        key: request.key.clone(),
        membership_id,
        membership_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Applies one closed membership command with monotonic revision and explicit
/// downstream obligations.
#[allow(clippy::too_many_arguments)]
pub fn transition_membership<C, P>(
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    key: &MembershipKey,
    command: MembershipCommand,
    expected_membership_revision: NonZeroRevision,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<MembershipTransitionReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    if next_registry_revision
        != expected_registry_revision
            .checked_add(1)
            .ok_or(RegistryError::RegistryRevisionOverflow)?
    {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::MembershipTransition,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = memberships
            .get(key)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(MembershipTransitionReceipt {
            key: key.clone(),
            membership_revision: record.membership_revision(),
            registry_revision: record.registry_revision(),
            operation_id: operation_id.clone(),
            obligations: vec![
                MembershipObligation::SecurityReconcile,
                MembershipObligation::PublicationReconcile,
                MembershipObligation::RetentionReconcile,
            ],
            receipt: record.last_receipt().clone(),
            replayed: true,
        });
    }
    let record = memberships
        .get_mut(key)
        .ok_or(RegistryError::MembershipNotFound)?;
    if record.membership_revision() != expected_membership_revision {
        return Err(RegistryError::MembershipGenerationMismatch);
    }
    let next_lifecycle = match (record.lifecycle(), command) {
        (
            MembershipLifecycle::Restricted | MembershipLifecycle::Suspended,
            MembershipCommand::Activate,
        ) => MembershipLifecycle::Active,
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
        _ => return Err(RegistryError::MembershipGenerationMismatch),
    };
    let next_revision = record
        .membership_revision()
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let replacement = MembershipRecord::new(
        record.key().clone(),
        record.membership_id(),
        record.generation(),
        next_revision,
        next_lifecycle,
        next_registry_revision,
        receipt.clone(),
    );
    persist(control_port, &entry, context, &mutation)?;
    *record = replacement;
    Ok(MembershipTransitionReceipt {
        key: key.clone(),
        membership_revision: record.membership_revision(),
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        obligations: vec![
            MembershipObligation::SecurityReconcile,
            MembershipObligation::PublicationReconcile,
            MembershipObligation::RetentionReconcile,
        ],
        receipt: receipt.clone(),
        replayed: false,
    })
}

fn load_existing<C, P>(
    control_port: &P,
    operation_id: &OpaqueId,
    context: &OperationContext<C>,
) -> Result<Option<RegistryJournalEntry>, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

fn persist<C, P>(
    control_port: &mut P,
    entry: &RegistryJournalEntry,
    context: &OperationContext<C>,
    mutation: &MutationIdentity,
) -> Result<(), RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    control_port
        .persist_entry(entry, context, mutation)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    Ok(())
}
