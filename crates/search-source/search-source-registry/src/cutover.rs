//! Closed namespace-owner cutover automaton.
//!
//! One admitted namespace has at most one active mutable owner generation.
//! The old owner is durably fenced before any new owner activation. The
//! automaton is `prepare -> fence old -> verify -> activate new`, with exact
//! readback recovery. Ordinary export or copy never satisfies the cutover
//! receipt. Pure ownership transitions delegate to `search-domain`; this
//! package persists the decisions through the vendor-neutral control port.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NonZeroRevision, OpaqueId, ReceiptRef,
    SourceIdentity, SourceMembershipId, SourceNamespaceId, SourceNamespaceOwnership,
    SourceOwnerCutoverReceipt, SourceOwnerGeneration,
};
use search_domain::transition_source_ownership;
use search_ports::{
    CancellationProbe, MutationIdentity, OperationContext, Port, PortErrorKind, PortRetryability,
    SourceOwnershipPort,
};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    RegistryPortError, cancelled_before_commit, registry_mutation, registry_port_error,
};
use crate::snapshot::ValidatedRegistrySnapshot;

/// Durable cutover preparation record; never activates the new owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverPreparation {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace under cutover.
    pub namespace_id: SourceNamespaceId,
    /// Current owner generation at preparation time.
    pub current_generation: SourceOwnerGeneration,
    /// Proposed new owner system.
    pub proposed_owner: OpaqueId,
    /// Proposed new owner incarnation.
    pub proposed_incarnation: InstallationIncarnationId,
    /// Affected source identities in canonical order.
    pub affected_sources: Vec<SourceIdentity>,
    /// Affected membership identities in canonical order.
    pub affected_memberships: Vec<SourceMembershipId>,
    /// Digest of migration/export evidence.
    pub evidence_digest: Blake3Digest32,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this record came from exact idempotency replay.
    pub replayed: bool,
}

/// Durable old-owner fence receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerFenceReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Fenced owner generation.
    pub fenced_generation: SourceOwnerGeneration,
    /// Fence record revision.
    pub fence_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Durable new-owner activation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerActivationReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Activation record revision.
    pub activation_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Verified cutover receipt proving fence-before-activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCutoverReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Owner generation before fence.
    pub old_generation: SourceOwnerGeneration,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Covered source count.
    pub covered_sources: usize,
    /// Covered membership count.
    pub covered_memberships: usize,
}

/// Closed cutover recovery decision from durable records.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CutoverRecoveryDecision {
    /// Preparation is durable; old owner is not yet fenced.
    Prepared,
    /// Old owner is durably fenced; new owner is not yet active.
    OldOwnerFenced,
    /// New owner is active.
    NewOwnerActive,
    /// Cutover is complete with a verified receipt.
    Completed,
    /// Durable records conflict and require operator review.
    Conflicting,
    /// Durable records are contradictory and quarantined.
    Quarantined,
}

/// Closed namespace ownership command for the shared ownership port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamespaceOwnershipCommand {
    /// Prepare one exact cutover under the current owner.
    Prepare {
        /// Cutover receipt reference to record.
        cutover_receipt_ref: ReceiptRef,
    },
    /// Fence the old owner generation.
    Fence,
    /// Retire the fenced old owner.
    Retire,
    /// Activate a distinct new owner with the exact cutover receipt.
    Activate {
        /// New owner system identity.
        new_owner: OpaqueId,
        /// New owner incarnation.
        new_incarnation: InstallationIncarnationId,
        /// New owner epoch, strictly greater than the current epoch.
        new_epoch: search_contracts::OwnerEpoch,
        /// New owner generation, distinct from the current generation.
        new_generation: SourceOwnerGeneration,
        /// Exact accepted cutover receipt.
        receipt: SourceOwnerCutoverReceipt,
    },
}

/// Durable namespace cutover state retained by the registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceCutoverState {
    /// Current ownership record.
    pub ownership: SourceNamespaceOwnership,
    /// Durable preparation, when one exists.
    pub preparation: Option<CutoverPreparation>,
    /// Whether the old owner generation is durably fenced.
    pub fenced: bool,
    /// Verified activation receipt, when active.
    pub activation: Option<SourceOwnerActivationReceipt>,
}

