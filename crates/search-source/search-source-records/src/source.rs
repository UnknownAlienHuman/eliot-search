//! Immutable source record value models owned by `search-source-records`.
//!
//! These types are the data/schema owner for source identity, lifecycle,
//! admission binding, and content-free receipts. No I/O, no admission
//! authority, no grant or currentness logic lives here; the current owner
//! retains all semantic mutation functions.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity};
use search_source_admission::original_envelope::OriginalAdmissionEnvelope;
use search_source_admission::{AdmissionOutcome, AdmissionReceipt};
use search_source_identity::SourceBinding;

use crate::error::RegistryError;
use crate::provenance::{PermanentSourceAdmissionV2, SourceProvenance};

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
    provenance: SourceProvenance,
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
            provenance: SourceProvenance::HistoricalV1 {
                admission,
                original_admission: None,
                assignment,
            },
            lifecycle,
            source_revision,
            registry_revision,
            last_receipt,
        }
    }

    /// Constructs permanent metadata with complete real originals and exact binding.
    /// This does not validate current Root/namespace state or grant native access.
    pub fn new_permanent(
        binding: SourceBinding,
        provenance: PermanentSourceAdmissionV2,
        lifecycle: SourceLifecycle,
        source_revision: NonZeroRevision,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Result<Self, RegistryError> {
        provenance.validate_binding(&binding)?;
        Ok(Self {
            binding,
            provenance: SourceProvenance::PermanentV2(provenance),
            lifecycle,
            source_revision,
            registry_revision,
            last_receipt,
        })
    }

    /// Exact closed provenance alternative; a version is not current authority.
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Stable source identity.
    pub const fn identity(&self) -> &SourceIdentity {
        self.binding.identity()
    }

    /// Current source binding.
    pub const fn binding(&self) -> &SourceBinding {
        &self.binding
    }

    /// Historical v1 receipt only. Permanent evidence has no v1 representation.
    pub const fn legacy_admission(&self) -> Option<&AdmissionReceipt> {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { admission, .. } => Some(admission),
            SourceProvenance::PermanentV2(_) => None,
        }
    }

    /// Complete permanent evidence, without a currentness assertion.
    pub const fn permanent_admission(&self) -> Option<&PermanentSourceAdmissionV2> {
        match &self.provenance {
            SourceProvenance::PermanentV2(original) => Some(original),
            SourceProvenance::HistoricalV1 { .. } => None,
        }
    }

    /// Actual outcome from the corresponding sole receipt owner.
    pub fn admission_outcome(&self) -> AdmissionOutcome {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { admission, .. } => admission.outcome(),
            SourceProvenance::PermanentV2(original) => {
                original.original().receipt().decision().outcome()
            }
        }
    }

    /// Exact admitted policy revision, independent of current policy.
    pub fn admission_policy_revision(&self) -> NonZeroRevision {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { admission, .. } => admission.policy_revision(),
            SourceProvenance::PermanentV2(original) => {
                original.original().receipt().decision().policy_revision()
            }
        }
    }

    /// Profile-bound fingerprint bytes. Comparing these never replaces profile checks.
    pub fn admission_policy_fingerprint(&self) -> [u8; 32] {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { admission, .. } => {
                *admission.policy_fingerprint().as_bytes()
            }
            SourceProvenance::PermanentV2(original) => *original
                .original()
                .receipt()
                .decision()
                .policy_fingerprint()
                .as_bytes(),
        }
    }

    /// Profile-bound observation bytes, without relabelling a legacy digest.
    pub fn admission_observation_fingerprint(&self) -> [u8; 32] {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { admission, .. } => {
                *admission.observation_digest().as_bytes()
            }
            SourceProvenance::PermanentV2(original) => *original
                .original()
                .receipt()
                .decision()
                .observation_fingerprint()
                .as_bytes(),
        }
    }

    /// Historical-v1 original inputs. Permanent-v2 originals are retained in `provenance()`.
    pub const fn original_admission(&self) -> Option<&OriginalAdmissionEnvelope> {
        match &self.provenance {
            SourceProvenance::HistoricalV1 {
                original_admission, ..
            } => original_admission.as_ref(),
            SourceProvenance::PermanentV2(_) => None,
        }
    }

    /// Original-input availability across the explicit provenance alternatives.
    pub const fn has_original_admission(&self) -> bool {
        match &self.provenance {
            SourceProvenance::HistoricalV1 {
                original_admission, ..
            } => original_admission.is_some(),
            SourceProvenance::PermanentV2(_) => true,
        }
    }

    /// Retains original admission metadata for legacy operation across the crate boundary.
    ///
    /// This is a metadata-only retention path: the envelope receipt must exactly match
    /// the bound admission, otherwise the binding is rejected. No grant or currentness
    /// semantics are introduced here; the current owner retains all admission authority.
    pub fn retaining_original(
        mut self,
        original: Option<OriginalAdmissionEnvelope>,
    ) -> Result<Self, RegistryError> {
        let SourceProvenance::HistoricalV1 {
            admission,
            original_admission,
            ..
        } = &mut self.provenance
        else {
            return Err(RegistryError::AdmissionReceiptMismatch);
        };
        if original
            .as_ref()
            .is_some_and(|envelope| envelope.receipt() != admission)
        {
            return Err(RegistryError::AdmissionBindingMismatch);
        }
        *original_admission = original;
        Ok(self)
    }

    /// Frozen historical assignment metadata; permanent admission accepts no verified flag.
    pub const fn legacy_assignment(&self) -> Option<&AdmissionBindingProof> {
        match &self.provenance {
            SourceProvenance::HistoricalV1 { assignment, .. } => Some(assignment),
            SourceProvenance::PermanentV2(_) => None,
        }
    }

    /// Retains exact provenance while producing restrictive retired metadata.
    /// The semantic owner checks the operation and monotone revisions before use.
    #[must_use]
    pub fn into_retired(
        mut self,
        source_revision: NonZeroRevision,
        registry_revision: u64,
        last_receipt: ReceiptRef,
    ) -> Self {
        self.lifecycle = SourceLifecycle::Retired;
        self.source_revision = source_revision;
        self.registry_revision = registry_revision;
        self.last_receipt = last_receipt;
        self
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
