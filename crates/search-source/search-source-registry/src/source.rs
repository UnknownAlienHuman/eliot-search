//! Admitted source persistence without reimplementing admission semantics.
//!
//! The registry stores verified admission receipts and never derives them.
//! Every durable mutation requires the exact current [`AdmissionReceipt`] with
//! an `ALLOW` outcome, checks it against the bound root fence and persists the
//! receipt through the vendor-neutral [`crate::error::RegistryControlPort`].
//! No source bytes, extracted text, vectors or Qdrant IDs are retained here.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity};
use search_ports::{CancellationProbe, MutationIdentity, OperationContext};
use search_source_admission::{AdmissionOutcome, AdmissionReceipt};
use search_source_identity::SourceBinding;

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};
use crate::root::{RootRecord, RootStatus};

/// Source lifecycle visible to portfolios.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceLifecycle {
    /// Source is active and may participate in admitted portfolios.
    Active,
    /// Source is retired and excluded from new portfolios.
    Retired,
}

/// Exact proof binding a verified admission observation to a
/// registry-assigned stable source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionBindingProof {
    /// Digest of the exact verified admission observation.
    pub observation_digest: Blake3Digest32,
    /// Registry-assigned stable source identity.
    pub source_identity: SourceIdentity,
    /// Digest of observation, stable identity, admission receipt and binding.
    pub assignment_digest: Blake3Digest32,
    /// Content-free assignment receipt.
    pub assignment_receipt: ReceiptRef,
    /// Whether exact authoritative assignment readback was verified.
    pub readback_verified: bool,
}

/// Registry-owned source record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredSource {
    binding: SourceBinding,
    admission: AdmissionReceipt,
    assignment: AdmissionBindingProof,
    lifecycle: SourceLifecycle,
    source_revision: NonZeroRevision,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl RegisteredSource {
    /// Creates a registry-owned source record from validated parts.
    #[must_use]
    pub const fn new(
        binding: SourceBinding,
        admission: AdmissionReceipt,
        assignment: AdmissionBindingProof,
        lifecycle: SourceLifecycle,
        source_revision: NonZeroRevision,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Self {
        Self {
            binding,
            admission,
            assignment,
            lifecycle,
            source_revision,
            registry_revision,
            last_receipt,
        }
    }

    /// Stable source identity.
    pub const fn identity(&self) -> &SourceIdentity {
        self.binding.identity()
    }

    /// Current source binding.
    pub const fn binding(&self) -> &SourceBinding {
        &self.binding
    }

    /// Verified admission receipt bound to this registration.
    pub const fn admission(&self) -> &AdmissionReceipt {
        &self.admission
    }

    /// Exact observation-to-source assignment proof.
    pub const fn assignment(&self) -> &AdmissionBindingProof {
        &self.assignment
    }

    /// Current lifecycle.
    pub const fn lifecycle(&self) -> SourceLifecycle {
        self.lifecycle
    }

    /// Monotone source-record revision.
    pub const fn source_revision(&self) -> NonZeroRevision {
        self.source_revision
    }

    /// Registry revision that last changed this record.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt for the last source-record mutation.
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }
}

/// Content-free admitted-source receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedSourceReceipt {
    /// Stable source identity.
    pub source_identity: SourceIdentity,
    /// Source-record revision after commit.
    pub source_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Closed revalidation obligation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRevalidationObligation {
    /// Downstream preparation/serving must reconcile under the new fence.
    ReconcileDownstream,
    /// Cached views using this source require invalidation review.
    InvalidateViews,
    /// Later preparation/serving is fenced until the new decision is honored.
    FenceServing,
}

/// Content-free source admission-update receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAdmissionUpdateReceipt {
    /// Stable source identity.
    pub source_identity: SourceIdentity,
    /// Source-record revision after commit.
    pub source_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Explicit downstream obligations.
    pub obligations: Vec<SourceRevalidationObligation>,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Checks that a receipt is an exact current `ALLOW` under the root fence.
