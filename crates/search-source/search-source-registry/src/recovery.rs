//! Atomic batches, unknown-outcome recovery and admission batch receipts.
//!
//! Every batch is finite, exact-revision guarded, replay-fenced, staged
//! before commit and either applies completely or not at all. Unknown commit
//! outcomes are resolved by exact idempotency/entity/generation readback,
//! never by retrying with a different payload.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, ReferencePortfolioId, SourceIdentity,
};
use search_ports::{CancellationProbe, OperationContext, PortErrorKind, PortFailure};
use search_source_admission::AdmissionReceipt;
use search_source_identity::SourceBinding;

use crate::cutover::NamespaceCutoverState;
use crate::error::{
    RegistryControlPort, RegistryError, RegistryLimits, cancelled_before_commit, registry_mutation,
};
use crate::membership::{
    MembershipKey, MembershipLifecycle, MembershipRecord, NewMembership, derive_membership_id,
};
use crate::portfolio::{PortfolioItem, ReferencePortfolioRecord};
use crate::portfolio_history::PortfolioHistory;
use crate::root::RootRecord;
use crate::source::{AdmissionBindingProof, RegisteredSource, SourceLifecycle};

#[path = "batch.rs"]
mod batch;
#[path = "source_v2.rs"]
pub mod source_v2;
pub use batch::{
    DecodedBatch, PreparedBatch, PreparedStateDelta, RegistryBatchLookup, RegistryBatchReadback,
    RegistryStateDelta, RegistryStateFamily, RegistryStateKey, RegistryStateRecord,
    decode_batch_record, decode_registry_state, decode_registry_state_entry,
    decode_registry_state_key,
};

#[path = "registry_restore.rs"]
mod restore;
pub use restore::{
    MAX_REGISTRY_RESTORE_BYTES, ObservedRegistryRestore, RegistryRestoreOperation,
    RegistryRestoreState, preflight_registry_restore_inputs, restore_registry_state,
    restore_registry_state_v4,
};

pub use search_source_records::operation::{
    BatchItemOutcome, BatchItemStatus, NamespaceCutover, RegistryBatch, RegistryBatchReceipt,
    RegistryChange, RegistryMutationRecovery, RegistryOperation, RegistryReceipt,
};

pub use search_source_records::batch::{
    BatchCodecV4Error, OriginalInputResolver, decode_batch_record_v4, decode_registry_state_key_v4,
    decode_registry_state_v4, required_original_refs_from_state_v4, required_original_refs_from_batch_v4,
    decode_batch_record_with_original_probe, decode_registry_state_entry_profiled_with_original_probe,
};

/// Finite atomic source registry.
#[derive(Clone, Debug)]
pub struct SourceRegistry {
    limits: RegistryLimits,
    revision: u64,
    roots: BTreeMap<search_contracts::RootBindingId, RootRecord>,
    sources: BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: BTreeMap<MembershipKey, MembershipRecord>,
    reverse_memberships: BTreeMap<search_contracts::SourceMembershipId, MembershipKey>,
    portfolios: BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
    portfolio_history: PortfolioHistory,
    namespaces: BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState>,
    active_generations: BTreeMap<OpaqueId, NonZeroRevision>,
    operations: BTreeMap<OpaqueId, RetainedRegistryOperation>,
    identity_cache_v2: Option<source_v2::IdentityCacheV2>,
    pending_batch: Option<(OpaqueId, Blake3Digest32)>,
    // Exact physical value formats established by full owner history/readback.
    // Missing format provenance never guesses the current writer's version.
    state_encoding_profiles: BTreeMap<RegistryStateKey, &'static str>,
}

#[derive(Clone, Debug)]
enum RetainedRegistryOperation {
    ProcessLocal(RegistryReceipt),
    Committed(DecodedBatch),
}
impl RetainedRegistryOperation {
    fn digest(&self) -> Blake3Digest32 {
        match self {
            Self::ProcessLocal(receipt) => receipt.mutation_digest,
            Self::Committed(record) => record.entry().mutation_digest,
        }
    }
    fn receipt(&self, replayed: bool) -> RegistryReceipt {
        match self {
            Self::ProcessLocal(receipt) => {
                let mut receipt = receipt.clone();
                receipt.replayed = replayed;
                receipt
            }
            Self::Committed(record) => record.registry_receipt(replayed),
        }
    }
}

