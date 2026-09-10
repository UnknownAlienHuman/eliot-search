//! Admitted root registration bound to a versioned admission-policy fingerprint.
//!
//! Roots are explicit records with no implicit recursive source admission.
//! Every mutation uses a stable operation identity, an expected registry
//! revision guard and durable receipt persistence through the vendor-neutral
//! [`crate::error::RegistryControlPort`]. No filesystem, redb or concrete
//! store handle is touched here.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef};
use search_ports::{CancellationProbe, MutationIdentity, OperationContext, PortErrorKind};

use crate::error::{
    JournalEntryKind, RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits,
    cancelled_before_commit, registry_mutation,
};

/// Lifecycle of one admitted root record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootStatus {
    /// Root accepts new admission/ownership work.
    Active,
    /// Root is fenced; new admission/ownership is denied, explicit
    /// invalidation work remains.
    Unbound,
}

/// Registry-owned admitted root record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootRecord {
    /// Stable admitted root binding identity.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Digest of the exact canonical root identity.
    pub canonical_root_digest: Blake3Digest32,
    /// Versioned admission-policy fingerprint bound at registration.
    pub policy_fingerprint: Blake3Digest32,
    /// Admission-policy revision bound to this root.
    pub policy_revision: NonZeroRevision,
    /// Owner epoch fence observed at registration.
    pub owner_epoch: OwnerEpoch,
    /// Monotone root-record revision.
    pub record_revision: NonZeroRevision,
    /// Registry revision that last changed this record.
    pub registry_revision: u64,
    /// Current lifecycle.
    pub status: RootStatus,
    /// Content-free receipt of the last root mutation.
    pub last_receipt: ReceiptRef,
}

/// Explicit root registration request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterRootRequest {
    /// Resolved local root identity.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Digest of the exact canonical root identity.
    pub canonical_root_digest: Blake3Digest32,
    /// Validated policy fingerprint.
    pub policy_fingerprint: Blake3Digest32,
    /// Validated policy revision.
    pub policy_revision: NonZeroRevision,
    /// Current data-root owner epoch fence.
    pub owner_epoch: OwnerEpoch,
    /// Content-free owner-fence verification receipt.
    pub owner_fence_receipt: ReceiptRef,
    /// Content-free policy-validation receipt.
    pub policy_receipt: ReceiptRef,
}

/// Content-free root registration receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootRegistrationReceipt {
    /// Admitted root binding.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Root-record revision after commit.
    pub record_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Closed root policy-change obligation emitted without reevaluating files.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootPolicyObligation {
    /// Downstream must reconcile under a more restrictive fence.
    ReconcileRestrictive,
    /// Downstream may reconcile under a more permissive fence.
    ReconcilePermissive,
    /// Source bindings under this root require invalidation review.
    InvalidateSources,
    /// Memberships under this root require invalidation review.
    InvalidateMemberships,
    /// Cached views under this root require invalidation review.
    InvalidateViews,
}

