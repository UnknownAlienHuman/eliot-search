//! Explicit versioned reference portfolios over admitted memberships.
//!
//! Portfolio selection is explicit and versioned. Only admitted active
//! sources participate; an empty scope without explicit permission returns a
//! typed reason instead of an implicit fallback.

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, ReferencePortfolioId, SourceIdentity,
    SourceMembershipId,
};
use search_ports::{CancellationProbe, MutationIdentity, OperationContext};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::portfolio_history::{
    ImmutablePortfolioRevision, PortfolioHistory, PortfolioRevisionKey,
};
use crate::source::{RegisteredSource, SourceLifecycle};

pub use search_source_records::portfolio::{
    PortfolioItem, PortfolioReceipt, PublishPortfolioRequest, ReferencePortfolioRecord,
};

/// Publishes one explicit reference portfolio revision.
///
/// Empty scope without explicit permission returns [`RegistryError::ReferenceScopeEmpty`].
/// Invalid membership refs or finite-bound violations return
/// [`RegistryError::ReferencePortfolioInvalid`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_reference_portfolio<C, P>(
    portfolios: &mut BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
    history: &mut PortfolioHistory,
    memberships: &BTreeMap<MembershipKey, MembershipRecord>,
    reverse_index: &BTreeMap<SourceMembershipId, MembershipKey>,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    request: &PublishPortfolioRequest,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<PortfolioReceipt, RegistryError>
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
    if request.membership_precedence.len() > limits.max_portfolio_items {
        return Err(RegistryError::ReferencePortfolioInvalid);
    }
    if request.membership_precedence.is_empty() && !request.allow_empty {
        return Err(RegistryError::ReferenceScopeEmpty);
    }
    let key = PortfolioRevisionKey::new(request.portfolio_id, request.portfolio_revision);
    if let Some(existing) = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
    {
        if existing.mutation_digest != mutation_digest
            || existing.kind != JournalEntryKind::PortfolioPublish
        {
            return Err(RegistryError::OperationConflict);
        }
        let record = history
            .revision(&key)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        if record.membership_precedence() != request.membership_precedence
            || record.operation_id() != operation_id
        {
            return Err(RegistryError::OperationConflict);
        }
        if record.registry_revision() != existing.after_revision
            || record.last_receipt() != &existing.receipt
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        return Ok(PortfolioReceipt {
            portfolio_id: request.portfolio_id,
            portfolio_revision: record.portfolio_revision(),
            registry_revision: existing.after_revision,
            operation_id: operation_id.clone(),
            receipt: existing.receipt,
            replayed: true,
        });
    }
    if let Some(record) = history.revision(&key) {
        if record.membership_precedence() != request.membership_precedence {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        return Ok(PortfolioReceipt {
            portfolio_id: request.portfolio_id,
            portfolio_revision: record.portfolio_revision(),
            registry_revision: record.registry_revision(),
            // No operation was committed for this request. Return the
            // original publication identity instead of acknowledging O2.
            operation_id: record.operation_id().clone(),
            receipt: record.last_receipt().clone(),
            replayed: true,
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for membership_id in &request.membership_precedence {
        if !seen.insert(*membership_id) {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        let key = reverse_index
            .get(membership_id)
            .ok_or(RegistryError::ReferencePortfolioInvalid)?;
        let membership = memberships
            .get(key)
            .ok_or(RegistryError::ReferencePortfolioInvalid)?;
        if membership.membership_id() != *membership_id {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        if membership.lifecycle() != MembershipLifecycle::Active {
            return Err(RegistryError::ReferencePortfolioInvalid);
        }
        let source = sources
            .get(&membership.key().source_identity)
            .ok_or(RegistryError::SourceNotAdmitted)?;
        if source.lifecycle() != SourceLifecycle::Active {
            return Err(RegistryError::SourceRetired);
        }
    }
    let mutation = registry_mutation(operation_id);
    let entry = RegistryJournalEntry::new(
        operation_id.clone(),
        mutation_digest,
        expected_registry_revision,
        next_registry_revision,
        JournalEntryKind::PortfolioPublish,
        receipt.clone(),
    );
    if !portfolios.contains_key(&request.portfolio_id) && portfolios.len() >= limits.max_portfolios
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let prepared = history.prepare_publication(
        key,
        ImmutablePortfolioRevision::new(request.membership_precedence.clone()),
        next_registry_revision,
        receipt,
        operation_id,
        limits.max_portfolio_revisions,
        request.expected_current,
    )?;
    let projected = ReferencePortfolioRecord {
        portfolio_id: prepared.record().portfolio_id(),
        portfolio_revision: prepared.record().portfolio_revision(),
        membership_precedence: prepared.record().membership_precedence().to_vec(),
        registry_revision: prepared.record().registry_revision(),
        last_receipt: prepared.record().last_receipt().clone(),
    };
    control_port
        .persist_entry(&entry, context, &mutation)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    // This port persists a receipt, not the complete registry state. All
    // fallible preparation precedes dispatch; installation cannot refuse.
    history.install(prepared);
    portfolios.insert(request.portfolio_id, projected);
    Ok(PortfolioReceipt {
        portfolio_id: request.portfolio_id,
        portfolio_revision: request.portfolio_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Returns the active admitted portfolio for one exact corpus generation.
///
/// Only active memberships over active sources are returned; a retired source
/// fails closed instead of being silently skipped.
pub fn active_portfolio_for_generation(
    memberships: &BTreeMap<MembershipKey, MembershipRecord>,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    active_generations: &BTreeMap<OpaqueId, NonZeroRevision>,
    corpus_id: &OpaqueId,
    generation: NonZeroRevision,
    max_items: usize,
    limits: RegistryLimits,
) -> Result<Vec<PortfolioItem>, RegistryError> {
    if max_items == 0 || max_items > limits.max_portfolio_items {
        return Err(RegistryError::PortfolioLimitInvalid);
    }
    if active_generations.get(corpus_id).copied() != Some(generation) {
        return Err(RegistryError::CutoverGenerationConflict);
    }
    let mut result = Vec::new();
    for (key, membership) in memberships {
        if &key.corpus_id != corpus_id
            || membership.generation() != generation
            || membership.lifecycle() != MembershipLifecycle::Active
        {
            continue;
        }
        let source = sources
            .get(&key.source_identity)
            .ok_or(RegistryError::SourceNotFound)?;
        if source.lifecycle() != SourceLifecycle::Active {
            return Err(RegistryError::SourceRetired);
        }
        if result.len() >= max_items {
            return Err(RegistryError::PortfolioLimitInvalid);
        }
        result.push(PortfolioItem {
            source: source.clone(),
            membership: membership.clone(),
        });
    }
    Ok(result)
}

#[allow(dead_code)]
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
