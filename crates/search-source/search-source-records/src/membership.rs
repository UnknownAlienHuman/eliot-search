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
    MembershipRevision, MembershipRole, NonZeroRevision, OpaqueId, ReceiptRef, RootBindingId,
    SourceIdentity, SourceMembership, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};

/// Schema version for complete permanent-contract memberships.
pub const PERMANENT_MEMBERSHIP_RECORD_V2_SCHEMA: u64 = 2;

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
    permanent_v2: Option<PermanentMembershipRecordV2>,
}

/// Exact source-admission provenance referenced by a permanent membership.
///
/// The original policy, observation and decision remain owned by the matching
/// permanent `RegisteredSource`. These exact references and fences bind the
/// membership to that retained provenance without copying source content or
/// manufacturing a legacy admission receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermanentMembershipProvenanceV2 {
    source_identity: SourceIdentity,
    source_revision: NonZeroRevision,
    root_binding_id: RootBindingId,
    root_record_revision: NonZeroRevision,
    source_namespace_id: SourceNamespaceId,
    namespace_record_revision: NonZeroRevision,
    owner_generation: SourceOwnerGeneration,
    policy_reference: Vec<u8>,
    admission_reference: Vec<u8>,
    policy_revision: NonZeroRevision,
}

impl PermanentMembershipProvenanceV2 {
    /// Captures the exact retained permanent source provenance and owner fences.
    #[must_use]
    pub fn capture(
        source_identity: SourceIdentity,
        source_revision: NonZeroRevision,
        root_binding_id: RootBindingId,
        admission: &crate::provenance::PermanentSourceAdmissionV2,
    ) -> Self {
        let decision = admission.original().receipt().decision();
        Self {
            source_namespace_id: source_identity.source_namespace_id,
            source_identity,
            source_revision,
            root_binding_id,
            root_record_revision: admission.root_record_revision(),
            namespace_record_revision: admission.namespace_record_revision(),
            owner_generation: admission.owner_generation(),
            policy_reference: admission.policy_reference().to_vec(),
            admission_reference: admission.admission_reference().to_vec(),
            policy_revision: decision.policy_revision(),
        }
    }

    /// Restores exact owner-produced references and fences from the versioned V2 codec.
    ///
    /// This reconstructs metadata only. Current source, root, namespace and
    /// policy authority must still be checked by `search-source-registry`.
    pub fn from_stored_parts(
        source_identity: SourceIdentity,
        source_revision: NonZeroRevision,
        root_binding_id: RootBindingId,
        root_record_revision: NonZeroRevision,
        source_namespace_id: SourceNamespaceId,
        namespace_record_revision: NonZeroRevision,
        owner_generation: SourceOwnerGeneration,
        policy_reference: Vec<u8>,
        admission_reference: Vec<u8>,
        policy_revision: NonZeroRevision,
    ) -> Result<Self, crate::error::RegistryError> {
        if source_namespace_id != source_identity.source_namespace_id
            || policy_reference.is_empty()
            || admission_reference.is_empty()
            || policy_reference.len() > search_contracts::MAX_CANONICAL_BYTES
            || admission_reference.len() > search_contracts::MAX_CANONICAL_BYTES
            || owner_generation.as_bytes().iter().all(|byte| *byte == 0)
        {
            return Err(crate::error::RegistryError::SnapshotInvalid);
        }
        Ok(Self {
            source_identity,
            source_revision,
            root_binding_id,
            root_record_revision,
            source_namespace_id,
            namespace_record_revision,
            owner_generation,
            policy_reference,
            admission_reference,
            policy_revision,
        })
    }

    /// Full stable source identity resolved by the current source owner.
    pub const fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    /// Source revision observed at membership binding.
    pub const fn source_revision(&self) -> NonZeroRevision {
        self.source_revision
    }

    /// Exact root binding and record revision observed by admission.
    pub const fn root_binding_id(&self) -> RootBindingId {
        self.root_binding_id
    }

    /// Root record revision observed by permanent admission.
    pub const fn root_record_revision(&self) -> NonZeroRevision {
        self.root_record_revision
    }

    /// Source namespace ID from the resolved source identity.
    pub const fn source_namespace_id(&self) -> SourceNamespaceId {
        self.source_namespace_id
    }

    /// Namespace ownership record revision observed by permanent admission.
    pub const fn namespace_record_revision(&self) -> NonZeroRevision {
        self.namespace_record_revision
    }

    /// Exact namespace owner generation observed by permanent admission.
    pub const fn owner_generation(&self) -> SourceOwnerGeneration {
        self.owner_generation
    }

    /// Exact retained immutable policy reference.
    pub fn policy_reference(&self) -> &[u8] {
        &self.policy_reference
    }

    /// Exact retained immutable admission reference.
    pub fn admission_reference(&self) -> &[u8] {
        &self.admission_reference
    }

