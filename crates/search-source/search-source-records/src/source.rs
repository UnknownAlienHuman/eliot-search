//! Immutable source record value models owned by `search-source-records`.
//!
//! These types are the data/schema owner for source identity, lifecycle,
//! admission binding, and content-free receipts. No I/O, no admission
//! authority, no grant or currentness logic lives here; the current owner
//! retains all semantic mutation functions.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity};
use search_source_admission::AdmissionReceipt;
use search_source_admission::original_envelope::OriginalAdmissionEnvelope;
use search_source_identity::SourceBinding;

use crate::error::RegistryError;

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
    original_admission: Option<OriginalAdmissionEnvelope>,
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
            original_admission: None,
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

    /// Retained original inputs; None explicitly means historical restoration is unavailable.
    pub const fn original_admission(&self) -> Option<&OriginalAdmissionEnvelope> {
        self.original_admission.as_ref()
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
        if original
            .as_ref()
            .is_some_and(|envelope| envelope.receipt() != &self.admission)
        {
            return Err(RegistryError::AdmissionBindingMismatch);
        }
        self.original_admission = original;
        Ok(self)
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
