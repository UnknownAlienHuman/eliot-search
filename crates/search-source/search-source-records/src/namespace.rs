//! Immutable namespace cutover values and retained permanent-v2 metadata.
//!
//! This module owns the value side of namespace state. It does not allocate a
//! namespace, perform a cutover, evaluate admission, grant native access or
//! read durable state. The registry remains the sole mutable owner of the
//! [`NamespaceCutoverState`] value.

use std::sync::Arc;

use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NamespaceOwnershipStatus, NonZeroRevision, OpaqueId,
    OwnerEpoch, ReceiptRef, RootBindingId, SourceIdentity, SourceMembershipId, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerCutoverReceipt, SourceOwnerGeneration,
};
use search_source_admission::{AdmissionError, policy_v2::CheckpointFailureV2};
use search_source_identity::FilesystemIdentityProfile;
use search_source_provenance::{OriginalInputsLayoutV2, RetainedPolicyV2};

use crate::error::RegistryError;

/// Durable cutover preparation record; it never activates a new owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverPreparation {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace under cutover.
    pub namespace_id: SourceNamespaceId,
    /// Current owner generation at preparation time.
    pub current_generation: SourceOwnerGeneration,
    /// Proposed new owner system.
    pub proposed_owner: OpaqueId,
    /// Proposed new owner incarnation.
    pub proposed_incarnation: InstallationIncarnationId,
    /// Affected source identities in canonical order.
    pub affected_sources: Vec<SourceIdentity>,
    /// Affected membership identities in canonical order.
    pub affected_memberships: Vec<SourceMembershipId>,
    /// Digest of migration/export evidence.
    pub evidence_digest: Blake3Digest32,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this record came from exact idempotency replay.
    pub replayed: bool,
}

/// Durable old-owner fence receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerFenceReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Fenced owner generation.
    pub fenced_generation: SourceOwnerGeneration,
    /// Fence record revision.
    pub fence_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Durable new-owner activation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerActivationReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Activation record revision.
    pub activation_revision: NonZeroRevision,
    /// Registry revision after commit.
    pub registry_revision: u64,
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free durability receipt.
    pub receipt: ReceiptRef,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// Verified cutover receipt proving fence-before-activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCutoverReceipt {
    /// Stable cutover identity.
    pub cutover_id: search_contracts::CutoverId,
    /// Source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Owner generation before fence.
    pub old_generation: SourceOwnerGeneration,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Covered source count.
    pub covered_sources: usize,
    /// Covered membership count.
    pub covered_memberships: usize,
}

/// Closed cutover recovery decision from durable records.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CutoverRecoveryDecision {
    /// Preparation is durable; old owner is not yet fenced.
    Prepared,
    /// Old owner is durably fenced; new owner is not yet active.
    OldOwnerFenced,
    /// New owner is active.
    NewOwnerActive,
    /// Cutover is complete with a verified receipt.
    Completed,
    /// Durable records conflict and require operator review.
    Conflicting,
    /// Durable records are contradictory and quarantined.
    Quarantined,
}

/// Closed namespace ownership command for the shared ownership port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamespaceOwnershipCommand {
    /// Prepare one exact cutover under the current owner.
    Prepare {
        /// Cutover receipt reference to record.
        cutover_receipt_ref: ReceiptRef,
    },
    /// Fence the old owner generation.
    Fence,
    /// Retire the fenced old owner.
    Retire,
    /// Activate a distinct new owner with the exact cutover receipt.
    Activate {
        /// New owner system identity.
        new_owner: OpaqueId,
        /// New owner incarnation.
        new_incarnation: InstallationIncarnationId,
        /// New owner epoch, strictly greater than the current epoch.
        new_epoch: OwnerEpoch,
        /// New owner generation, distinct from the current generation.
        new_generation: SourceOwnerGeneration,
        /// Exact accepted cutover receipt.
        receipt: SourceOwnerCutoverReceipt,
    },
}

/// Closed error for permanent-v2 namespace metadata construction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NamespaceMetadataError {
    /// The registry-owned namespace or revision fence was invalid.
    Registry(RegistryError),
    /// The retained policy reference or original checkpoint failed.
    Admission(AdmissionError),
}

impl From<RegistryError> for NamespaceMetadataError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<AdmissionError> for NamespaceMetadataError {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}

/// One retained permanent-v2 root/policy binding for a namespace owner.
///
/// The policy is shared by `Arc`; its canonical reference is retained as the
/// exact immutable locator used by the original-input owner. This value grants
/// no current policy authority and contains no source bytes.
#[derive(Clone, Debug)]
pub struct NamespaceMetadataV2 {
    root_binding_id: RootBindingId,
    identity_profile: FilesystemIdentityProfile,
    policy: Arc<RetainedPolicyV2>,
    policy_reference: Vec<u8>,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl PartialEq for NamespaceMetadataV2 {
    fn eq(&self, other: &Self) -> bool {
        self.root_binding_id == other.root_binding_id
            && self.identity_profile == other.identity_profile
            && self.policy.encoded_inputs() == other.policy.encoded_inputs()
            && self.policy_reference == other.policy_reference
            && self.registry_revision == other.registry_revision
            && self.last_receipt == other.last_receipt
    }
}

impl Eq for NamespaceMetadataV2 {}

impl NamespaceMetadataV2 {
    /// Actual admitted root binding associated with this namespace.
    #[must_use]
    pub const fn root_binding_id(&self) -> RootBindingId {
        self.root_binding_id
    }

