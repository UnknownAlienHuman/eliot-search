//! Source/corpus membership with verified admission-receipt gating.
//!
//! One source may hold multiple explicit memberships; each membership is keyed
//! by the exact `(corpus, source)` pair and carries a stable reverse
//! [`search_contracts::SourceMembershipId`]. Membership creation requires an
//! admitted source plus the exact current verified [`AdmissionGrant`]; it can
//! never weaken admission.

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, MembershipRole, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity,
    SourceMembershipId,
};
use search_ports::{CancellationProbe, MutationIdentity, OperationContext};
use search_source_admission::{AdmissionOutcome, AdmissionReceipt};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};
use crate::source::{RegisteredSource, SourceLifecycle};

/// Stable source/corpus membership key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MembershipKey {
    /// Corpus identity.
    pub corpus_id: OpaqueId,
    /// Stable source identity.
    pub source_identity: SourceIdentity,
}

/// Membership lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MembershipLifecycle {
    /// Membership is active in its generation.
    Active,
    /// Membership is restricted but retained.
    Restricted,
    /// Membership is suspended but retained.
    Suspended,
    /// Membership is retired but retained as a tombstone.
    Retired,
    /// Membership is removed but retained as a tombstone; removal is not purge.
    Removed,
}

/// Registry-owned source/corpus membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipRecord {
    key: MembershipKey,
    membership_id: SourceMembershipId,
    generation: NonZeroRevision,
    membership_revision: NonZeroRevision,
    lifecycle: MembershipLifecycle,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl MembershipRecord {
    /// Creates a membership record from validated parts.
    #[must_use]
    pub const fn new(
        key: MembershipKey,
        membership_id: SourceMembershipId,
        generation: NonZeroRevision,
        membership_revision: NonZeroRevision,
        lifecycle: MembershipLifecycle,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Self {
        Self {
            key,
            membership_id,
            generation,
            membership_revision,
            lifecycle,
            registry_revision,
            last_receipt,
        }
    }

    /// Stable membership key.
    pub const fn key(&self) -> &MembershipKey {
        &self.key
    }

    /// Stable reverse membership identity.
    pub const fn membership_id(&self) -> SourceMembershipId {
        self.membership_id
    }

    /// Active or retired namespace generation.
    pub const fn generation(&self) -> NonZeroRevision {
        self.generation
    }

    /// Monotone membership revision.
    pub const fn membership_revision(&self) -> NonZeroRevision {
        self.membership_revision
    }

    /// Membership lifecycle.
    pub const fn lifecycle(&self) -> MembershipLifecycle {
        self.lifecycle
    }

    /// Registry revision that last changed the membership.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt for the last membership mutation.
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }
}

/// New membership request (legacy batch path; admission gating happens in
/// [`bind_membership`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMembership {
    /// Corpus/source key.
    pub key: MembershipKey,
    /// Active namespace generation.
    pub generation: NonZeroRevision,
    /// Initial content-free membership receipt.
    pub receipt: ReceiptRef,
}

/// Explicit membership policies required at bind time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipPolicies {
    /// Membership role.
    pub role: MembershipRole,
    /// Access policy binding.
    pub access_policy: OpaqueId,
    /// Scoring policy binding.
    pub scoring_policy: OpaqueId,
    /// Residency policy binding.
    pub residency_policy: OpaqueId,
    /// Corpus policy revision observed by the caller.
    pub corpus_policy_revision: NonZeroRevision,
}

/// Explicit bind-membership request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindMembershipRequest {
    /// Corpus/source key.
    pub key: MembershipKey,
    /// Active namespace generation.
    pub generation: NonZeroRevision,
    /// Explicit membership/role/access/scoring/residency policies.
    pub policies: MembershipPolicies,
}

/// Content-free membership bind receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMembershipReceipt {
    /// Stable membership key.
    pub key: MembershipKey,
    /// Stable reverse membership identity.
    pub membership_id: SourceMembershipId,
    /// Membership revision after commit.
    pub membership_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Closed membership transition command.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MembershipCommand {
    /// Activate a restricted/suspended membership.
    Activate,
    /// Restrict an active membership.
    Restrict,
    /// Suspend an active/restricted membership.
    Suspend,
    /// Retire an active/restricted/suspended membership.
    Retire,
    /// Remove a retired membership tombstone (never purge).
    Remove,
}

/// Closed downstream obligation emitted by a membership transition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MembershipObligation {
    /// Downstream security owner must reconcile.
    SecurityReconcile,
    /// Downstream publication owner must reconcile.
    PublicationReconcile,
    /// Downstream retention owner must reconcile.
    RetentionReconcile,
}

/// Content-free membership transition receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipTransitionReceipt {
    /// Stable membership key.
    pub key: MembershipKey,
    /// Membership revision after commit.
    pub membership_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Explicit downstream obligations.
    pub obligations: Vec<MembershipObligation>,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Derives a stable reverse membership identity deterministically.
#[must_use]
pub fn derive_membership_id(key: &MembershipKey) -> SourceMembershipId {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.corpus_id.as_str().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in key.source_identity.source_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in key.source_identity.source_namespace_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    bytes[8..].copy_from_slice(&(!hash).to_le_bytes());
    SourceMembershipId::from_bytes(bytes)
}

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
            membership_id: record.membership_id,
            membership_revision: record.membership_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            receipt: record.last_receipt.clone(),
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
    persist(control_port, &entry, context, &mutation)?;
    let membership_id = derive_membership_id(&request.key);
    if let Some(existing) = reverse_index.get(&membership_id)
        && existing != &request.key
    {
        return Err(RegistryError::MembershipConflict);
    }
    let membership_revision =
        NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    memberships.insert(
        request.key.clone(),
        MembershipRecord {
            key: request.key.clone(),
            membership_id,
            generation: request.generation,
            membership_revision,
            lifecycle: MembershipLifecycle::Active,
            registry_revision: next_registry_revision,
            last_receipt: receipt.clone(),
        },
    );
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
            membership_revision: record.membership_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            obligations: vec![
                MembershipObligation::SecurityReconcile,
                MembershipObligation::PublicationReconcile,
                MembershipObligation::RetentionReconcile,
            ],
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    let record = memberships
        .get_mut(key)
        .ok_or(RegistryError::MembershipNotFound)?;
    if record.membership_revision != expected_membership_revision {
        return Err(RegistryError::MembershipGenerationMismatch);
    }
    let next_lifecycle = match (record.lifecycle, command) {
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
    persist(control_port, &entry, context, &mutation)?;
    record.membership_revision = record
        .membership_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    record.lifecycle = next_lifecycle;
    record.registry_revision = next_registry_revision;
    record.last_receipt.clone_from(receipt);
    Ok(MembershipTransitionReceipt {
        key: key.clone(),
        membership_revision: record.membership_revision,
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
