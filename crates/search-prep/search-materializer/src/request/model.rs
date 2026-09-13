//! Materialization request and accepted-profile models.

use crate::profile::{
    MaterializerProfileId, SourceEncoding, SourceKind, ValidatedMaterializerProfile,
};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

/// Unvalidated materialization request. Paths, file handles and index payloads
/// cannot be expressed here by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationRequest {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Exact retained revision to reopen.
    pub revision: NonZeroRevision,
    /// Expected residency identity of the retained revision.
    pub residency: OpaqueId,
    /// Exact content digest attested by the revision pipeline.
    pub content_digest: Blake3Digest32,
    /// Caller-recorded exact byte count.
    pub byte_count: u64,
    /// Declared source kind hint, checked against the profile.
    pub declared_kind: SourceKind,
    /// Declared encoding hint, checked against the profile.
    pub declared_encoding: SourceEncoding,
    /// Required baseline profile identity.
    pub profile_id: MaterializerProfileId,
    /// Stable operation identity for retry correlation.
    pub operation_id: OpaqueId,
    /// Whether the bytes originate from an unsaved buffer.
    pub from_unsaved_bytes: bool,
    /// Explicit authenticated durable snapshot-admission receipt for unsaved bytes.
    pub unsaved_snapshot_receipt: Option<ReceiptRef>,
}

/// Accepted baseline profile set for request validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedProfiles {
    profiles: Vec<ValidatedMaterializerProfile>,
}

impl AcceptedProfiles {
    /// Builds the accepted set. An empty set accepts nothing.
    #[must_use]
    pub const fn new(profiles: Vec<ValidatedMaterializerProfile>) -> Self {
        Self { profiles }
    }

    /// Finds an accepted profile by canonical identity.
    #[must_use]
    pub fn find(&self, id: &MaterializerProfileId) -> Option<&ValidatedMaterializerProfile> {
        self.profiles.iter().find(|profile| &profile.id() == id)
    }

    /// Number of accepted profiles.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Reports whether the accepted set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// Request bound to one accepted profile and finite budgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMaterializationRequest {
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) residency: OpaqueId,
    pub(super) content_digest: Blake3Digest32,
    pub(super) byte_count: u64,
    pub(super) declared_kind: SourceKind,
    pub(super) declared_encoding: SourceEncoding,
    pub(super) profile: ValidatedMaterializerProfile,
    pub(super) operation_id: OpaqueId,
    pub(super) admitted_unsaved: bool,
}

impl ValidatedMaterializationRequest {
    /// Stable source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Exact retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Expected residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Exact content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    /// Caller-recorded exact byte count.
    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    /// Declared source kind hint.
    #[must_use]
    pub const fn declared_kind(&self) -> SourceKind {
        self.declared_kind
    }

    /// Declared encoding hint.
    #[must_use]
    pub const fn declared_encoding(&self) -> SourceEncoding {
        self.declared_encoding
    }

    /// Bound accepted profile.
    #[must_use]
    pub const fn profile(&self) -> &ValidatedMaterializerProfile {
        &self.profile
    }

    /// Stable operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Whether unsaved bytes were admitted through an explicit snapshot receipt.
    #[must_use]
    pub const fn admitted_unsaved(&self) -> bool {
        self.admitted_unsaved
    }

    /// Re-targets a validated request at a different retained revision.
    ///
    /// All other bindings are unchanged. Production callers revalidate and
    /// re-admit the new revision; this helper exists for verification and
    /// determinism flows that must hold every other input stable.
    #[must_use]
    pub const fn with_revision(mut self, revision: NonZeroRevision) -> Self {
        self.revision = revision;
        self
    }
}
