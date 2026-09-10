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
use search_ports::{CancellationProbe, OperationContext};
use search_source_admission::AdmissionReceipt;
use search_source_identity::SourceBinding;

use crate::cutover::NamespaceCutoverState;
use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};
use crate::membership::{
    MembershipKey, MembershipLifecycle, MembershipRecord, NewMembership, derive_membership_id,
};
use crate::portfolio::{PortfolioItem, ReferencePortfolioRecord};
use crate::root::RootRecord;
use crate::snapshot::ValidatedRegistrySnapshot;
use crate::source::{AdmissionBindingProof, RegisteredSource, SourceLifecycle};

/// Full-payload immutable registry operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryOperation {
    operation_id: OpaqueId,
    mutation_digest: Blake3Digest32,
}

impl RegistryOperation {
    /// Creates an operation from immutable identity and digest of the complete
    /// canonical batch payload.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, mutation_digest: Blake3Digest32) -> Self {
        Self {
            operation_id,
            mutation_digest,
        }
    }

    /// Immutable operation identity.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Digest of the complete canonical batch payload.
    pub const fn mutation_digest(&self) -> Blake3Digest32 {
        self.mutation_digest
    }
}

/// Exact namespace cutover request (legacy atomic path).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceCutover {
    /// Corpus whose active inventory changes atomically.
    pub corpus_id: OpaqueId,
    /// Exact current active generation.
    pub expected_generation: Option<NonZeroRevision>,
    /// Strictly newer generation.
    pub next_generation: NonZeroRevision,
    /// Complete frozen active inventory in canonical source-identity order.
    pub frozen_inventory: Vec<SourceIdentity>,
    /// Digest of exact canonical frozen inventory.
    pub inventory_digest: Blake3Digest32,
    /// Whether mutation authorization was verified.
    pub authorization_verified: bool,
    /// Whether authoritative post-cutover readback is required and available.
    pub readback_verified: bool,
    /// Content-free cutover receipt.
    pub receipt: ReceiptRef,
}

/// One source-registry change inside an atomic batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryChange {
    /// Register a newly assigned admitted source.
    RegisterSource {
        /// Verified admission receipt with `ALLOW` outcome.
        admission: AdmissionReceipt,
        /// First current source binding.
        binding: SourceBinding,
        /// Observation-to-stable-identity assignment proof.
        assignment: AdmissionBindingProof,
        /// Content-free registration receipt.
        receipt: ReceiptRef,
    },
    /// Replace only the current source binding under exact source revision.
    UpdateSourceBinding {
        /// Stable source identity.
        identity: SourceIdentity,
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Exact replacement source binding.
        binding: SourceBinding,
        /// Content-free update receipt.
        receipt: ReceiptRef,
    },
    /// Retire one active source.
    RetireSource {
        /// Stable source identity.
        identity: SourceIdentity,
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Content-free retirement receipt.
        receipt: ReceiptRef,
    },
    /// Add one source/corpus membership.
    AddMembership(NewMembership),
    /// Retire one active source/corpus membership.
    RetireMembership {
        /// Exact membership key.
        key: MembershipKey,
        /// Expected current membership revision.
        expected_membership_revision: NonZeroRevision,
        /// Content-free retirement receipt.
        receipt: ReceiptRef,
    },
    /// Atomically replace one corpus active inventory and generation.
    CutoverNamespace(NamespaceCutover),
}

impl RegistryChange {
    fn source_target(&self) -> Option<&SourceIdentity> {
        match self {
            Self::RegisterSource { binding, .. } => Some(binding.identity()),
            Self::UpdateSourceBinding { identity, .. } | Self::RetireSource { identity, .. } => {
                Some(identity)
            }
            Self::AddMembership(_) | Self::RetireMembership { .. } | Self::CutoverNamespace(_) => {
                None
            }
        }
    }