impl SourceRegistry {
    /// Creates an empty finite registry.
    pub fn new(limits: RegistryLimits) -> Result<Self, RegistryError> {
        Ok(Self {
            limits: limits.validate()?,
            revision: 0,
            roots: BTreeMap::new(),
            sources: BTreeMap::new(),
            memberships: BTreeMap::new(),
            reverse_memberships: BTreeMap::new(),
            portfolios: BTreeMap::new(),
            portfolio_history: PortfolioHistory::new(),
            namespaces: BTreeMap::new(),
            active_generations: BTreeMap::new(),
            operations: BTreeMap::new(),
            identity_cache_v2: None,
            pending_batch: None,
            state_encoding_profiles: BTreeMap::new(),
        })
    }

    /// Exact current registry revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Finite limits.
    pub const fn limits(&self) -> RegistryLimits {
        self.limits
    }

    /// Complete retained committed operation, including original receipt and historical targets.
    ///
    /// This observed cache provides no independent durable or live-authority proof.
    pub fn committed_operation(&self, id: &OpaqueId) -> Option<&DecodedBatch> {
        self.operations
            .get(id)
            .and_then(|operation| match operation {
                RetainedRegistryOperation::Committed(record) => Some(record),
                RetainedRegistryOperation::ProcessLocal(_) => None,
            })
    }

    /// Returns one exact source record.
    pub fn source(&self, identity: &SourceIdentity) -> Result<&RegisteredSource, RegistryError> {
        self.sources
            .get(identity)
            .ok_or(RegistryError::SourceNotFound)
    }

    /// Returns one exact membership record.
    pub fn membership(&self, key: &MembershipKey) -> Result<&MembershipRecord, RegistryError> {
        self.memberships
            .get(key)
            .ok_or(RegistryError::MembershipNotFound)
    }

    /// Resolves one exact membership key by reverse membership identity.
    pub fn membership_key_by_id(
        &self,
        membership_id: search_contracts::SourceMembershipId,
    ) -> Result<&MembershipKey, RegistryError> {
        self.reverse_memberships
            .get(&membership_id)
            .ok_or(RegistryError::MembershipNotFound)
    }

    /// Returns one exact root record.
    pub fn root(
        &self,
        root_binding_id: search_contracts::RootBindingId,
    ) -> Result<&RootRecord, RegistryError> {
        self.roots
            .get(&root_binding_id)
            .ok_or(RegistryError::RootNotRegistered)
    }

    /// Mutable memberships for normative membership operations.
    pub(crate) fn memberships_mut(&mut self) -> &mut BTreeMap<MembershipKey, MembershipRecord> {
        &mut self.memberships
    }

    /// Mutable namespace cutover states.
    pub(crate) fn namespaces_mut(
        &mut self,
    ) -> &mut BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState> {
        &mut self.namespaces
    }

    /// Immutable roots.
    #[must_use]
    pub const fn roots(&self) -> &BTreeMap<search_contracts::RootBindingId, RootRecord> {
        &self.roots
    }

    /// Immutable sources.
    #[must_use]
    pub const fn sources(&self) -> &BTreeMap<SourceIdentity, RegisteredSource> {
        &self.sources
    }

    /// Immutable memberships.
    #[must_use]
    pub const fn memberships(&self) -> &BTreeMap<MembershipKey, MembershipRecord> {
        &self.memberships
    }

    /// Immutable reverse membership index.
    #[must_use]
    pub const fn reverse_memberships(
        &self,
    ) -> &BTreeMap<search_contracts::SourceMembershipId, MembershipKey> {
        &self.reverse_memberships
    }

    /// Immutable portfolios.
    #[must_use]
    pub const fn portfolios(&self) -> &BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord> {
        &self.portfolios
    }

    /// Immutable portfolio history with every retained revision and pointer.
    ///
    /// Historical revisions stay readable after any pointer move, so an old
    /// query or handle keeps its exact interpretation.
    #[must_use]
    pub const fn portfolio_history(&self) -> &PortfolioHistory {
        &self.portfolio_history
    }

    /// Immutable namespace states.
    #[must_use]
    pub const fn namespaces(
        &self,
    ) -> &BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState> {
        &self.namespaces
    }

