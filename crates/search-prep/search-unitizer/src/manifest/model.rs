//! Durable unit-manifest data model and content-free receipts.

use crate::UnitizationLimits;
use search_contracts::{Blake3Digest32, DigestAlgorithm, NonZeroRevision, OpaqueId};

use super::profile::UnitizerProfileId;

/// Exact materializer provenance bound into a unit manifest.
///
/// Every digest is the true materializer output: representation identity,
/// canonical-text digest, coordinate-map digest and loss-map digest, plus the
/// exact bytes of the materializer profile identity. A content-free receipt
/// reference is never a substitute: verification compares these digests
/// against the live materializer product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializerProvenance {
    pub(super) materializer_profile_digest: [u8; 32],
    pub(super) representation_id: Blake3Digest32,
    pub(super) canonical_digest: Blake3Digest32,
    pub(super) coordinate_digest: Blake3Digest32,
    pub(super) loss_digest: Blake3Digest32,
}

impl MaterializerProvenance {
    /// Binds the exact materializer output digests for one representation.
    #[must_use]
    pub const fn new(
        materializer_profile_digest: [u8; 32],
        representation_id: Blake3Digest32,
        canonical_digest: Blake3Digest32,
        coordinate_digest: Blake3Digest32,
        loss_digest: Blake3Digest32,
    ) -> Self {
        Self {
            materializer_profile_digest,
            representation_id,
            canonical_digest,
            coordinate_digest,
            loss_digest,
        }
    }

    /// Exact bytes of the materializer profile identity.
    #[must_use]
    pub const fn materializer_profile_digest(&self) -> &[u8; 32] {
        &self.materializer_profile_digest
    }

    /// Deterministic representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Digest over the canonical representation bytes.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Digest over the serialized coordinate map.
    #[must_use]
    pub const fn coordinate_digest(&self) -> Blake3Digest32 {
        self.coordinate_digest
    }

    /// Digest over the serialized loss map.
    #[must_use]
    pub const fn loss_digest(&self) -> Blake3Digest32 {
        self.loss_digest
    }
}

/// Immutable descriptor for one ordered unit occurrence.
///
/// Carries spans, line attachment, boundary flags and the unit identity
/// digest only. Source text, paths, ranking scores and vendor payloads are
/// forbidden inputs and are never stored here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitDescriptor {
    pub(super) ordinal: u64,
    pub(super) source_start: u64,
    pub(super) source_end: u64,
    pub(super) logical_line_start: u64,
    pub(super) logical_line_end: u64,
    pub(super) starts_at_line_boundary: bool,
    pub(super) ends_at_line_boundary: bool,
    pub(super) unit_digest: Blake3Digest32,
}

impl UnitDescriptor {
    /// Zero-based unit ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Inclusive exact source-byte start.
    #[must_use]
    pub const fn source_start(&self) -> u64 {
        self.source_start
    }

    /// Exclusive exact source-byte end.
    #[must_use]
    pub const fn source_end(&self) -> u64 {
        self.source_end
    }

    /// Inclusive zero-based logical line index.
    #[must_use]
    pub const fn logical_line_start(&self) -> u64 {
        self.logical_line_start
    }

    /// Exclusive zero-based logical line index.
    #[must_use]
    pub const fn logical_line_end(&self) -> u64 {
        self.logical_line_end
    }

    /// Whether the unit starts at an exact logical-line boundary.
    #[must_use]
    pub const fn starts_at_line_boundary(&self) -> bool {
        self.starts_at_line_boundary
    }

    /// Whether the unit ends at an exact logical-line boundary.
    #[must_use]
    pub const fn ends_at_line_boundary(&self) -> bool {
        self.ends_at_line_boundary
    }

    /// Domain-separated unit identity digest.
    #[must_use]
    pub const fn unit_digest(&self) -> Blake3Digest32 {
        self.unit_digest
    }
}

/// Immutable durable unit manifest: ordered unit descriptors plus the exact
/// source, representation, materializer and unitizer binding under one
/// explicit digest algorithm.
///
/// The manifest stores no source bodies and no ranking data. Persistence
/// belongs to the revision store; this type owns the data and verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitManifest {
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) content_digest: Blake3Digest32,
    pub(super) representation_id: Blake3Digest32,
    pub(super) materializer_profile_digest: [u8; 32],
    pub(super) canonical_digest: Blake3Digest32,
    pub(super) coordinate_digest: Blake3Digest32,
    pub(super) loss_digest: Blake3Digest32,
    pub(super) unitizer_profile_id: UnitizerProfileId,
    pub(super) unitizer_profile_revision: u64,
    pub(super) unitizer_limits: UnitizationLimits,
    pub(super) digest_algorithm: DigestAlgorithm,
    pub(super) input_bytes: u64,
    pub(super) emitted_bytes: u64,
    pub(super) line_count: u64,
    pub(super) units: Vec<UnitDescriptor>,
    pub(super) manifest_digest: Blake3Digest32,
}

impl UnitManifest {
    /// Stable source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Retained source revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Exact source content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    /// Deterministic representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Exact bytes of the materializer profile identity.
    #[must_use]
    pub const fn materializer_profile_digest(&self) -> &[u8; 32] {
        &self.materializer_profile_digest
    }