    /// Admission policy revision retained in the permanent source decision.
    pub const fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }
}

/// Complete membership record for the accepted permanent `SourceMembership` contract.
///
/// Its typed UUIDs and policy/profile bindings are retained verbatim. The
/// legacy membership ID formula and serialized V1 fields remain frozen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermanentMembershipRecordV2 {
    member: SourceMembership,
    provenance: PermanentMembershipProvenanceV2,
    lifecycle: MembershipLifecycle,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl PermanentMembershipRecordV2 {
    /// Constructs a record from the exact accepted member and permanent provenance.
    #[must_use]
    pub const fn new(
        member: SourceMembership,
        provenance: PermanentMembershipProvenanceV2,
        lifecycle: MembershipLifecycle,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Self {
        Self {
            member,
            provenance,
            lifecycle,
            registry_revision,
            last_receipt,
        }
    }

    /// Exact accepted typed member, including every UUID and policy/profile field.
    pub const fn member(&self) -> &SourceMembership {
        &self.member
    }

    /// Exact permanent source-provenance references and admission fences.
    pub const fn provenance(&self) -> &PermanentMembershipProvenanceV2 {
        &self.provenance
    }

    /// Current retained lifecycle state.
    pub const fn lifecycle(&self) -> MembershipLifecycle {
        self.lifecycle
    }

    /// Registry revision that last changed this record.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt of the last membership mutation.
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }

    /// Creates a lifecycle revision while preserving the entire typed member and provenance.
    #[must_use]
    pub fn with_lifecycle(
        &self,
        membership_revision: MembershipRevision,
        lifecycle: MembershipLifecycle,
        registry_revision: u64,
        receipt: ReceiptRef,
    ) -> Self {
        let mut member = self.member.clone();
        member.membership_revision = membership_revision;
        Self {
            member,
            provenance: self.provenance.clone(),
            lifecycle,
            registry_revision,
            last_receipt: receipt,
        }
    }
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
            permanent_v2: None,
        }
    }

    /// Creates a permanent-contract record without changing the frozen legacy constructor.
    pub fn new_permanent_v2(
        key: MembershipKey,
        generation: NonZeroRevision,
        member: SourceMembership,
        provenance: PermanentMembershipProvenanceV2,
        lifecycle: MembershipLifecycle,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Result<Self, crate::error::RegistryError> {
        if key.source_identity != *provenance.source_identity()
            || member.source_id != provenance.source_identity().source_id
        {
            return Err(crate::error::RegistryError::SnapshotInvalid);
        }
        let membership_revision = NonZeroRevision::new(member.membership_revision.get())
            .map_err(|_| crate::error::RegistryError::SnapshotInvalid)?;
        let membership_id = member.source_membership_id;
        let v2 = PermanentMembershipRecordV2::new(
            member,
            provenance,
            lifecycle,
            registry_revision,
            last_receipt.clone(),
        );
        Ok(Self {
            key,
            membership_id,
            // This compatibility field mirrors the contract membership revision.
            // Permanent callers use `permanent_v2()` for namespace/source fences.
            generation,
            membership_revision,
            lifecycle,
            registry_revision,
            last_receipt,
            permanent_v2: Some(v2),
        })
    }

    /// Stable membership key.
    pub const fn key(&self) -> &MembershipKey {
        &self.key
    }

    /// Stable reverse membership identity.
    pub const fn membership_id(&self) -> SourceMembershipId {
        self.membership_id
    }

    /// Legacy corpus generation; permanent V2 callers use `permanent_v2()` for owner fences.
    pub const fn generation(&self) -> NonZeroRevision {
        self.generation
    }

    /// Full permanent-contract membership when this record uses schema v2.
    pub const fn permanent_v2(&self) -> Option<&PermanentMembershipRecordV2> {
        self.permanent_v2.as_ref()
    }

    /// Whether this record retains an accepted permanent SourceMembership.
    pub const fn is_permanent_v2(&self) -> bool {
        self.permanent_v2.is_some()
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

    /// Applies a typed permanent lifecycle revision without changing member policy fields.
    pub fn with_permanent_v2_lifecycle(
        &self,
        membership_revision: MembershipRevision,
        lifecycle: MembershipLifecycle,
        registry_revision: u64,
        receipt: ReceiptRef,
    ) -> Result<Self, crate::error::RegistryError> {
        let v2 = self
            .permanent_v2
            .as_ref()
            .ok_or(crate::error::RegistryError::SnapshotInvalid)?
            .with_lifecycle(
                membership_revision,
                lifecycle,
                registry_revision,
                receipt.clone(),
            );
        let generation = NonZeroRevision::new(membership_revision.get())
            .map_err(|_| crate::error::RegistryError::SnapshotInvalid)?;
        let key = self.key.clone();
        Self::new_permanent_v2(
            key,
            generation,
            v2.member,
            v2.provenance,
            lifecycle,
            registry_revision,
            receipt,
        )
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