    /// Immutable active generations snapshot for view resolution.
    #[must_use]
    pub const fn active_generations_snapshot(&self) -> &BTreeMap<OpaqueId, NonZeroRevision> {
        &self.active_generations
    }

    /// Sets the registry revision after one durable normative mutation.
    pub(crate) fn set_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    /// Split mutable access to roots and sources for admission.
    pub(crate) fn roots_and_sources_mut(
        &mut self,
    ) -> (
        &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
        &mut BTreeMap<SourceIdentity, RegisteredSource>,
    ) {
        (&mut self.roots, &mut self.sources)
    }

    /// Split mutable access for membership binding.
    #[allow(clippy::type_complexity)]
    pub(crate) fn membership_parts_mut(
        &mut self,
    ) -> (
        &BTreeMap<SourceIdentity, RegisteredSource>,
        &mut BTreeMap<MembershipKey, MembershipRecord>,
        &mut BTreeMap<search_contracts::SourceMembershipId, MembershipKey>,
        &mut BTreeMap<OpaqueId, NonZeroRevision>,
    ) {
        (
            &self.sources,
            &mut self.memberships,
            &mut self.reverse_memberships,
            &mut self.active_generations,
        )
    }

    /// Split mutable access for portfolio publication.
    #[allow(clippy::type_complexity)]
    pub(crate) fn portfolio_parts_mut(
        &mut self,
    ) -> (
        &BTreeMap<SourceIdentity, RegisteredSource>,
        &BTreeMap<MembershipKey, MembershipRecord>,
        &BTreeMap<search_contracts::SourceMembershipId, MembershipKey>,
        &mut BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
        &mut PortfolioHistory,
    ) {
        (
            &self.sources,
            &self.memberships,
            &self.reverse_memberships,
            &mut self.portfolios,
            &mut self.portfolio_history,
        )
    }

    /// Current active generation for a corpus.
    pub fn active_generation(&self, corpus_id: &OpaqueId) -> Option<NonZeroRevision> {
        self.active_generations.get(corpus_id).copied()
    }

