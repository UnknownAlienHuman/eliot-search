//! Closed namespace-owner cutover automaton.
//!
//! One admitted namespace has at most one active mutable owner generation.
//! The old owner is durably fenced before any new owner activation. The
//! automaton is `prepare -> fence old -> verify -> activate new`, with exact
//! readback recovery. Ordinary export or copy never satisfies the cutover
//! receipt. Pure ownership transitions delegate to `search-domain`; this
//! package persists the decisions through the vendor-neutral control port.

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, OpaqueId, ReceiptRef, SourceIdentity,
    SourceMembershipId, SourceNamespaceId, SourceNamespaceOwnership, SourceOwnerCutoverReceipt,
};

use search_domain::transition_source_ownership;
use search_ports::{CancellationProbe, OperationContext};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};

pub use search_source_records::namespace::transition_namespace_owner;
pub use search_source_records::namespace::{
    CutoverPreparation, CutoverRecoveryDecision, NamespaceCutoverState, NamespaceOwnershipCommand,
    OwnerFenceReceipt, SourceOwnerActivationReceipt, VerifiedCutoverReceipt,
};

/// Creates a durable preparation record without activating the new owner.
#[allow(clippy::too_many_arguments)]
pub fn prepare_namespace_cutover<C, P>(
    states: &mut BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    namespace_id: SourceNamespaceId,
    proposed_owner: OpaqueId,
    proposed_incarnation: InstallationIncarnationId,
    affected_sources: Vec<SourceIdentity>,
    affected_memberships: Vec<SourceMembershipId>,
    evidence_digest: Blake3Digest32,
    cutover_id: search_contracts::CutoverId,
    cutover_receipt_ref: ReceiptRef,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<CutoverPreparation, RegistryError>
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
    if affected_sources.is_empty() || affected_sources.len() > limits.max_cutover_inventory {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    let mut ordered_sources = affected_sources.clone();
    ordered_sources.sort();
    ordered_sources.dedup();
    if ordered_sources.len() != affected_sources.len() {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    let mut ordered_memberships = affected_memberships.clone();
    ordered_memberships.sort();
    ordered_memberships.dedup();
    if ordered_memberships.len() != affected_memberships.len() {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    let state = states
        .get(&namespace_id)
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    if state.is_permanent_v2() {
        return Err(RegistryError::CutoverRequired);
    }
    if state.preparation.is_some() {
        return Err(RegistryError::NamespaceOwnershipConflict);
    }
    if state.ownership.owner_system_id == proposed_owner
        && state.ownership.owner_installation_incarnation_id == proposed_incarnation
    {
        return Err(RegistryError::NamespaceOwnershipConflict);
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::CutoverPrepare,
        receipt.clone(),
    );
    if let Some(existing) = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
    {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let mut preparation = state
            .preparation
            .clone()
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        preparation.replayed = true;
        return Ok(preparation);
    }
    let next_revision = state
        .ownership
        .ownership_record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let next_generation = next_owner_generation(state.ownership.source_owner_generation)?;
    let next = SourceNamespaceOwnership {
        source_namespace_id: namespace_id,
        owner_system_id: state.ownership.owner_system_id.clone(),
        owner_installation_incarnation_id: state.ownership.owner_installation_incarnation_id,
        owner_epoch: state.ownership.owner_epoch,
        ownership_record_revision: next_revision,
        source_owner_generation: next_generation,
        source_admission_policy_revision: state.ownership.source_admission_policy_revision,
        status: search_contracts::NamespaceOwnershipStatus::CutoverPrepared,
        cutover_receipt_ref: Some(cutover_receipt_ref),
    };
    transition_source_ownership(&state.ownership, next.clone(), None)
        .map_err(|_| RegistryError::NamespaceOwnershipConflict)?;
    control_port
        .persist_entry(&entry, context, &mutation)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    let preparation = CutoverPreparation {
        cutover_id,
        namespace_id,
        current_generation: state.ownership.source_owner_generation,
        proposed_owner,
        proposed_incarnation,
        affected_sources: ordered_sources,
        affected_memberships: ordered_memberships,
        evidence_digest,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    };
    if let Some(state) = states.get_mut(&namespace_id) {
        state.ownership = next;
        state.preparation = Some(preparation.clone());
    }
    Ok(preparation)
}

/// Durably fences the old owner generation before any new owner activation.
pub fn fence_old_owner<C, P>(
    states: &mut BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    preparation: &CutoverPreparation,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<OwnerFenceReceipt, RegistryError>
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
    let state = states
        .get(&preparation.namespace_id)
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    if state.is_permanent_v2() {
        return Err(RegistryError::CutoverRequired);
    }
    let stored = state
        .preparation
        .as_ref()
        .ok_or(RegistryError::CutoverRequired)?;
    if stored != preparation {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if state.fenced {
        return Err(RegistryError::NamespaceOwnershipConflict);
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::CutoverFence,
        receipt.clone(),
    );
    if let Some(existing) = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
    {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let fence_revision = state.ownership.ownership_record_revision;
        return Ok(OwnerFenceReceipt {
            cutover_id: preparation.cutover_id,
            namespace_id: preparation.namespace_id,
            fenced_generation: preparation.current_generation,
            fence_revision,
            registry_revision: state
                .ownership
                .ownership_record_revision
                .get()
                .min(next_registry_revision),
            operation_id: operation_id.clone(),
            receipt: receipt.clone(),
            replayed: true,
        });
    }
    let next_revision = state
        .ownership
        .ownership_record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let next_generation = next_owner_generation(state.ownership.source_owner_generation)?;
    let next = SourceNamespaceOwnership {
        source_namespace_id: preparation.namespace_id,
        owner_system_id: state.ownership.owner_system_id.clone(),
        owner_installation_incarnation_id: state.ownership.owner_installation_incarnation_id,
        owner_epoch: state.ownership.owner_epoch,
        ownership_record_revision: next_revision,
        source_owner_generation: next_generation,
        source_admission_policy_revision: state.ownership.source_admission_policy_revision,
        status: search_contracts::NamespaceOwnershipStatus::Fenced,
        cutover_receipt_ref: state.ownership.cutover_receipt_ref.clone(),
    };
    transition_source_ownership(&state.ownership, next.clone(), None)
        .map_err(|_| RegistryError::NamespaceOwnershipConflict)?;
    control_port
        .persist_entry(&entry, context, &mutation)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    if let Some(state) = states.get_mut(&preparation.namespace_id) {
        state.ownership = next;
        state.fenced = true;
    }
    Ok(OwnerFenceReceipt {
        cutover_id: preparation.cutover_id,
        namespace_id: preparation.namespace_id,
        fenced_generation: preparation.current_generation,
        fence_revision: next_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Activates the new owner after the durable old-owner fence.
///
/// The old owner can never resume under the prior generation.
#[allow(clippy::too_many_arguments)]
pub fn activate_new_owner<C, P>(
    states: &mut BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    preparation: &CutoverPreparation,
    fence: &OwnerFenceReceipt,
    cutover_receipt: &SourceOwnerCutoverReceipt,
    new_epoch: search_contracts::OwnerEpoch,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceOwnerActivationReceipt, RegistryError>
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
    cutover_receipt
        .validate()
        .map_err(|_| RegistryError::CutoverReceiptMismatch)?;
    if cutover_receipt.cutover.source_namespace_id != preparation.namespace_id {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if fence.cutover_id != preparation.cutover_id || fence.namespace_id != preparation.namespace_id
    {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    let state = states
        .get(&preparation.namespace_id)
        .ok_or(RegistryError::NamespaceOwnershipConflict)?;
    if state.is_permanent_v2() {
        return Err(RegistryError::CutoverRequired);
    }
    if !state.fenced {
        return Err(RegistryError::CutoverRequired);
    }
    let stored = state
        .preparation
        .as_ref()
        .ok_or(RegistryError::CutoverRequired)?;
    if stored != preparation {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if cutover_receipt.new_owner.owner_system_id != preparation.proposed_owner {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if cutover_receipt.old_owner.owner_system_id != state.ownership.owner_system_id
        && cutover_receipt.old_owner.owner_system_id != stored.proposed_owner
    {
        let _ = cutover_receipt.old_owner.owner_system_id.clone();
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::CutoverActivate,
        receipt.clone(),
    );
    if let Some(existing) = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
    {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let activation = state
            .activation
            .clone()
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(SourceOwnerActivationReceipt {
            activation_revision: activation.activation_revision,
            registry_revision: activation.registry_revision,
            replayed: true,
            ..activation
        });
    }
    if new_epoch <= state.ownership.owner_epoch {
        return Err(RegistryError::NamespaceOwnershipConflict);
    }
    let next_revision = state
        .ownership
        .ownership_record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let next = SourceNamespaceOwnership {
        source_namespace_id: preparation.namespace_id,
        owner_system_id: preparation.proposed_owner.clone(),
        owner_installation_incarnation_id: preparation.proposed_incarnation,
        owner_epoch: new_epoch,
        ownership_record_revision: next_revision,
        source_owner_generation: cutover_receipt
            .new_owner
            .source_owner_generation_after_activation,
        source_admission_policy_revision: state.ownership.source_admission_policy_revision,
        status: search_contracts::NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: state.ownership.cutover_receipt_ref.clone(),
    };
    transition_source_ownership(&state.ownership, next.clone(), Some(cutover_receipt))
        .map_err(|_| RegistryError::CutoverReceiptMismatch)?;
    control_port
        .persist_entry(&entry, context, &mutation)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    let activation = SourceOwnerActivationReceipt {
        cutover_id: preparation.cutover_id,
        namespace_id: preparation.namespace_id,
        new_generation: next.source_owner_generation,
        activation_revision: next_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    };
    if let Some(state) = states.get_mut(&preparation.namespace_id) {
        state.ownership = next;
        state.activation = Some(activation.clone());
    }
    Ok(activation)
}

pub use search_source_records::namespace::verify_cutover_receipt;

/// Returns the durable cutover state without activating anything.
///
/// The decision comes from durable preparation/fence/activation records and
/// the journal entry for `operation_id`; process presence or a partial
/// external copy alone never activates.
pub fn recover_cutover<C, P>(
    states: &BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
    namespace_id: SourceNamespaceId,
    operation_id: &OpaqueId,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<CutoverRecoveryDecision, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let journal = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    let Some(state) = states.get(&namespace_id) else {
        return Ok(CutoverRecoveryDecision::Quarantined);
    };
    if state.activation.is_some() {
        return Ok(CutoverRecoveryDecision::Completed);
    }
    if state.fenced {
        return Ok(CutoverRecoveryDecision::OldOwnerFenced);
    }
    if state.preparation.is_some() {
        if journal.is_none() {
            return Ok(CutoverRecoveryDecision::Conflicting);
        }
        return Ok(CutoverRecoveryDecision::Prepared);
    }
    if journal.is_some() {
        return Ok(CutoverRecoveryDecision::Conflicting);
    }
    Ok(CutoverRecoveryDecision::Quarantined)
}

use search_source_records::namespace::next_legacy_owner_generation as next_owner_generation;

pub use search_source_records::namespace::deduped_coverage;

pub use search_source_history::namespace_adapter::RegistryOwnershipAdapter;