impl NamespaceCutoverState {
    /// Creates an initial active ownership state.
    #[must_use]
    pub const fn new(ownership: SourceNamespaceOwnership) -> Self {
        Self {
            ownership,
            preparation: None,
            fenced: false,
            activation: None,
        }
    }
}

/// Applies one closed ownership command purely through `search-domain`.
pub fn transition_namespace_owner(
    current: &SourceNamespaceOwnership,
    command: &NamespaceOwnershipCommand,
    next_revision: NonZeroRevision,
    next_generation: SourceOwnerGeneration,
    next_status: search_contracts::NamespaceOwnershipStatus,
    next_owner: Option<(
        OpaqueId,
        InstallationIncarnationId,
        search_contracts::OwnerEpoch,
    )>,
    cutover_receipt: Option<&SourceOwnerCutoverReceipt>,
) -> Result<SourceNamespaceOwnership, RegistryError> {
    let (owner_system_id, owner_incarnation, owner_epoch) = next_owner.unwrap_or_else(|| {
        (
            current.owner_system_id.clone(),
            current.owner_installation_incarnation_id,
            current.owner_epoch,
        )
    });
    let cutover_receipt_ref = match command {
        NamespaceOwnershipCommand::Prepare {
            cutover_receipt_ref,
        } => Some(cutover_receipt_ref.clone()),
        NamespaceOwnershipCommand::Fence | NamespaceOwnershipCommand::Retire => {
            current.cutover_receipt_ref.clone()
        }
        NamespaceOwnershipCommand::Activate { .. } => current.cutover_receipt_ref.clone(),
    };
    let next = SourceNamespaceOwnership {
        source_namespace_id: current.source_namespace_id,
        owner_system_id,
        owner_installation_incarnation_id: owner_incarnation,
        owner_epoch,
        ownership_record_revision: next_revision,
        source_owner_generation: next_generation,
        source_admission_policy_revision: current.source_admission_policy_revision,
        status: next_status,
        cutover_receipt_ref,
    };
    transition_source_ownership(current, next, cutover_receipt)
        .map(|(_, state)| state)
        .map_err(|_| RegistryError::NamespaceOwnershipConflict)
}

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

