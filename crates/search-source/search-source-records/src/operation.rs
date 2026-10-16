//! Immutable SourceRegistry request, receipt and recovery descriptors.
//!
//! These metadata values do not perform mutations or establish current authority.
use crate::error::{RegistryError, RegistryLimits};
use crate::membership::{MembershipCommand, MembershipKey, NewMembership};
use crate::source::AdmissionBindingProof;
use search_contracts::{
    Blake3Digest32, MembershipRevision, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity,
    SourceMembership, SourceMembershipId, SourceOwnerGeneration,
};
use search_source_admission::AdmissionReceipt;
use search_source_identity::SourceBinding;
use std::collections::BTreeSet;
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
    /// Register a complete root record, with no implicit source admission.
    RegisterRoot {
        /// Original root registration request, including owner/policy receipt references.
        request: crate::root::RegisterRootRequest,
        /// Original root mutation receipt.
        receipt: ReceiptRef,
    },
    /// Replace a root policy fence under its exact record revision.
    UpdateRootPolicy {
        /// Exact stable root identity.
        root_binding_id: search_contracts::RootBindingId,
        /// Expected root record revision.
        expected_record_revision: NonZeroRevision,
        /// Replacement policy fingerprint.
        policy_fingerprint: Blake3Digest32,
        /// Replacement policy revision.
        policy_revision: NonZeroRevision,
        /// Original mutation receipt.
        receipt: ReceiptRef,
    },
    /// Retain a complete unbound root under its exact record revision.
    UnbindRoot {
        /// Exact stable root identity.
        root_binding_id: search_contracts::RootBindingId,
        /// Expected root record revision.
        expected_record_revision: NonZeroRevision,
        /// Original mutation receipt.
        receipt: ReceiptRef,
    },
    /// Register a newly assigned admitted source.
    RegisterSource {
        /// Verified admission receipt with `ALLOW` outcome.
        admission: AdmissionReceipt,
        /// Original owner-produced input envelope; absent legacy provenance is unavailable.
        original_admission:
            Option<search_source_admission::original_envelope::OriginalAdmissionEnvelope>,
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
    /// Bind the accepted typed SourceMembership under exact live owner fences.
    BindPermanentMembershipV2 {
        /// Exact accepted public contract member, retained without conversion.
        member: SourceMembership,
        /// Namespace used to resolve the member's source ID to one full identity.
        source_namespace_id: search_contracts::SourceNamespaceId,
        /// Exact admitted root binding used by the source.
        root_binding_id: search_contracts::RootBindingId,
        /// Exact namespace owner identity observed by the operation.
        owner: crate::source_request_v2::SourceOwnerScopeV2,
        /// Full current source identity resolved by the source owner.
        source_identity: SourceIdentity,
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Expected active root record revision.
        expected_root_record_revision: NonZeroRevision,
        /// Expected active namespace ownership record revision.
        expected_namespace_record_revision: NonZeroRevision,
        /// Expected active namespace owner generation.
        expected_owner_generation: SourceOwnerGeneration,
        /// Content-free membership receipt.
        receipt: ReceiptRef,
    },
    /// Apply a closed lifecycle command while retaining all typed policy fields.
    TransitionPermanentMembershipV2 {
        /// Exact stable accepted membership UUID.
        membership_id: SourceMembershipId,
        /// Exact current typed membership revision.
        expected_membership_revision: MembershipRevision,
        /// Closed lifecycle command.
        command: MembershipCommand,
        /// Content-free transition receipt.
        receipt: ReceiptRef,
    },
    /// Atomically replace one corpus active inventory and generation.
    CutoverNamespace(NamespaceCutover),
}

impl RegistryChange {
    /// Root mutation target, if present in this immutable descriptor.
    pub fn root_target(&self) -> Option<search_contracts::RootBindingId> {
        match self {
            Self::RegisterRoot { request, .. } => Some(request.root_binding_id),
            Self::UpdateRootPolicy {
                root_binding_id, ..
            }
            | Self::UnbindRoot {
                root_binding_id, ..
            } => Some(*root_binding_id),
            _ => None,
        }
    }
    /// Source mutation target, if present in this immutable descriptor.
    pub fn source_target(&self) -> Option<&SourceIdentity> {
        match self {
            Self::RegisterSource { binding, .. } => Some(binding.identity()),
            Self::UpdateSourceBinding { identity, .. } | Self::RetireSource { identity, .. } => {
                Some(identity)
            }
            _ => None,
        }
    }

    /// Membership mutation target, if present in this immutable descriptor.
    pub fn membership_target(&self) -> Option<&MembershipKey> {
        match self {
            Self::AddMembership(membership) => Some(&membership.key),
            Self::RetireMembership { key, .. } => Some(key),
            _ => None,
        }
    }

    /// Permanent-contract membership identity targeted by this change.
    pub fn permanent_membership_target(&self) -> Option<SourceMembershipId> {
        match self {
            Self::BindPermanentMembershipV2 { member, .. } => Some(member.source_membership_id),
            Self::TransitionPermanentMembershipV2 { membership_id, .. } => Some(*membership_id),
            _ => None,
        }
    }

    /// Corpus cutover target, if present in this immutable descriptor.
    pub fn cutover_target(&self) -> Option<&OpaqueId> {
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
        let mut roots = BTreeSet::new();
        let mut original_bytes = 0_usize;
        let mut memberships = BTreeSet::new();
        let mut permanent_memberships = BTreeSet::new();
        let mut cutovers = BTreeSet::new();
        for change in &self.changes {
            if let Some(root) = change.root_target()
                && !roots.insert(root)
            {
                return Err(RegistryError::DuplicateBatchTarget);
            }
            if let RegistryChange::RegisterSource {
                original_admission: Some(original),
                ..
            } = change
            {
                original_bytes = original_bytes
                    .checked_add(original.encoded().len())
                    .ok_or(RegistryError::CapacityExceeded)?;
                if original_bytes > search_contracts::MAX_CANONICAL_BYTES {
                    return Err(RegistryError::CapacityExceeded);
                }
            }
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
            if let Some(membership) = change.permanent_membership_target()
                && (!permanent_memberships.insert(membership)
                    || memberships.iter().any(|legacy| {
                        crate::membership::derive_membership_id(legacy) == membership
                    }))
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
            if permanent_memberships.contains(&crate::membership::derive_membership_id(membership))
            {
                return Err(RegistryError::DuplicateBatchTarget);
            }
        }
        if !permanent_memberships.is_empty() && self.changes.len() != 1 {
            return Err(RegistryError::BatchSizeInvalid);
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