    /// Exact qualified C04 filesystem identity profile pinned to this namespace.
    #[must_use]
    pub const fn identity_profile(&self) -> FilesystemIdentityProfile {
        self.identity_profile
    }

    /// Shared retained policy model, never a live policy decision authority.
    #[must_use]
    pub fn policy(&self) -> &Arc<RetainedPolicyV2> {
        &self.policy
    }

    /// Exact cached policy archive reference.
    #[must_use]
    pub fn policy_reference(&self) -> &[u8] {
        &self.policy_reference
    }

    /// Registry revision that committed this namespace metadata.
    #[must_use]
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt for the metadata commit.
    #[must_use]
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }
}

/// Explicit namespace metadata provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamespaceMetadata {
    /// Historical state has no retained root/policy binding.
    HistoricalV1Unbound,
    /// Complete permanent-v2 root/policy binding.
    PermanentV2(NamespaceMetadataV2),
}

/// Durable namespace cutover state retained by the registry.
///
/// The four legacy fields remain the cutover owner's state. `metadata` is an
/// explicit provenance value on that same owner; it is not a second namespace
/// map or an alternate ownership evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceCutoverState {
    /// Current ownership record.
    pub ownership: SourceNamespaceOwnership,
    /// Durable preparation, when one exists.
    pub preparation: Option<CutoverPreparation>,
    /// Whether the old owner generation is durably fenced.
    pub fenced: bool,
    /// Verified activation receipt, when active.
    pub activation: Option<SourceOwnerActivationReceipt>,
    metadata: NamespaceMetadata,
}

impl NamespaceCutoverState {
    /// Creates an explicitly unbound historical state.
    ///
    /// This preserves the legacy constructor for old cutover records. It does
    /// not infer a root, policy, registry revision or permanent-v2 authority.
    #[must_use]
    pub const fn new(ownership: SourceNamespaceOwnership) -> Self {
        Self {
            ownership,
            preparation: None,
            fenced: false,
            activation: None,
            metadata: NamespaceMetadata::HistoricalV1Unbound,
        }
    }

    /// Creates the first permanent-v2 namespace metadata state.
    ///
    /// The caller supplies the actual owner record and retained policy. The
    /// factory accepts only an active first-admission state, verifies the
    /// policy revision and the six-axis domain-owned generation, and compares
    /// the supplied cached policy reference with the actual owner reference.
    /// No legacy cutover fields are inferred or persisted for this binding.
    ///
    /// # Errors
    /// Returns the closed registry or original-admission failure without a
    /// partial state value.
    pub fn new_v2_metadata(
        ownership: SourceNamespaceOwnership,
        root_binding_id: RootBindingId,
        identity_profile: FilesystemIdentityProfile,
        policy: Arc<RetainedPolicyV2>,
        cached_policy_reference: Vec<u8>,
        registry_revision: u64,
        last_receipt: ReceiptRef,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, NamespaceMetadataError> {
        checkpoint(probe)?;
        FilesystemIdentityProfile::new(
            identity_profile.revision(),
            identity_profile.schema_digest(),
            identity_profile.case_behavior(),
            identity_profile.unicode_behavior(),
            identity_profile.volume_identity(),
            identity_profile.file_identity(),
            identity_profile.link_behavior(),
            identity_profile.reparse_behavior(),
        )
        .map_err(|_| RegistryError::SnapshotInvalid)?;
        if ownership.status != NamespaceOwnershipStatus::Active
            || ownership.source_admission_policy_revision.get()
                != policy.compiled().revision().get()
            || registry_revision == 0
            || ownership.ownership_record_revision.get() > registry_revision
        {
            return Err(RegistryError::NamespaceOwnershipConflict.into());
        }
        let generation = search_domain::source_ownership_v2::source_owner_generation_v2(&ownership)
            .map_err(|_| RegistryError::CutoverGenerationConflict)?;
        checkpoint(probe)?;
        if generation != ownership.source_owner_generation {
            return Err(RegistryError::CutoverGenerationConflict.into());
        }
        let actual_policy_reference =
            OriginalInputsLayoutV2::policy_reference(policy.as_ref(), probe)?;
        if actual_policy_reference != cached_policy_reference {
            return Err(AdmissionError::ReceiptMismatch.into());
        }
        checkpoint(probe)?;
        Ok(Self {
            ownership,
            preparation: None,
            fenced: false,
            activation: None,
            metadata: NamespaceMetadata::PermanentV2(NamespaceMetadataV2 {
                root_binding_id,
                identity_profile,
                policy,
                policy_reference: cached_policy_reference,
                registry_revision,
                last_receipt,
            }),
        })
    }

    /// Exact provenance metadata on this same namespace owner.
    #[must_use]
    pub const fn metadata(&self) -> &NamespaceMetadata {
        &self.metadata
    }

    /// Returns the permanent-v2 binding when present.
    #[must_use]
    pub const fn permanent_metadata(&self) -> Option<&NamespaceMetadataV2> {
        match &self.metadata {
            NamespaceMetadata::HistoricalV1Unbound => None,
            NamespaceMetadata::PermanentV2(metadata) => Some(metadata),
        }
    }

    /// Whether legacy cutover mutation must refuse this permanent binding.
    #[must_use]
    pub const fn is_permanent_v2(&self) -> bool {
        matches!(&self.metadata, NamespaceMetadata::PermanentV2(_))
    }
}

fn checkpoint(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), NamespaceMetadataError> {
    probe().map_err(|failure| {
        NamespaceMetadataError::Admission(match failure {
            CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
            CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
        })
    })
}