    /// Applies a pure process-local batch or returns its process-local replay receipt.
    ///
    /// This compatibility path provides no persistence evidence. Product batches
    /// use [`crate::api::apply_admission_batch`] and its atomic state transaction.
    pub fn apply(&mut self, batch: RegistryBatch) -> Result<RegistryReceipt, RegistryError> {
        if self.pending_batch.is_some() {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        batch.validate(self.limits)?;
        if let Some(operation) = self.operations.get(batch.operation.operation_id()) {
            if operation.digest() != batch.operation.mutation_digest() {
                return Err(RegistryError::OperationConflict);
            }
            if let RetainedRegistryOperation::Committed(record) = operation
                && record.request_fingerprint() != batch::request_fingerprint(&batch)?
            {
                return Err(RegistryError::OperationConflict);
            }
            return Ok(operation.receipt(true));
        }
        if self.operations.len() >= self.limits.max_operations {
            return Err(RegistryError::OperationCapacityExceeded);
        }
        if batch.expected_registry_revision != self.revision {
            return Err(RegistryError::RegistryRevisionConflict);
        }
        let after_revision = self
            .revision
            .checked_add(1)
            .ok_or(RegistryError::RegistryRevisionOverflow)?;

        let mut staged_sources = self.sources.clone();
        let mut staged_roots = self.roots.clone();
        let mut staged_memberships = self.memberships.clone();
        let mut staged_generations = self.active_generations.clone();
        for change in &batch.changes {
            apply_change(
                change,
                after_revision,
                self.limits,
                &mut staged_roots,
                &mut staged_sources,
                &mut staged_memberships,
                &mut staged_generations,
            )?;
        }
        if staged_sources.len() > self.limits.max_sources
            || staged_memberships.len() > self.limits.max_memberships
        {
            return Err(RegistryError::CapacityExceeded);
        }

        let receipt = RegistryReceipt {
            operation: batch.operation.clone(),
            before_revision: self.revision,
            after_revision,
            change_count: batch.changes.len(),
            mutation_digest: batch.operation.mutation_digest(),
            replayed: false,
        };
        self.sources = staged_sources;
        self.roots = staged_roots;
        self.memberships = staged_memberships;
        self.active_generations = staged_generations;
        self.rebuild_reverse_index();
        self.revision = after_revision;
        self.operations.insert(
            batch.operation.operation_id().clone(),
            RetainedRegistryOperation::ProcessLocal(receipt.clone()),
        );
        self.identity_cache_v2 = None;
        Ok(receipt)
    }

    /// Returns the active admitted portfolio for one exact corpus generation.
    pub fn active_portfolio(
        &self,
        corpus_id: &OpaqueId,
        generation: NonZeroRevision,
        max_items: usize,
    ) -> Result<Vec<PortfolioItem>, RegistryError> {
        crate::portfolio::active_portfolio_for_generation(
            &self.memberships,
            &self.sources,
            &self.active_generations,
            corpus_id,
            generation,
            max_items,
            self.limits,
        )
    }

    fn rebuild_reverse_index(&mut self) {
        self.reverse_memberships.clear();
        for (key, record) in &self.memberships {
            self.reverse_memberships
                .insert(record.membership_id(), key.clone());
        }
    }
}

/// Classifies a batch commit outcome by atomic operation and target readback.
///
/// This read-only classification installs no state. The public batch producer
/// performs guarded recovery installation; legacy receipt-only entries cannot
/// establish a committed batch.
pub fn recover_registry_mutation<C, P>(
    registry: &SourceRegistry,
    operation_id: &OpaqueId,
    expected_digest: Blake3Digest32,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<RegistryMutationRecovery, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let lookup = control_port
        .load_batch(operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?;
    match lookup {
        RegistryBatchLookup::Absent { revision } => {
            if revision != registry.revision || registry.operations.contains_key(operation_id) {
                Ok(RegistryMutationRecovery::Quarantined)
            } else {
                Ok(RegistryMutationRecovery::RetrySameOperation)
            }
        }
        RegistryBatchLookup::Committed(readback) => {
            let record = readback.record();
            if record.entry().operation_id != *operation_id
                || record.entry().mutation_digest != expected_digest
            {
                return Ok(RegistryMutationRecovery::Conflict);
            }
            if let Some((id, fingerprint)) = &registry.pending_batch
                && (id != operation_id || *fingerprint != record.request_fingerprint())
            {
                return Ok(RegistryMutationRecovery::Conflict);
            }
            if readback.verify_operation().is_err() {
                return Ok(RegistryMutationRecovery::Quarantined);
            }
            // Acknowledged historical operations remain immutable even when
            // later transactions legitimately replace their target states.
            let historical = registry.pending_batch.is_none()
                && registry.revision == readback.revision()
                && registry.revision >= record.entry().after_revision;
            if !historical
                && (readback.verify().is_err()
                    || batch::validate_install(registry, record).is_err())
            {
                return Ok(RegistryMutationRecovery::Quarantined);
            }
            Ok(RegistryMutationRecovery::Recovered(
                record.registry_receipt(true),
            ))
        }
    }
}

/// Processes finite canonical items and commits one outcome per source.
///
/// Cancellation before commit is clean; after a possible commit, recovery uses
/// the batch operation identity. Partial per-item business outcomes are
/// explicit and never collapsed to total success.
pub fn apply_admission_batch<C, P>(
    registry: &mut SourceRegistry,
    batch: RegistryBatch,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RegistryBatchReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    batch.validate(registry.limits)?;
    let fingerprint = batch::request_fingerprint(&batch)?;
    let operation_id = batch.operation.operation_id();
    if let Some((pending_id, pending_fingerprint)) = &registry.pending_batch
        && (pending_id != operation_id || *pending_fingerprint != fingerprint)
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    match control_port
        .load_batch(operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    {
        RegistryBatchLookup::Committed(readback) => {
            let record = readback.record();
            if record.entry().operation_id != *operation_id
                || record.entry().mutation_digest != batch.operation.mutation_digest()
                || record.request_fingerprint() != fingerprint
            {
                return Err(RegistryError::OperationConflict);
            }
            readback.verify_operation()?;
            if registry.pending_batch.is_none()
                && registry.revision == readback.revision()
                && registry.revision >= record.entry().after_revision
            {
                // Historical replay acknowledges the original transaction;
                // it never restores obsolete targets or advances memory.
                return Ok(record.receipt(true));
            }
            readback.verify()?;
            if registry.revision == record.entry().before_revision {
                let expected = batch::prepare_observed(registry, &batch, record)?;
                if expected.record() != record {
                    return Err(RegistryError::MutationOutcomeUnknown);
                }
            }
            batch::install(registry, record)?;
            return Ok(record.receipt(true));
        }
        RegistryBatchLookup::Absent { revision } => {
            if revision != registry.revision {
                return Err(RegistryError::RegistryRevisionConflict);
            }
            if registry.operations.contains_key(operation_id) {
                // A process-local receipt is not atomic-state evidence.
                return Err(RegistryError::MutationOutcomeUnknown);
            }
            registry.pending_batch = None;
        }
    }
    let prepared = match batch::prepare(registry, &batch, receipt) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Ok(RegistryBatchReceipt {
                operation: batch.operation,
                before_revision: registry.revision,
                after_revision: registry.revision,
                items: (0..batch.changes.len())
                    .map(|input_index| BatchItemOutcome {
                        input_index,
                        status: BatchItemStatus::Rejected,
                        reason: Some(error),
                    })
                    .collect(),
                receipt: receipt.clone(),
                replayed: false,
            });
        }
    };
    // All semantic checks and encoding precede the single external mutation.
    batch::validate_install(registry, prepared.record())?;
    let mutation = registry_mutation(operation_id);
    let readback = match control_port.commit_batch(&prepared, context, &mutation) {
        Ok(readback) => readback,
        Err(error) => {
            let refusal = match error.kind() {
                PortErrorKind::CancelledBeforeSideEffect => {
                    Some(RegistryError::CancelledBeforeCommit)
                }
                PortErrorKind::CompareAndSwapRejected | PortErrorKind::StaleGeneration => {
                    Some(RegistryError::RegistryRevisionConflict)
                }
                PortErrorKind::Conflict => Some(RegistryError::OperationConflict),
                PortErrorKind::ResourceExhausted => Some(RegistryError::DurabilityExhausted),
                PortErrorKind::InvalidContext
                | PortErrorKind::DeadlineBeforeStart
                | PortErrorKind::InvalidInput
                | PortErrorKind::Unauthorized => Some(RegistryError::DurabilityRejected),
                _ => None,
            };
            if let Some(refusal) = refusal {
                return Err(refusal);
            }
            registry.pending_batch = Some((operation_id.clone(), fingerprint));
            return Err(RegistryError::MutationOutcomeUnknown);
        }
    };
    if readback.record() != prepared.record() || readback.verify().is_err() {
        registry.pending_batch = Some((operation_id.clone(), fingerprint));
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    batch::install_prepared(registry, &prepared);
    Ok(readback.record().receipt(false))
}

fn apply_change(
    change: &RegistryChange,
    registry_revision: u64,
    limits: RegistryLimits,
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    match change {
        RegistryChange::RegisterRoot { request, receipt } => {
            crate::root::stage_registration(roots, request, registry_revision, receipt, limits)
        }
        RegistryChange::UpdateRootPolicy {
            root_binding_id,
            expected_record_revision,
            policy_fingerprint,
            policy_revision,
            receipt,
        } => crate::root::stage_policy(
            roots,
            *root_binding_id,
            *expected_record_revision,
            *policy_fingerprint,
            *policy_revision,
            registry_revision,
            receipt,
        ),
        RegistryChange::UnbindRoot {
            root_binding_id,
            expected_record_revision,
            receipt,
        } => crate::root::stage_unbind(
            roots,
            *root_binding_id,
            *expected_record_revision,
            registry_revision,
            receipt,
        ),
        RegistryChange::RegisterSource {
            admission,
            original_admission,
            binding,
            assignment,
            receipt,
        } => register_source(
            admission,
            original_admission.as_ref(),
            binding,
            assignment,
            receipt,
            registry_revision,
            sources,
        ),
        RegistryChange::UpdateSourceBinding {
            identity,
            expected_source_revision,
            binding,
            receipt,
        } => update_source_binding(
            identity,
            *expected_source_revision,
            binding,
            receipt,
            registry_revision,
            sources,
        ),
        RegistryChange::RetireSource {
            identity,
            expected_source_revision,
            receipt,
        } => retire_source(
            identity,
            *expected_source_revision,
            receipt,
            registry_revision,
            sources,
            memberships,
        ),
        RegistryChange::AddMembership(membership) => add_membership(
            membership,
            registry_revision,
            sources,
            memberships,
            active_generations,
        ),
        RegistryChange::RetireMembership {
            key,
            expected_membership_revision,
            receipt,
        } => retire_membership(
            key,
            *expected_membership_revision,
            receipt,
            registry_revision,
            memberships,
        ),
        RegistryChange::CutoverNamespace(cutover) => apply_cutover(
            cutover,
            registry_revision,
            limits,
            sources,
            memberships,
            active_generations,
        ),
    }
}

fn register_source(
    admission: &AdmissionReceipt,
    original: Option<&search_source_admission::original_envelope::OriginalAdmissionEnvelope>,
    binding: &SourceBinding,
    assignment: &AdmissionBindingProof,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
) -> Result<(), RegistryError> {
    use search_source_admission::AdmissionOutcome;
    if sources.contains_key(binding.identity()) {
        return Err(RegistryError::SourceAlreadyRegistered);
    }
    if admission.outcome() != AdmissionOutcome::Allow {
        return Err(RegistryError::AdmissionBindingMismatch);
    }
    if assignment.observation_digest.as_bytes() != admission.observation_digest().as_bytes()
        || binding.identity() != &assignment.source_identity
    {
        return Err(RegistryError::AdmissionBindingMismatch);
    }
    if !assignment.readback_verified {
        return Err(RegistryError::AdmissionBindingEvidenceMissing);
    }
    let source_revision = NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    sources.insert(
        binding.identity().clone(),
        RegisteredSource::new(
            binding.clone(),
            admission.clone(),
            assignment.clone(),
            SourceLifecycle::Active,
            source_revision,
            registry_revision,
            receipt.clone(),
        )
        .retaining_original(original.cloned())?,
    );
    Ok(())
}

fn update_source_binding(
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    binding: &SourceBinding,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
) -> Result<(), RegistryError> {
    let source = sources
        .get_mut(identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle() != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if source.source_revision() != expected_source_revision {
        return Err(RegistryError::SourceRevisionConflict);
    }
    if binding.identity() != identity {
        return Err(RegistryError::SourceBindingConflict);
    }
    let next_revision = source
        .source_revision()
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let updated = RegisteredSource::new(
        binding.clone(),
        source
            .legacy_admission()
            .ok_or(RegistryError::AdmissionReceiptMismatch)?
            .clone(),
        source
            .legacy_assignment()
            .ok_or(RegistryError::AdmissionReceiptMismatch)?
            .clone(),
        SourceLifecycle::Active,
        next_revision,
        registry_revision,
        receipt.clone(),
    )
    .retaining_original(source.original_admission().cloned())?;
    sources.insert(identity.clone(), updated);
    Ok(())
}

fn retire_source(
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
) -> Result<(), RegistryError> {
    let source = sources.get(identity).ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle() != SourceLifecycle::Active
        || source.source_revision() != expected_source_revision
    {
        return Err(RegistryError::SourceRevisionConflict);
    }
    let next_revision = source
        .source_revision()
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let retired = source
        .clone()
        .into_retired(next_revision, registry_revision, receipt.clone());
    sources.insert(identity.clone(), retired);
    let to_retire: Vec<MembershipKey> = memberships
        .iter()
        .filter(|(key, record)| {
            &key.source_identity == identity && record.lifecycle() == MembershipLifecycle::Active
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in to_retire {
        let Some(record) = memberships.get(&key).cloned() else {
            continue;
        };
        let next_membership = record
            .membership_revision()
            .checked_next()
            .map_err(|_| RegistryError::ContractExhausted)?;
        let retired_membership = MembershipRecord::new(
            record.key().clone(),
            record.membership_id(),
            record.generation(),
            next_membership,
            MembershipLifecycle::Retired,
            registry_revision,
            receipt.clone(),
        );
        memberships.insert(key, retired_membership);
    }
    Ok(())
}

fn add_membership(
    membership: &NewMembership,
    registry_revision: u64,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    let source = sources
        .get(&membership.key.source_identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle() != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if memberships.contains_key(&membership.key) {
        return Err(RegistryError::MembershipCollision);
    }
    if let Some(active) = active_generations.get(&membership.key.corpus_id) {
        if *active != membership.generation {
            return Err(RegistryError::CutoverGenerationConflict);
        }
    } else {
        active_generations.insert(membership.key.corpus_id.clone(), membership.generation);
    }
    let membership_id = derive_membership_id(&membership.key);
    memberships.insert(
        membership.key.clone(),
        MembershipRecord::new(
            membership.key.clone(),
            membership_id,
            membership.generation,
            NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?,
            MembershipLifecycle::Active,
            registry_revision,
            membership.receipt.clone(),
        ),
    );
    Ok(())
}

fn retire_membership(
    key: &MembershipKey,
    expected_membership_revision: NonZeroRevision,
    receipt: &ReceiptRef,
    registry_revision: u64,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
) -> Result<(), RegistryError> {
    let membership = memberships
        .get(key)
        .ok_or(RegistryError::MembershipNotFound)?;
    if membership.lifecycle() != MembershipLifecycle::Active
        || membership.membership_revision() != expected_membership_revision
    {
        return Err(RegistryError::MembershipRevisionConflict);
    }
    let next_revision = membership
        .membership_revision()
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    let retired = MembershipRecord::new(
        membership.key().clone(),
        membership.membership_id(),
        membership.generation(),
        next_revision,
        MembershipLifecycle::Retired,
        registry_revision,
        receipt.clone(),
    );
    memberships.insert(key.clone(), retired);
    Ok(())
}

fn apply_cutover(
    cutover: &NamespaceCutover,
    registry_revision: u64,
    limits: RegistryLimits,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    if !cutover.authorization_verified || !cutover.readback_verified {
        return Err(RegistryError::CutoverEvidenceMissing);
    }
    if cutover.frozen_inventory.is_empty()
        || cutover.frozen_inventory.len() > limits.max_cutover_inventory
    {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    let inventory = cutover
        .frozen_inventory
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if inventory.len() != cutover.frozen_inventory.len() {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    if active_generations.get(&cutover.corpus_id).copied() != cutover.expected_generation {
        return Err(RegistryError::CutoverGenerationConflict);
    }
    if let Some(current) = cutover.expected_generation
        && cutover.next_generation <= current
    {
        return Err(RegistryError::CutoverGenerationConflict);
    }
    for identity in &inventory {
        let source = sources
            .get(identity)
            .ok_or(RegistryError::CutoverSourceUnavailable)?;
        if source.lifecycle() != SourceLifecycle::Active {
            return Err(RegistryError::CutoverSourceUnavailable);
        }
    }

    let to_retire: Vec<MembershipKey> = memberships
        .keys()
        .filter(|key| key.corpus_id == cutover.corpus_id)
        .cloned()
        .collect();
    for key in to_retire {
        let Some(record) = memberships.get(&key).cloned() else {
            continue;
        };
        let next_revision = record
            .membership_revision()
            .checked_next()
            .map_err(|_| RegistryError::ContractExhausted)?;
        let retired = MembershipRecord::new(
            record.key().clone(),
            record.membership_id(),
            record.generation(),
            next_revision,
            MembershipLifecycle::Retired,
            registry_revision,
            cutover.receipt.clone(),
        );
        memberships.insert(key, retired);
    }

    for identity in inventory {
        let key = MembershipKey {
            corpus_id: cutover.corpus_id.clone(),
            source_identity: identity,
        };
        if let Some(membership) = memberships.get(&key).cloned() {
            let reactivated = MembershipRecord::new(
                key.clone(),
                membership.membership_id(),
                cutover.next_generation,
                membership
                    .membership_revision()
                    .checked_next()
                    .map_err(|_| RegistryError::ContractExhausted)?,
                MembershipLifecycle::Active,
                registry_revision,
                cutover.receipt.clone(),
            );
            memberships.insert(key, reactivated);
        } else {
            if memberships.len() >= limits.max_memberships {
                return Err(RegistryError::CapacityExceeded);
            }
            let membership_id = derive_membership_id(&key);
            memberships.insert(
                key.clone(),
                MembershipRecord::new(
                    key,
                    membership_id,
                    cutover.next_generation,
                    NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?,
                    MembershipLifecycle::Active,
                    registry_revision,
                    cutover.receipt.clone(),
                ),
            );
        }
    }
    active_generations.insert(cutover.corpus_id.clone(), cutover.next_generation);
    Ok(())
}