    fn membership_target(&self) -> Option<&MembershipKey> {
        match self {
            Self::AddMembership(membership) => Some(&membership.key),
            Self::RetireMembership { key, .. } => Some(key),
            Self::RegisterSource { .. }
            | Self::UpdateSourceBinding { .. }
            | Self::RetireSource { .. }
            | Self::CutoverNamespace(_) => None,
        }
    }

    fn cutover_target(&self) -> Option<&OpaqueId> {
        match self {
            Self::CutoverNamespace(cutover) => Some(&cutover.corpus_id),
            _ => None,
        }
    }
}

/// Finite exact-revision guarded atomic registry batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryBatch {
    /// Exact registry revision required before application.
    pub expected_registry_revision: u64,
    /// Full-payload immutable operation.
    pub operation: RegistryOperation,
    /// Finite ordered changes.
    pub changes: Vec<RegistryChange>,
}

impl RegistryBatch {
    /// Validates finite size and duplicate mutation targets.
    pub fn validate(&self, limits: RegistryLimits) -> Result<(), RegistryError> {
        let limits = limits.validate()?;
        if self.changes.is_empty() || self.changes.len() > limits.max_batch_changes {
            return Err(RegistryError::BatchSizeInvalid);
        }
        let mut sources = BTreeSet::new();
        let mut memberships = BTreeSet::new();
        let mut cutovers = BTreeSet::new();
        for change in &self.changes {
            if let Some(source) = change.source_target()
                && !sources.insert(source.clone())
            {
                return Err(RegistryError::DuplicateBatchTarget);
            }
            if let Some(membership) = change.membership_target()
                && !memberships.insert(membership.clone())
            {
                return Err(RegistryError::DuplicateBatchTarget);
            }
            if let Some(corpus) = change.cutover_target() {
                if !cutovers.insert(corpus.clone()) {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
                if memberships
                    .iter()
                    .any(|membership| &membership.corpus_id == corpus)
                {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
            }
        }
        for membership in &memberships {
            if cutovers.contains(&membership.corpus_id) {
                return Err(RegistryError::DuplicateBatchTarget);
            }
        }
        Ok(())
    }
}

/// Content-free atomic registry receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryReceipt {
    /// Full-payload operation.
    pub operation: RegistryOperation,
    /// Registry revision before the atomic batch.
    pub before_revision: u64,
    /// Registry revision after the atomic batch.
    pub after_revision: u64,
    /// Number of committed changes.
    pub change_count: usize,
    /// Digest of complete canonical batch payload.
    pub mutation_digest: Blake3Digest32,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Closed per-item batch outcome; partial outcomes stay explicit.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BatchItemStatus {
    /// Item committed atomically with the batch.
    Committed,
    /// Item was rejected before commit with a typed reason.
    Rejected,
}

/// One per-input batch outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchItemOutcome {
    /// Zero-based input index.
    pub input_index: usize,
    /// Item status.
    pub status: BatchItemStatus,
    /// Typed reason code for rejected items.
    pub reason: Option<RegistryError>,
}

/// Content-free admission batch receipt with one outcome per input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryBatchReceipt {
    /// Full-payload operation.
    pub operation: RegistryOperation,
    /// Registry revision before the batch.
    pub before_revision: u64,
    /// Registry revision after the batch.
    pub after_revision: u64,
    /// One outcome per input in canonical order.
    pub items: Vec<BatchItemOutcome>,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Closed registry-mutation recovery decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryMutationRecovery {
    /// Exact prior receipt reconstructed by readback.
    Recovered(RegistryReceipt),
    /// Operation was never committed; same-operation retry is permitted.
    RetrySameOperation,
    /// Operation identity conflicts with a different payload.
    Conflict,
    /// Partial or contradictory state requires quarantine.
    Quarantined,
}

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
    namespaces: BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState>,
    active_generations: BTreeMap<OpaqueId, NonZeroRevision>,
    operations: Vec<(OpaqueId, Blake3Digest32, RegistryReceipt)>,
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
            namespaces: BTreeMap::new(),
            active_generations: BTreeMap::new(),
            operations: Vec::new(),
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