    /// Digest over the canonical representation bytes.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Digest over the serialized coordinate map.
    #[must_use]
    pub const fn coordinate_digest(&self) -> Blake3Digest32 {
        self.coordinate_digest
    }

    /// Digest over the serialized loss map.
    #[must_use]
    pub const fn loss_digest(&self) -> Blake3Digest32 {
        self.loss_digest
    }

    /// Bound unitizer profile identity.
    #[must_use]
    pub const fn unitizer_profile_id(&self) -> UnitizerProfileId {
        self.unitizer_profile_id
    }

    /// Bound unitizer profile revision.
    #[must_use]
    pub const fn unitizer_profile_revision(&self) -> u64 {
        self.unitizer_profile_revision
    }

    /// Bound unitizer finite limits.
    #[must_use]
    pub const fn unitizer_limits(&self) -> UnitizationLimits {
        self.unitizer_limits
    }

    /// Exact digest algorithm bound into this manifest.
    #[must_use]
    pub const fn digest_algorithm(&self) -> DigestAlgorithm {
        self.digest_algorithm
    }

    /// Exact input bytes covered by this manifest.
    #[must_use]
    pub const fn input_bytes(&self) -> u64 {
        self.input_bytes
    }

    /// Exact bytes covered by emitted units; always equals the input bytes.
    #[must_use]
    pub const fn emitted_bytes(&self) -> u64 {
        self.emitted_bytes
    }

    /// Number of exact logical lines.
    #[must_use]
    pub const fn line_count(&self) -> u64 {
        self.line_count
    }

    /// Ordered unit descriptors.
    #[must_use]
    pub fn units(&self) -> &[UnitDescriptor] {
        &self.units
    }

    /// Number of emitted units.
    #[must_use]
    pub const fn unit_count(&self) -> usize {
        self.units.len()
    }

    /// Domain-separated digest over the canonical manifest bytes.
    #[must_use]
    pub const fn manifest_digest(&self) -> Blake3Digest32 {
        self.manifest_digest
    }
}

/// Deterministic canonical bytes for one durable manifest.
///
/// Source content travels by digest only: technical receipts never embed
/// content or paths.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalUnitManifestBytes {
    pub(super) bytes: Vec<u8>,
}

impl CanonicalUnitManifestBytes {
    /// Canonical bytes borrowed without copying.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Canonical byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the canonical bytes are empty (never for valid output).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl core::fmt::Debug for CanonicalUnitManifestBytes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CanonicalUnitManifestBytes")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

/// Content-free verification receipt: the manifest belongs to the exact
/// source revision, representation and profiles. It cannot prove current
/// filesystem state or indexed publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitManifestVerificationReceipt {
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) representation_id: Blake3Digest32,
    pub(super) unitizer_profile_id: UnitizerProfileId,
    pub(super) materializer_profile_digest: [u8; 32],
    pub(super) unit_count: u64,
    pub(super) manifest_digest: Blake3Digest32,
}

impl UnitManifestVerificationReceipt {
    /// Verified source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Verified retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Verified representation identity.
    #[must_use]
    pub const fn representation_id(&self) -> Blake3Digest32 {
        self.representation_id
    }

    /// Verified unitizer profile identity.
    #[must_use]
    pub const fn unitizer_profile_id(&self) -> UnitizerProfileId {
        self.unitizer_profile_id
    }

    /// Verified materializer profile-digest bytes.
    #[must_use]
    pub const fn materializer_profile_digest(&self) -> &[u8; 32] {
        &self.materializer_profile_digest
    }

    /// Verified unit count.
    #[must_use]
    pub const fn unit_count(&self) -> u64 {
        self.unit_count
    }

    /// Verified manifest digest.
    #[must_use]
    pub const fn manifest_digest(&self) -> Blake3Digest32 {
        self.manifest_digest
    }
}

/// Exact unit-identity difference between two manifests.
///
/// Retained requires exact unit-identity digest equality; heuristic
/// span or name similarity can never retain a unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitManifestDiff {
    pub(super) old_digest: Blake3Digest32,
    pub(super) new_digest: Blake3Digest32,
    pub(super) retained: Vec<Blake3Digest32>,
    pub(super) created: Vec<Blake3Digest32>,
    pub(super) retired: Vec<Blake3Digest32>,
}

impl UnitManifestDiff {
    /// Manifest digest of the old side.
    #[must_use]
    pub const fn old_digest(&self) -> Blake3Digest32 {
        self.old_digest
    }

    /// Manifest digest of the new side.
    #[must_use]
    pub const fn new_digest(&self) -> Blake3Digest32 {
        self.new_digest
    }

    /// Unit identities present on both sides.
    #[must_use]
    pub fn retained(&self) -> &[Blake3Digest32] {
        &self.retained
    }

    /// Unit identities present only on the new side.
    #[must_use]
    pub fn created(&self) -> &[Blake3Digest32] {
        &self.created
    }

    /// Unit identities present only on the old side.
    #[must_use]
    pub fn retired(&self) -> &[Blake3Digest32] {
        &self.retired
    }

    /// Reports whether both sides carry identical unit identities.
    #[must_use]
    pub const fn is_identical(&self) -> bool {
        self.created.is_empty() && self.retired.is_empty()
    }
}