fn check_allow_under_root(
    receipt: &AdmissionReceipt,
    root: &RootRecord,
) -> Result<(), RegistryError> {
    if receipt.outcome() != AdmissionOutcome::Allow {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    if receipt.policy_revision() != root.policy_revision {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    if receipt.policy_fingerprint().as_bytes() != root.policy_fingerprint.as_bytes() {
        return Err(RegistryError::AdmissionReceiptStale);
    }
    Ok(())
}

/// Admits one source under the exact current root/policy fence.
///
/// The commit creates technical registry state only; it does not read or
/// retain bytes and does not publish any projection.
#[allow(clippy::too_many_arguments)]
pub fn admit_source<C, P>(
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    roots: &BTreeMap<search_contracts::RootBindingId, RootRecord>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    identity: &SourceIdentity,
    binding: &SourceBinding,
    receipt_input: &AdmissionReceipt,
    assignment: &AdmissionBindingProof,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<AdmittedSourceReceipt, RegistryError>
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
        JournalEntryKind::SourceAdmission,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = sources
            .get(identity)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(AdmittedSourceReceipt {
            source_identity: identity.clone(),
            source_revision: record.source_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    let root_id = binding.root_binding_id();
    let root = roots
        .get(&root_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    if root.status != RootStatus::Active {
        return Err(RegistryError::RootNotRegistered);
    }
    if binding.identity() != identity || &assignment.source_identity != identity {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    check_allow_under_root(receipt_input, root)?;
    if assignment.observation_digest.as_bytes() != receipt_input.observation_digest().as_bytes() {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    if !assignment.readback_verified {
        return Err(RegistryError::AdmissionBindingEvidenceMissing);
    }
    if sources.contains_key(identity) {
        return Err(RegistryError::SourceAlreadyAdmittedConflict);
    }
    if sources.len() >= limits.max_sources {
        return Err(RegistryError::CapacityExceeded);
    }
    persist(control_port, &entry, context, &mutation)?;
    let source_revision = NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    sources.insert(
        identity.clone(),
        RegisteredSource {
            binding: binding.clone(),
            admission: receipt_input.clone(),
            assignment: assignment.clone(),
            lifecycle: SourceLifecycle::Active,
            source_revision,
            registry_revision: next_registry_revision,
            last_receipt: receipt.clone(),
        },
    );
    Ok(AdmittedSourceReceipt {
        source_identity: identity.clone(),
        source_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Records the exact current admission decision and policy fence for one
/// admitted source, with explicit downstream obligations.
///
/// A restrictive decision fences later preparation/serving through downstream
/// owners; it is never silently delayed until reindex.
#[allow(clippy::too_many_arguments)]
pub fn revalidate_admitted_source<C, P>(
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    roots: &BTreeMap<search_contracts::RootBindingId, RootRecord>,
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    current_receipt: &AdmissionReceipt,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceAdmissionUpdateReceipt, RegistryError>
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
        JournalEntryKind::SourceRevalidation,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = sources
            .get(identity)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(SourceAdmissionUpdateReceipt {
            source_identity: identity.clone(),
            source_revision: record.source_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            obligations: vec![
                SourceRevalidationObligation::ReconcileDownstream,
                SourceRevalidationObligation::InvalidateViews,
                SourceRevalidationObligation::FenceServing,
            ],
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    let record = sources
        .get_mut(identity)
        .ok_or(RegistryError::SourceNotAdmitted)?;
    if record.lifecycle != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if record.source_revision != expected_source_revision {
        return Err(RegistryError::SourceRevisionConflict);
    }
    if current_receipt.observation_digest().as_bytes()
        != record.assignment.observation_digest.as_bytes()
    {
        return Err(RegistryError::AdmissionReceiptMismatch);
    }
    let root_id = record.binding.root_binding_id();
    let root = roots
        .get(&root_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    check_allow_under_root(current_receipt, root)?;
    persist(control_port, &entry, context, &mutation)?;
    record.source_revision = record
        .source_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    record.admission.clone_from(current_receipt);
    record.registry_revision = next_registry_revision;
    record.last_receipt.clone_from(receipt);
    Ok(SourceAdmissionUpdateReceipt {
        source_identity: identity.clone(),
        source_revision: record.source_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        obligations: vec![
            SourceRevalidationObligation::ReconcileDownstream,
            SourceRevalidationObligation::InvalidateViews,
            SourceRevalidationObligation::FenceServing,
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
