//! Immutable root metadata values, without currentness or source admission grants.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef};

use crate::error::RegistryError;

/// Validates complete root metadata without granting physical custody or current access.
pub fn validate_metadata(root: &RootRecord) -> Result<(), RegistryError> {
    if root.registry_revision == 0 || root.record_revision.get() > root.registry_revision {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(())
}

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