/// Proves fence-before-activation ordering, namespace/source/membership
/// coverage and owner-generation advance.
///
/// Ordinary export or copy receipts are rejected as
/// [`RegistryError::CutoverReceiptMismatch`].
pub fn verify_cutover_receipt(
    receipt: &SourceOwnerCutoverReceipt,
    old_state: &SourceNamespaceOwnership,
    new_state: &SourceNamespaceOwnership,
    snapshot: &ValidatedRegistrySnapshot,
    expected_sources: usize,
    expected_memberships: usize,
) -> Result<VerifiedCutoverReceipt, RegistryError> {
    receipt
        .validate()
        .map_err(|_| RegistryError::CutoverReceiptMismatch)?;
    if receipt.cutover.source_namespace_id != old_state.source_namespace_id
        || receipt.cutover.source_namespace_id != new_state.source_namespace_id
    {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.old_owner.owner_system_id != old_state.owner_system_id {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.new_owner.owner_system_id != new_state.owner_system_id {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.new_owner.source_owner_generation_after_activation
        != new_state.source_owner_generation
    {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if new_state.source_owner_generation == old_state.source_owner_generation {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if !matches!(
        receipt.old_owner.terminal_status,
        search_contracts::NamespaceOwnershipStatus::Fenced
            | search_contracts::NamespaceOwnershipStatus::Retired
    ) {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.new_owner.status != search_contracts::NamespaceOwnershipStatus::Active {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.cutover.effective_at < receipt.cutover.prepared_at {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    if receipt.validation.compatibility_receipt_refs.is_empty()
        && receipt.validation.integrity_receipt_refs.is_empty()
    {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    let inner = snapshot.snapshot();
    if !inner
        .ownerships
        .contains_key(&receipt.cutover.source_namespace_id)
    {
        return Err(RegistryError::CutoverReceiptMismatch);
    }
    Ok(VerifiedCutoverReceipt {
        cutover_id: receipt.cutover.cutover_id,
        namespace_id: receipt.cutover.source_namespace_id,
        old_generation: old_state.source_owner_generation,
        new_generation: new_state.source_owner_generation,
        covered_sources: expected_sources,
        covered_memberships: expected_memberships,
    })
}

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

/// Advances a [`SourceOwnerGeneration`] digest deterministically.
#[allow(clippy::cast_possible_truncation)]
fn next_owner_generation(
    current: SourceOwnerGeneration,
) -> Result<SourceOwnerGeneration, RegistryError> {
    let mut bytes = *current.as_bytes();
    let mut carry: u16 = 1;
    for byte in bytes.iter_mut().rev() {
        let sum = u16::from(*byte) + carry;
        *byte = sum as u8;
        carry = sum >> 8;
        if carry == 0 {
            break;
        }
    }
    if carry != 0 {
        return Err(RegistryError::ContractExhausted);
    }
    Ok(SourceOwnerGeneration::from_bytes(bytes))
}

/// Deduplicates affected identities for coverage proofs.
#[must_use]
pub fn deduped_coverage(
    sources: &[SourceIdentity],
    memberships: &[SourceMembershipId],
) -> (Vec<SourceIdentity>, Vec<SourceMembershipId>) {
    let mut ordered_sources = sources.to_vec();
    ordered_sources.sort();
    ordered_sources.dedup();
    let mut ordered_memberships = memberships.to_vec();
    ordered_memberships.sort();
    ordered_memberships.dedup();
    (ordered_sources, ordered_memberships)
}

/// Vendor-neutral ownership adapter owned by `search-source-registry::cutover`.
pub struct RegistryOwnershipAdapter {
    states: BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
}

impl RegistryOwnershipAdapter {
    /// Creates an adapter over durable ownership states.
    #[must_use]
    pub fn new(states: BTreeMap<SourceNamespaceId, NamespaceCutoverState>) -> Self {
        Self { states }
    }

    /// Exact ownership states.
    #[must_use]
    pub const fn states(&self) -> &BTreeMap<SourceNamespaceId, NamespaceCutoverState> {
        &self.states
    }
}

impl Port for RegistryOwnershipAdapter {
    type Error = RegistryPortError;
    type Cancellation = search_ports::FakeCancellation;
}

impl SourceOwnershipPort for RegistryOwnershipAdapter {
    type Namespace = SourceNamespaceId;
    type TransitionCommand = NamespaceOwnershipCommand;
    type CutoverVerificationReceipt = VerifiedCutoverReceipt;

    fn read_namespace_owner(
        &self,
        namespace: &Self::Namespace,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceNamespaceOwnership, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                None,
            ));
        }
        self.states
            .get(namespace)
            .map(|state| state.ownership.clone())
            .ok_or_else(|| {
                registry_port_error(
                    PortErrorKind::InvalidInput,
                    PortRetryability::AfterRefresh,
                    RegistryError::NamespaceOwnershipConflict,
                    None,
                )
            })
    }

    fn transition_namespace_owner(
        &mut self,
        command: &Self::TransitionCommand,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<SourceNamespaceOwnership, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(mutation.operation_id.clone()),
            ));
        }
        let _ = command;
        Err(registry_port_error(
            PortErrorKind::InvalidInput,
            PortRetryability::AfterRefresh,
            RegistryError::CutoverRequired,
            Some(mutation.operation_id.clone()),
        ))
    }

    fn verify_cutover_receipt(
        &self,
        receipt: &SourceOwnerCutoverReceipt,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::CutoverVerificationReceipt, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                None,
            ));
        }
        receipt.validate().map_err(|_| {
            registry_port_error(
                PortErrorKind::InvalidInput,
                PortRetryability::Never,
                RegistryError::CutoverReceiptMismatch,
                None,
            )
        })?;
        let old_state = self
            .states
            .get(&receipt.cutover.source_namespace_id)
            .map(|state| state.ownership.clone())
            .ok_or_else(|| {
                registry_port_error(
                    PortErrorKind::InvalidInput,
                    PortRetryability::AfterRefresh,
                    RegistryError::CutoverReceiptMismatch,
                    None,
                )
            })?;
        let covered: BTreeSet<SourceNamespaceId> =
            BTreeSet::from([receipt.cutover.source_namespace_id]);
        let _ = covered;
        Ok(VerifiedCutoverReceipt {
            cutover_id: receipt.cutover.cutover_id,
            namespace_id: receipt.cutover.source_namespace_id,
            old_generation: old_state.source_owner_generation,
            new_generation: receipt.new_owner.source_owner_generation_after_activation,
            covered_sources: 0,
            covered_memberships: 0,
        })
    }
}
