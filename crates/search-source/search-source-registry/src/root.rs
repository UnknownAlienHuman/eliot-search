//! Admitted root registration bound to a versioned admission-policy fingerprint.
//!
//! Roots are explicit records with no implicit recursive source admission.
//! Every mutation uses a stable operation identity, an expected registry
//! revision guard and atomic state-plus-receipt persistence through the vendor-neutral
//! [`crate::error::RegistryControlPort`]. No filesystem, redb or concrete
//! store handle is touched here.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef};
use search_ports::{CancellationProbe, OperationContext, PortErrorKind};

use crate::error::{RegistryControlPort, RegistryError, RegistryLimits, cancelled_before_commit};

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

/// Registers a complete root and its original operation receipt atomically.
pub fn register_root<C, P>(
    registry: &mut crate::recovery::SourceRegistry,
    request: &RegisterRootRequest,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootRegistrationReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    let committed = transact_root(
        registry,
        request.root_binding_id,
        crate::recovery::RegistryChange::RegisterRoot {
            request: request.clone(),
            receipt: receipt.clone(),
        },
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    Ok(RootRegistrationReceipt {
        root_binding_id: committed.after.root_binding_id,
        record_revision: committed.after.record_revision,
        registry_revision: committed.after.registry_revision,
        operation_id: operation_id.clone(),
        receipt: committed.receipt,
        replayed: committed.replayed,
    })
}

/// Replaces a complete root policy fence through the same atomic state transaction.
pub fn update_root_policy<C, P>(
    registry: &mut crate::recovery::SourceRegistry,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
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
    let committed = transact_root(
        registry,
        root_binding_id,
        crate::recovery::RegistryChange::UpdateRootPolicy {
            root_binding_id,
            expected_record_revision,
            policy_fingerprint: new_policy_fingerprint,
            policy_revision: new_policy_revision,
            receipt: receipt.clone(),
        },
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    let old = committed
        .before
        .ok_or(RegistryError::MutationOutcomeUnknown)?;
    Ok(RootPolicyChangeReceipt {
        root_binding_id,
        record_revision: committed.after.record_revision,
        registry_revision: committed.after.registry_revision,
        operation_id: operation_id.clone(),
        obligations: obligations_for(old.policy_revision, committed.after.policy_revision),
        receipt: committed.receipt,
        replayed: committed.replayed,
    })
}

/// Retains a complete unbound root and original invalidation receipt atomically.
pub fn unbind_root<C, P>(
    registry: &mut crate::recovery::SourceRegistry,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
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
    let committed = transact_root(
        registry,
        root_binding_id,
        crate::recovery::RegistryChange::UnbindRoot {
            root_binding_id,
            expected_record_revision,
            receipt: receipt.clone(),
        },
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    Ok(RootUnbindReceipt {
        root_binding_id,
        record_revision: committed.after.record_revision,
        registry_revision: committed.after.registry_revision,
        operation_id: operation_id.clone(),
        obligations: vec![
            RootPolicyObligation::InvalidateSources,
            RootPolicyObligation::InvalidateMemberships,
            RootPolicyObligation::InvalidateViews,
        ],
        receipt: committed.receipt,
        replayed: committed.replayed,
    })
}

struct CommittedRoot {
    before: Option<RootRecord>,
    after: RootRecord,
    receipt: ReceiptRef,
    replayed: bool,
}
fn transact_root<C, P>(
    registry: &mut crate::recovery::SourceRegistry,
    root_id: search_contracts::RootBindingId,
    mut change: crate::recovery::RegistryChange,
    operation_id: &OpaqueId,
    digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control: &mut P,
    context: &OperationContext<C>,
) -> Result<CommittedRoot, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    use crate::recovery::{
        RegistryBatch, RegistryBatchLookup, RegistryOperation, RegistryStateKey,
        RegistryStateRecord,
    };
    cancelled_before_commit(context)?;
    // The immutable operation retains its original global revision for exact
    // replay. Verified absence carries a coherent current owner revision.
    let (expected, original_receipt) = match control
        .load_batch(operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    {
        RegistryBatchLookup::Committed(readback) => {
            let deltas = readback.record().deltas();
            if deltas.len() != 1 || deltas[0].key() != &RegistryStateKey::Root(root_id) {
                return Err(RegistryError::OperationConflict);
            }
            let RegistryStateRecord::Root(root) = deltas[0].after() else {
                return Err(RegistryError::OperationConflict);
            };
            (
                readback.record().expected_registry_revision(),
                root.last_receipt.clone(),
            )
        }
        RegistryBatchLookup::Absent { revision } if revision == registry.revision() => {
            (revision, receipt.clone())
        }
        RegistryBatchLookup::Absent { .. } => return Err(RegistryError::RegistryRevisionConflict),
    };
    match &mut change {
        crate::recovery::RegistryChange::RegisterRoot { receipt, .. }
        | crate::recovery::RegistryChange::UpdateRootPolicy { receipt, .. }
        | crate::recovery::RegistryChange::UnbindRoot { receipt, .. } => {
            *receipt = original_receipt;
        }
        _ => return Err(RegistryError::OperationConflict),
    }
    let result = crate::recovery::apply_admission_batch(
        registry,
        RegistryBatch {
            expected_registry_revision: expected,
            operation: RegistryOperation::new(operation_id.clone(), digest),
            changes: vec![change],
        },
        receipt,
        control,
        context,
    )?;
    if let Some(reason) = result.items.iter().find_map(|item| item.reason) {
        return Err(reason);
    }
    let RegistryBatchLookup::Committed(readback) = control
        .load_batch(operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    else {
        return Err(RegistryError::MutationOutcomeUnknown);
    };
    let record = readback.record();
    if record.entry().operation_id != *operation_id
        || record.entry().mutation_digest != digest
        || record.entry().before_revision != result.before_revision
        || record.entry().after_revision != result.after_revision
        || record.entry().receipt != result.receipt
        || record.deltas().len() != 1
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let delta = &record.deltas()[0];
    if delta.key() != &RegistryStateKey::Root(root_id) {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let RegistryStateRecord::Root(after) = delta.after() else {
        return Err(RegistryError::MutationOutcomeUnknown);
    };
    if after.registry_revision != result.after_revision {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let before = match delta.before() {
        None => None,
        Some(RegistryStateRecord::Root(root)) => Some(root.clone()),
        Some(_) => return Err(RegistryError::MutationOutcomeUnknown),
    };
    Ok(CommittedRoot {
        before,
        after: after.clone(),
        receipt: result.receipt,
        replayed: result.replayed,
    })
}

/// Validates complete root metadata without granting physical custody or current access.
pub(crate) fn validate_metadata(root: &RootRecord) -> Result<(), RegistryError> {
    if root.registry_revision == 0 || root.record_revision.get() > root.registry_revision {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(())
}
pub(crate) fn stage_registration(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    request: &RegisterRootRequest,
    revision: u64,
    receipt: &ReceiptRef,
    limits: RegistryLimits,
) -> Result<(), RegistryError> {
    if roots.contains_key(&request.root_binding_id) {
        return Err(RegistryError::RootAlreadyRegistered);
    }
    if roots
        .values()
        .any(|root| root.canonical_root_digest == request.canonical_root_digest)
    {
        return Err(RegistryError::RootIdentityConflict);
    }
    if roots.len() >= limits.max_roots {
        return Err(RegistryError::CapacityExceeded);
    }
    let root = RootRecord {
        root_binding_id: request.root_binding_id,
        canonical_root_digest: request.canonical_root_digest,
        policy_fingerprint: request.policy_fingerprint,
        policy_revision: request.policy_revision,
        owner_epoch: request.owner_epoch,
        record_revision: NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?,
        registry_revision: revision,
        status: RootStatus::Active,
        last_receipt: receipt.clone(),
    };
    validate_metadata(&root)?;
    roots.insert(root.root_binding_id, root);
    Ok(())
}
pub(crate) fn stage_policy(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    id: search_contracts::RootBindingId,
    expected: NonZeroRevision,
    fingerprint: Blake3Digest32,
    policy_revision: NonZeroRevision,
    revision: u64,
    receipt: &ReceiptRef,
) -> Result<(), RegistryError> {
    let root = roots.get_mut(&id).ok_or(RegistryError::RootNotRegistered)?;
    if root.status != RootStatus::Active || root.record_revision != expected {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    let next = root
        .record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    if revision <= root.registry_revision {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    root.record_revision = next;
    root.policy_fingerprint = fingerprint;
    root.policy_revision = policy_revision;
    root.registry_revision = revision;
    root.last_receipt = receipt.clone();
    validate_metadata(root)
}
pub(crate) fn stage_unbind(
    roots: &mut BTreeMap<search_contracts::RootBindingId, RootRecord>,
    id: search_contracts::RootBindingId,
    expected: NonZeroRevision,
    revision: u64,
    receipt: &ReceiptRef,
) -> Result<(), RegistryError> {
    let root = roots.get_mut(&id).ok_or(RegistryError::RootNotRegistered)?;
    if root.status != RootStatus::Active || root.record_revision != expected {
        return Err(RegistryError::RootPolicyGenerationMismatch);
    }
    let next = root
        .record_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    if revision <= root.registry_revision {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    root.record_revision = next;
    root.registry_revision = revision;
    root.status = RootStatus::Unbound;
    root.last_receipt = receipt.clone();
    validate_metadata(root)
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

/// Returns whether a port failure kind signals operation conflict.
#[must_use]
pub const fn is_conflict_kind(kind: PortErrorKind) -> bool {
    matches!(kind, PortErrorKind::Conflict)
}