/// Content-free root policy-change receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPolicyChangeReceipt {
    /// Admitted root binding.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Root-record revision after commit.
    pub record_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Explicit reconciliation/invalidation obligations.
    pub obligations: Vec<RootPolicyObligation>,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Content-free root unbind receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootUnbindReceipt {
    /// Admitted root binding.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Root-record revision after commit.
    pub record_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Explicit invalidation work created by the fence.
    pub obligations: Vec<RootPolicyObligation>,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Registers one explicit root with no implicit recursive source admission.
///
/// Same operation plus same request is idempotent. A conflicting canonical
/// root or a reused operation identity with another payload is rejected.
#[allow(clippy::too_many_arguments)]
pub fn register_root<C, P>(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    request: &RegisterRootRequest,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootRegistrationReceipt, RegistryError>
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
        JournalEntryKind::RootRegistration,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = roots
            .get(&request.root_binding_id)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(RootRegistrationReceipt {
            root_binding_id: request.root_binding_id,
            record_revision: record.record_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    if roots.contains_key(&request.root_binding_id) {
        return Err(RegistryError::RootAlreadyRegistered);
    }
    for record in roots.values() {
        if record.canonical_root_digest == request.canonical_root_digest {
            return Err(RegistryError::RootIdentityConflict);
        }
    }
    if roots.len() >= limits.max_roots {
        return Err(RegistryError::CapacityExceeded);
    }
    persist(control_port, &entry, context, &mutation)?;
    let record_revision = NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    roots.insert(
        request.root_binding_id,
        RootRecord {
            root_binding_id: request.root_binding_id,
            canonical_root_digest: request.canonical_root_digest,
            policy_fingerprint: request.policy_fingerprint,
            policy_revision: request.policy_revision,
            owner_epoch: request.owner_epoch,
            record_revision,
            registry_revision: next_registry_revision,
            status: RootStatus::Active,
            last_receipt: receipt.clone(),
        },
    );
    Ok(RootRegistrationReceipt {
        root_binding_id: request.root_binding_id,
        record_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Commits a new policy fence without reevaluating files or silently
/// altering membership.
pub fn update_root_policy<C, P>(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    new_policy_fingerprint: Blake3Digest32,
    new_policy_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootPolicyChangeReceipt, RegistryError>
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
        JournalEntryKind::RootPolicyChange,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = roots
            .get(&root_binding_id)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(RootPolicyChangeReceipt {
            root_binding_id,
            record_revision: record.record_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            obligations: obligations_for(record.policy_revision, new_policy_revision),
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    let record = roots
        .get_mut(&root_binding_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    if record.status != RootStatus::Active {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    if record.record_revision != expected_record_revision {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    persist(control_port, &entry, context, &mutation)?;
    let obligations = obligations_for(record.policy_revision, new_policy_revision);
    record.record_revision = record
        .record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    record.policy_fingerprint = new_policy_fingerprint;
    record.policy_revision = new_policy_revision;
    record.registry_revision = next_registry_revision;
    record.last_receipt = receipt.clone();
    Ok(RootPolicyChangeReceipt {
        root_binding_id,
        record_revision: record.record_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        obligations,
        receipt: receipt.clone(),
        replayed: false,
    })
}

/// Fences new admission/ownership under the root and creates explicit
/// source/membership/view invalidation work. It never deletes revisions or
/// claims purge.
#[allow(clippy::too_many_arguments)]
pub fn unbind_root<C, P>(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
    expected_registry_revision: u64,
    next_registry_revision: u64,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootUnbindReceipt, RegistryError>
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
        JournalEntryKind::RootUnbind,
        receipt.clone(),
    );
    if let Some(existing) = load_existing(control_port, operation_id, context)? {
        if existing.mutation_digest != mutation_digest {
            return Err(RegistryError::OperationConflict);
        }
        let record = roots
            .get(&root_binding_id)
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        return Ok(RootUnbindReceipt {
            root_binding_id,
            record_revision: record.record_revision,
            registry_revision: record.registry_revision,
            operation_id: operation_id.clone(),
            obligations: vec![
                RootPolicyObligation::InvalidateSources,
                RootPolicyObligation::InvalidateMemberships,
                RootPolicyObligation::InvalidateViews,
            ],
            receipt: record.last_receipt.clone(),
            replayed: true,
        });
    }
    let record = roots
        .get_mut(&root_binding_id)
        .ok_or(RegistryError::RootNotRegistered)?;
    if record.record_revision != expected_record_revision {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    if record.status != RootStatus::Active {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    persist(control_port, &entry, context, &mutation)?;
    record.record_revision = record
        .record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    record.status = RootStatus::Unbound;
    record.registry_revision = next_registry_revision;
    record.last_receipt = receipt.clone();
    Ok(RootUnbindReceipt {
        root_binding_id,
        record_revision: record.record_revision,
        registry_revision: next_registry_revision,
        operation_id: operation_id.clone(),
        obligations: vec![
            RootPolicyObligation::InvalidateSources,
            RootPolicyObligation::InvalidateMemberships,
            RootPolicyObligation::InvalidateViews,
        ],
        receipt: receipt.clone(),
        replayed: false,
    })
}

fn obligations_for(
    old_revision: NonZeroRevision,
    new_revision: NonZeroRevision,
) -> Vec<RootPolicyObligation> {
    let mut obligations = vec![
        RootPolicyObligation::InvalidateSources,
        RootPolicyObligation::InvalidateMemberships,
        RootPolicyObligation::InvalidateViews,
    ];
    if new_revision < old_revision {
        obligations.push(RootPolicyObligation::ReconcileRestrictive);
    } else {
        obligations.push(RootPolicyObligation::ReconcilePermissive);
    }
    obligations
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
        .map_err(|error| {
            let _ = error;
            RegistryError::DurabilityRejected
        })
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
        .map_err(|error| {
            let _ = error;
            RegistryError::DurabilityRejected
        })?;
    Ok(())
}

/// Returns whether a port failure kind signals operation conflict.
#[must_use]
pub const fn is_conflict_kind(kind: PortErrorKind) -> bool {
    matches!(kind, PortErrorKind::Conflict)
}