    /// Mutable roots for normative root operations.
    pub fn roots_mut(&mut self) -> &mut BTreeMap<search_contracts::RootBindingId, RootRecord> {
        &mut self.roots
    }

    /// Mutable sources for normative source operations.
    pub fn sources_mut(&mut self) -> &mut BTreeMap<SourceIdentity, RegisteredSource> {
        &mut self.sources
    }

    /// Mutable memberships for normative membership operations.
    pub fn memberships_mut(&mut self) -> &mut BTreeMap<MembershipKey, MembershipRecord> {
        &mut self.memberships
    }

    /// Mutable reverse membership index.
    pub fn reverse_memberships_mut(
        &mut self,
    ) -> &mut BTreeMap<search_contracts::SourceMembershipId, MembershipKey> {
        &mut self.reverse_memberships
    }

    /// Mutable portfolios for normative portfolio operations.
    pub fn portfolios_mut(
        &mut self,
    ) -> &mut BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord> {
        &mut self.portfolios
    }

    /// Mutable namespace cutover states.
    pub fn namespaces_mut(
        &mut self,
    ) -> &mut BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState> {
        &mut self.namespaces
    }

    /// Mutable active generations.
    pub fn active_generations_mut(&mut self) -> &mut BTreeMap<OpaqueId, NonZeroRevision> {
        &mut self.active_generations
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
    pub fn set_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    /// Split mutable access to roots and sources for admission.
    pub fn roots_and_sources_mut(
        &mut self,
    ) -> (
        &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
        &mut BTreeMap<SourceIdentity, RegisteredSource>,
    ) {
        (&mut self.roots, &mut self.sources)
    }

    /// Split mutable access for membership binding.
    #[allow(clippy::type_complexity)]
    pub fn membership_parts_mut(
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
    pub fn portfolio_parts_mut(
        &mut self,
    ) -> (
        &BTreeMap<SourceIdentity, RegisteredSource>,
        &BTreeMap<MembershipKey, MembershipRecord>,
        &BTreeMap<search_contracts::SourceMembershipId, MembershipKey>,
        &mut BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
    ) {
        (
            &self.sources,
            &self.memberships,
            &self.reverse_memberships,
            &mut self.portfolios,
        )
    }

    /// Advances the registry revision after one durable normative mutation.
    pub fn advance_revision(&mut self) -> Result<(u64, u64), RegistryError> {
        let before = self.revision;
        let after = before
            .checked_add(1)
            .ok_or(RegistryError::RegistryRevisionOverflow)?;
        self.revision = after;
        Ok((before, after))
    }

    /// Current active generation for a corpus.
    pub fn active_generation(&self, corpus_id: &OpaqueId) -> Option<NonZeroRevision> {
        self.active_generations.get(corpus_id).copied()
    }

    /// Applies one finite atomic batch or returns the exact prior replay receipt.
    pub fn apply(&mut self, batch: RegistryBatch) -> Result<RegistryReceipt, RegistryError> {
        batch.validate(self.limits)?;
        if let Some((_, digest, receipt)) = self
            .operations
            .iter()
            .find(|(operation_id, _, _)| operation_id == batch.operation.operation_id())
        {
            if *digest != batch.operation.mutation_digest() {
                return Err(RegistryError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(replay);
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
        let mut staged_memberships = self.memberships.clone();
        let mut staged_generations = self.active_generations.clone();
        for change in &batch.changes {
            apply_change(
                change,
                after_revision,
                self.limits,
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
        self.memberships = staged_memberships;
        self.active_generations = staged_generations;
        self.rebuild_reverse_index();
        self.revision = after_revision;
        self.operations.push((
            batch.operation.operation_id().clone(),
            batch.operation.mutation_digest(),
            receipt.clone(),
        ));
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

/// Resolves an unknown commit outcome by exact idempotency/entity/generation
/// readback.
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
    if let Some((_, digest, receipt)) = registry
        .operations
        .iter()
        .find(|(id, _, _)| id == operation_id)
    {
        if *digest != expected_digest {
            return Ok(RegistryMutationRecovery::Conflict);
        }
        return Ok(RegistryMutationRecovery::Recovered(receipt.clone()));
    }
    let journal = control_port
        .load_entry(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?;
    match journal {
        None => Ok(RegistryMutationRecovery::RetrySameOperation),
        Some(entry) => {
            if entry.mutation_digest != expected_digest {
                Ok(RegistryMutationRecovery::Conflict)
            } else if entry.after_revision != registry.revision.saturating_add(1) {
                Ok(RegistryMutationRecovery::Quarantined)
            } else {
                Ok(RegistryMutationRecovery::RetrySameOperation)
            }
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
    expected_snapshot: &ValidatedRegistrySnapshot,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RegistryBatchReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    if expected_snapshot.snapshot().revision != registry.revision {
        return Err(RegistryError::SourceViewStale);
    }
    if let Some((_, digest, _)) = registry
        .operations
        .iter()
        .find(|(id, _, _)| id == batch.operation.operation_id())
    {
        if *digest != batch.operation.mutation_digest() {
            return Err(RegistryError::OperationConflict);
        }
        let items = batch
            .changes
            .iter()
            .enumerate()
            .map(|(input_index, _)| BatchItemOutcome {
                input_index,
                status: BatchItemStatus::Committed,
                reason: None,
            })
            .collect();
        return Ok(RegistryBatchReceipt {
            operation: batch.operation.clone(),
            before_revision: registry.revision,
            after_revision: registry.revision,
            items,
            receipt: receipt.clone(),
            replayed: true,
        });
    }
    let before = registry.revision;
    match registry.apply(batch.clone()) {
        Ok(applied) => {
            let mutation = registry_mutation(batch.operation.operation_id());
            let entry = RegistryJournalEntry::new(
                batch.operation.operation_id().clone(),
                batch.operation.mutation_digest(),
                applied.before_revision,
                applied.after_revision,
                JournalEntryKind::BatchApply,
                receipt.clone(),
            );
            control_port
                .persist_entry(&entry, context, &mutation)
                .map_err(|_| RegistryError::DurabilityRejected)?;
            let items = batch
                .changes
                .iter()
                .enumerate()
                .map(|(input_index, _)| BatchItemOutcome {
                    input_index,
                    status: BatchItemStatus::Committed,
                    reason: None,
                })
                .collect();
            Ok(RegistryBatchReceipt {
                operation: applied.operation,
                before_revision: applied.before_revision,
                after_revision: applied.after_revision,
                items,
                receipt: receipt.clone(),
                replayed: false,
            })
        }
        Err(error) => {
            let _ = before;
            let RegistryBatch {
                operation, changes, ..
            } = batch;
            let items = changes
                .iter()
                .enumerate()
                .map(|(input_index, _)| BatchItemOutcome {
                    input_index,
                    status: BatchItemStatus::Rejected,
                    reason: Some(error),
                })
                .collect();
            Ok(RegistryBatchReceipt {
                operation,
                before_revision: registry.revision,
                after_revision: registry.revision,
                items,
                receipt: receipt.clone(),
                replayed: false,
            })
        }
    }
}

fn apply_change(
    change: &RegistryChange,
    registry_revision: u64,
    limits: RegistryLimits,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    match change {
        RegistryChange::RegisterSource {
            admission,
            binding,
            assignment,
            receipt,
        } => register_source(
            admission,
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
        ),
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
        source.admission().clone(),
        source.assignment().clone(),
        SourceLifecycle::Active,
        next_revision,
        registry_revision,
        receipt.clone(),
    );
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
    let retired = RegisteredSource::new(
        source.binding().clone(),
        source.admission().clone(),
        source.assignment().clone(),
        SourceLifecycle::Retired,
        next_revision,
        registry_revision,
        receipt.clone(),
    );
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
