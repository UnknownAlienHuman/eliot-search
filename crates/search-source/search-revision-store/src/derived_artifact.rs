//! Version-one derived objects, independent of raw source-history occurrences.
//!
//! Descriptors and decoded references are unqualified equality metadata. Only
//! actual owner verification and the shared immutable I/O kernel produce an I/O
//! receipt. This owner issues no key, nonce, counter, use permit or control ref.

#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use crate::{DomainBinding, residency_canonical_bytes};
use core::fmt;
use search_contracts::{
    Blake3Digest32, MaterializationId, NonZeroRevision, OpaqueId, ProfileId, RepresentationId,
    SourceRevisionRef,
};

mod codec;
mod io;
mod payload;
mod set;
pub use io::*;
pub use payload::*;
pub use set::*;

/// Hard bound on the complete canonical descriptor, including all six domains.
pub const MAX_DERIVED_DESCRIPTOR_BYTES: usize = 16 * 1024;
/// Maximum complete canonical selected-Current record supplied by its owner.
pub const MAX_DERIVED_CURRENT_BYTES: usize = 8192;
/// Maximum complete derived AAD; the existing selected-reference ceiling is retained.
pub const MAX_DERIVED_AAD_BYTES: usize = 32 * 1024;

/// Closed persisted artifact kinds. None aliases a raw revision object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactKindV1 {
    /// Actual canonical representation bytes; durable storage requires AEAD.
    RepresentationBody,
    /// C08 descriptors/maps/warnings, with source content referenced by digest.
    MaterializationMetadata,
    /// Owner-encoded unit manifest, including its actual versioned envelope.
    UnitManifest,
    /// Owner-encoded projection manifest; vector values remain outside this object.
    ProjectionManifest,
    /// C07's four-object immutable reference graph, without source bodies.
    SetManifest,
}
impl DerivedArtifactKindV1 {
    /// Exact closed wire label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepresentationBody => "representation-body",
            Self::MaterializationMetadata => "materialization-metadata",
            Self::UnitManifest => "unit-manifest",
            Self::ProjectionManifest => "projection-manifest",
            Self::SetManifest => "set-manifest",
        }
    }
    /// Whether an owner-verified plaintext metadata object is permissible in this profile.
    /// Root confidentiality policy may still require encryption.
    pub const fn is_metadata_only(self) -> bool {
        !matches!(self, Self::RepresentationBody)
    }
    pub(crate) fn parse(value: &str) -> Result<Self, DerivedArtifactErrorV1> {
        match value {
            "representation-body" => Ok(Self::RepresentationBody),
            "materialization-metadata" => Ok(Self::MaterializationMetadata),
            "unit-manifest" => Ok(Self::UnitManifest),
            "projection-manifest" => Ok(Self::ProjectionManifest),
            "set-manifest" => Ok(Self::SetManifest),
            _ => Err(DerivedArtifactErrorV1::ProfileInvalid),
        }
    }
}

/// Exact profile identity supplied by the producing owner, never an acceptance receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedOwnerProfileV1 {
    /// Actual materializer, unitizer, projection-profile-set or C07 set profile ID.
    pub id: ProfileId,
    /// Actual owner profile revision, independent of the artifact envelope version.
    pub revision: NonZeroRevision,
    /// Actual owner profile fingerprint; C07 never substitutes its byte digest for it.
    pub identity_digest: Blake3Digest32,
}

/// Candidate/decoded metadata has no cryptographic or durable qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactQualificationV1 {
    /// Actual producer/profile/AEAD and native readback have not been established.
    Unqualified,
}

/// Closed, content-free profile refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactErrorV1 {
    /// Unknown version, shape, kind, encoding or owner profile.
    ProfileInvalid,
    /// Root-supplied source/materialization/representation UUID is not RFC4122 v4.
    IdentityInvalid,
    /// Finite size/encoding budget was exceeded.
    BudgetExceeded,
    /// Exact encoded bytes, length, digest, descriptor or AAD differ.
    Mismatch,
    /// One or more complete domain meanings differ.
    ResidencyMismatch,
    /// Representation bytes cannot use the metadata-only storage path.
    PlaintextBodyForbidden,
}
impl fmt::Display for DerivedArtifactErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProfileInvalid => "DERIVED_ARTIFACT_PROFILE_INVALID",
            Self::IdentityInvalid => "DERIVED_ARTIFACT_IDENTITY_INVALID",
            Self::BudgetExceeded => "DERIVED_ARTIFACT_BUDGET_EXCEEDED",
            Self::Mismatch => "DERIVED_ARTIFACT_MISMATCH",
            Self::ResidencyMismatch => "DERIVED_ARTIFACT_RESIDENCY_MISMATCH",
            Self::PlaintextBodyForbidden => "DERIVED_ARTIFACT_PLAINTEXT_BODY_FORBIDDEN",
        })
    }
}
impl std::error::Error for DerivedArtifactErrorV1 {}

