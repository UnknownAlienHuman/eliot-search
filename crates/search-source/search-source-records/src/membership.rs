//! Source/corpus membership value types and the frozen membership-ID formula.
//!
//! This module owns the pure data side of membership: keys, lifecycle states,
//! records, commands, policies, obligations and receipts. The exact reverse
//! membership identity is derived deterministically from the frozen FNV-1a
//! formula over the corpus/source pair.
//!
//! Effectful bind/transition operations, admission gating and SourceRegistry
//! access decisions remain in the owning registry module. No filesystem,
//! clock, persistence or port I/O occurs here.

use search_contracts::{
    MembershipRole, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity,
    SourceMembershipId,
};

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
/// the owning registry).
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
///
/// The frozen FNV-1a-64 formula hashes the corpus ID string and the source
/// identity bytes. The result is encoded as two 8-byte halves: the hash and
/// its bitwise complement. This wire formula must remain unchanged for
/// compatibility with existing journal entries and reverse-index lookups.
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