/// Immutable derived-object descriptor. The source reference is parent provenance
/// only; the encoded-body identity is calculated over the actual artifact bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactDescriptorV1 {
    pub(crate) kind: DerivedArtifactKindV1,
    pub(crate) source: SourceRevisionRef,
    pub(crate) materialization: MaterializationId,
    pub(crate) representation: RepresentationId,
    pub(crate) profile: DerivedOwnerProfileV1,
    pub(crate) semantic_digest: Blake3Digest32,
    pub(crate) encoded_digest: Blake3Digest32,
    pub(crate) encoded_bytes: u64,
    pub(crate) domains: [DomainBinding; 6],
    pub(crate) operation: OpaqueId,
}
impl fmt::Debug for DerivedArtifactDescriptorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedArtifactDescriptorV1")
            .field("kind", &self.kind)
            .field("encoded_bytes", &self.encoded_bytes)
            .finish_non_exhaustive()
    }
}
impl DerivedArtifactDescriptorV1 {
    /// Bind real parent/owner identities and hash the exact encoded artifact body.
    /// This pure bounded operation neither admits those identities nor writes bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn from_encoded(
        kind: DerivedArtifactKindV1,
        source: SourceRevisionRef,
        materialization: MaterializationId,
        representation: RepresentationId,
        profile: DerivedOwnerProfileV1,
        semantic_digest: Blake3Digest32,
        domains: [DomainBinding; 6],
        operation: OpaqueId,
        encoded: &[u8],
        maximum_bytes: usize,
    ) -> Result<Self, DerivedArtifactErrorV1> {
        let value = Self {
            kind,
            source,
            materialization,
            representation,
            profile,
            semantic_digest,
            encoded_digest: codec::hash_bytes(encoded, maximum_bytes)?,
            encoded_bytes: u64::try_from(encoded.len())
                .map_err(|_| DerivedArtifactErrorV1::BudgetExceeded)?,
            domains,
            operation,
        };
        value.validate()?;
        value.encode()?;
        Ok(value)
    }
    pub(crate) fn validate(&self) -> Result<(), DerivedArtifactErrorV1> {
        for id in [
            self.source.source_namespace_id.as_bytes(),
            self.source.source_id.as_bytes(),
            self.source.revision_id.as_bytes(),
            self.materialization.as_bytes(),
            self.representation.as_bytes(),
        ] {
            if id[6] & 0xf0 != 0x40 || id[8] & 0xc0 != 0x80 {
                return Err(DerivedArtifactErrorV1::IdentityInvalid);
            }
        }
        residency_canonical_bytes(&self.domains)
            .map_err(|_| DerivedArtifactErrorV1::ResidencyMismatch)?;
        Ok(())
    }
    /// Canonical version-one descriptor; contains source provenance metadata only.
    pub fn encode(&self) -> Result<Vec<u8>, DerivedArtifactErrorV1> {
        codec::encode_descriptor(self)
    }
    /// Strict decode and exact canonical parity. The result remains unqualified.
    pub fn decode(bytes: &[u8]) -> Result<Self, DerivedArtifactErrorV1> {
        codec::decode_descriptor(bytes)
    }
    /// No decoded or caller-built descriptor establishes owner or durable authority.
    pub const fn qualification(&self) -> DerivedArtifactQualificationV1 {
        DerivedArtifactQualificationV1::Unqualified
    }
    /// Closed artifact kind.
    pub const fn kind(&self) -> DerivedArtifactKindV1 {
        self.kind
    }
    /// Exact immutable parent source revision; never the derived object's identity.
    pub const fn source(&self) -> &SourceRevisionRef {
        &self.source
    }
    /// Genuine root-supplied materialization identity.
    pub const fn materialization_id(&self) -> MaterializationId {
        self.materialization
    }
    /// Genuine root-supplied representation identity, distinct from owner commitments.
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation
    }
    /// Actual producing owner's profile identity.
    pub const fn owner_profile(&self) -> &DerivedOwnerProfileV1 {
        &self.profile
    }
    /// Producing owner's semantic digest; it need not hash the persisted envelope.
    pub const fn owner_semantic_digest(&self) -> Blake3Digest32 {
        self.semantic_digest
    }
    /// Direct BLAKE3 over the exact encoded plaintext artifact body.
    pub const fn encoded_body_digest(&self) -> Blake3Digest32 {
        self.encoded_digest
    }
    /// Exact encoded plaintext artifact-body length.
    pub const fn encoded_body_bytes(&self) -> u64 {
        self.encoded_bytes
    }
    /// All six existing semantic domain bindings, in their canonical order.
    pub const fn domains(&self) -> &[DomainBinding; 6] {
        &self.domains
    }
    /// Original root operation; this is not a nonce permit or a phase reservation.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation
    }
    /// Check genuine IDs, full canonical bodies and tagged digests in all six domains.
    /// Equality is necessary for same-key reuse; this method never selects a key.
    pub fn require_same_residency(&self, other: &Self) -> Result<(), DerivedArtifactErrorV1> {
        for (left, right) in self.domains.iter().zip(&other.domains) {
            left.verify(right)
                .map_err(|_| DerivedArtifactErrorV1::ResidencyMismatch)?;
        }
        Ok(())
    }
    pub(crate) fn verify_body(
        &self,
        encoded: &[u8],
        maximum: usize,
    ) -> Result<(), DerivedArtifactErrorV1> {
        if encoded.len() as u64 != self.encoded_bytes
            || codec::hash_bytes(encoded, maximum)? != self.encoded_digest
        {
            return Err(DerivedArtifactErrorV1::Mismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
