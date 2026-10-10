//! Immutable v3 occurrence metadata and opaque verified output.

#![forbid(unsafe_code)]

use search_contracts::{
    Blake3Digest32, MaterializationId, ProfileId, Representation, RepresentationId, SourceId,
    SourceNamespaceId, SourceRevisionId, UnitId, UnitOccurrence,
};

/// Exact source and representation coordinates of one unit manifest.
///
/// Admission supplies these identifiers. This record contains no path or access grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V3SourceBinding {
    /// Admitted source namespace.
    pub source_namespace_id: SourceNamespaceId,
    /// Admitted source within the namespace.
    pub source_id: SourceId,
    /// Admitted retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Deterministic materialization identity of the admitted revision.
    pub materialization_id: MaterializationId,
    /// Deterministic representation identity bound to the unitizer profile.
    pub representation_id: RepresentationId,
}

/// Materializer provenance bound into one v3 manifest.
///
/// The materializer's full representation commitment remains a digest, never a UUID.
/// No field carries source text, paths or ranking state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializerProvenance {
    /// Admitted source and representation coordinates of this provenance.
    pub(crate) binding: V3SourceBinding,
    /// Materializer representation commitment over the immutable revision.
    pub(crate) materializer_commitment: Blake3Digest32,
    /// Exact bytes of the materializer profile identity.
    pub(crate) materializer_profile_digest: Blake3Digest32,
    /// Digest over the canonical materialized object bytes.
    pub(crate) canonical_digest: Blake3Digest32,
    /// Digest over the serialized native coordinate map.
    pub(crate) coordinate_digest: Blake3Digest32,
    /// Digest over the serialized loss map.
    pub(crate) loss_digest: Blake3Digest32,
    /// Digest over the exact admitted input bytes.
    pub(crate) input_digest: Blake3Digest32,
    /// Admitted input byte length covered by this provenance.
    pub(crate) native_bytes: u64,
    /// Canonical materialized byte length.
    pub(crate) canonical_bytes: u64,
    /// Monotone legacy revision sequence this provenance admits.
    pub(crate) legacy_revision_sequence: u64,
}

impl MaterializerProvenance {
    /// Admitted source and representation coordinates of this provenance.
    ///
    /// Admission owns this binding; the unitizer only records it.
    #[must_use]
    pub const fn binding(&self) -> &V3SourceBinding {
        &self.binding
    }

    /// Materializer representation commitment over the immutable revision.
    ///
    /// A content digest. It is not a UUID, must not be rendered or parsed as
    /// one, and must not be reused as point identity.
    #[must_use]
    pub const fn commitment(&self) -> Blake3Digest32 {
        self.materializer_commitment
    }

    /// Exact bytes of the materializer profile identity.
    #[must_use]
    pub const fn materializer_profile_digest(&self) -> Blake3Digest32 {
        self.materializer_profile_digest
    }

    /// Digest over the canonical materialized object bytes.
    #[must_use]
    pub const fn canonical_digest(&self) -> Blake3Digest32 {
        self.canonical_digest
    }

    /// Digest over the serialized native coordinate map.
    #[must_use]
    pub const fn coordinate_digest(&self) -> Blake3Digest32 {
        self.coordinate_digest
    }

    /// Digest over the serialized loss map.
    #[must_use]
    pub const fn loss_digest(&self) -> Blake3Digest32 {
        self.loss_digest
    }

    /// Digest over the exact admitted input bytes.
    #[must_use]
    pub const fn input_digest(&self) -> Blake3Digest32 {
        self.input_digest
    }

    /// Admitted input byte length covered by this provenance.
    #[must_use]
    pub const fn native_bytes(&self) -> u64 {
        self.native_bytes
    }

    /// Canonical materialized byte length.
    #[must_use]
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }

    /// Monotone legacy revision sequence this provenance admits.
    #[must_use]
    pub const fn legacy_revision_sequence(&self) -> u64 {
        self.legacy_revision_sequence
    }
}

/// One ordered unit occurrence with its exact source coordinates.
///
/// The occurrence carries the contract `UnitId`, `UnitKind` and `NativeAnchor`
/// that downstream projection planning consumes. Span and line fields are the
/// canonical byte and logical-line bounds of the occurrence; the three digests
/// keep the unit content, the coordinate reference and the occurrence identity
/// distinct so a collision in one cannot masquerade as agreement in another.
/// No field holds source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitDescriptor {
    /// Contract occurrence identity, kind, ordinal and native anchor.
    pub(crate) occurrence: UnitOccurrence,
    /// Inclusive exact canonical source-byte start.
    pub(crate) source_start: u64,
    /// Exclusive exact canonical source-byte end.
    pub(crate) source_end: u64,
    /// Inclusive zero-based logical line index.
    pub(crate) logical_line_start: u64,
    /// Exclusive zero-based logical line index.
    pub(crate) logical_line_end: u64,
    /// Whether the occurrence starts at an exact logical-line boundary.
    pub(crate) starts_at_line_boundary: bool,
    /// Whether the occurrence ends at an exact logical-line boundary.
    pub(crate) ends_at_line_boundary: bool,
    /// Digest over the exact unit content bytes.
    pub(crate) unit_content_digest: Blake3Digest32,
    /// Digest over the coordinate reference for this occurrence.
    pub(crate) reference_digest: Blake3Digest32,
    /// Digest binding occurrence identity, ordinal, span and coordinate basis.
    pub(crate) identity_digest: Blake3Digest32,
}

impl UnitDescriptor {
    /// Contract occurrence identity, kind, ordinal and native anchor.
    #[must_use]
    pub const fn occurrence(&self) -> &UnitOccurrence {
        &self.occurrence
    }

    /// Contract unit identity of this occurrence.
    #[must_use]
    pub const fn unit_id(&self) -> UnitId {
        self.occurrence.unit_id
    }

    /// Inclusive exact canonical source-byte start.
    #[must_use]
    pub const fn source_start(&self) -> u64 {
        self.source_start
    }

    /// Exclusive exact canonical source-byte end.
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

    /// Whether the occurrence starts at an exact logical-line boundary.
    #[must_use]
    pub const fn starts_at_line_boundary(&self) -> bool {
        self.starts_at_line_boundary
    }

    /// Whether the occurrence ends at an exact logical-line boundary.
    #[must_use]
    pub const fn ends_at_line_boundary(&self) -> bool {
        self.ends_at_line_boundary
    }

    /// Digest over the exact unit content bytes.
    #[must_use]
    pub const fn unit_content_digest(&self) -> Blake3Digest32 {
        self.unit_content_digest
    }

    /// Digest over the coordinate reference for this occurrence.
    #[must_use]
    pub const fn reference_digest(&self) -> Blake3Digest32 {
        self.reference_digest
    }

    /// Digest binding occurrence identity, ordinal, span and coordinate basis.
    #[must_use]
    pub const fn identity_digest(&self) -> Blake3Digest32 {
        self.identity_digest
    }
}

/// Unsealed v3 manifest: provenance, profile binding and the complete ordered
/// unit set with explicit accounting.
///
/// `represented_bytes` is the byte total the occurrences cover and
/// `omitted_bytes` is the total the admitted profile permits to be skipped;
/// both are required so completeness is an exact account rather than an
/// assertion. `units` is the complete ordered occurrence list in canonical
/// order. A body without complete accounting cannot become a manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestBody {
    /// Materializer provenance for the exact admitted revision.
    pub(crate) provenance: MaterializerProvenance,
    /// Admitted unitizer profile descriptor under which the units were cut.
    pub(crate) profile: super::v3_profile::V3UnitizerProfileDescriptor,
    /// Canonical identity of the admitted unitizer profile.
    pub(crate) profile_id: ProfileId,
    /// Exact input byte length covered by this manifest.
    pub(crate) input_bytes: u64,
    /// Byte total covered by the emitted occurrences.
    pub(crate) represented_bytes: u64,
    /// Byte total the admitted profile permits to be omitted.
    pub(crate) omitted_bytes: u64,
    /// Number of exact logical lines in the admitted input.
    pub(crate) line_count: u64,
    /// Complete ordered occurrence descriptors in canonical order.
    pub(crate) units: Vec<UnitDescriptor>,
}

/// Sealed durable v3 manifest: the body plus its canonical digest.
///
/// The digest covers the canonical form of the whole body, so the body cannot
/// be edited after sealing. An ordinary constructor is intentionally absent:
/// only the package verifier can seal a body, which is what keeps a caller
/// from minting a manifest digest over an unverified unit list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitManifest {
    /// Sealed manifest body with complete accounting.
    pub(crate) body: ManifestBody,
    /// Canonical digest over the sealed body.
    pub(crate) manifest_digest: Blake3Digest32,
}

impl UnitManifest {
    /// Sealed manifest body with complete accounting.
    #[must_use]
    pub const fn provenance(&self) -> &MaterializerProvenance {
        &self.body.provenance
    }

    /// Admitted unitizer profile descriptor.
    #[must_use]
    pub const fn profile(&self) -> &super::v3_profile::V3UnitizerProfileDescriptor {
        &self.body.profile
    }

    /// Canonical identity of the admitted unitizer profile.
    #[must_use]
    pub const fn profile_id(&self) -> &ProfileId {
        &self.body.profile_id
    }

    /// Complete ordered occurrence descriptors in canonical order.
    #[must_use]
    pub fn units(&self) -> &[UnitDescriptor] {
        &self.body.units
    }

    /// Number of emitted occurrences.
    #[must_use]
    pub const fn unit_count(&self) -> usize {
        self.body.units.len()
    }

    /// Exact input byte length covered by this manifest.
    #[must_use]
    pub const fn input_bytes(&self) -> u64 {
        self.body.input_bytes
    }

    /// Byte total covered by the emitted occurrences.
    #[must_use]
    pub const fn emitted_bytes(&self) -> u64 {
        self.body.represented_bytes
    }

    /// Byte total the admitted profile permits to be omitted.
    #[must_use]
    pub const fn omitted_bytes(&self) -> u64 {
        self.body.omitted_bytes
    }

    /// Canonical digest over the sealed body.
    #[must_use]
    pub const fn manifest_digest(&self) -> Blake3Digest32 {
        self.manifest_digest
    }
}

/// One package-verified complete ordered `UnitSet` bound to its representation.
///
/// Only package construction or exact-source verification produces this value.
/// Its digest equals `Representation.unit_manifest_digest`; callers cannot
/// substitute a list, count or digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedUnitSet {
    /// Sealed verified manifest for this occurrence set.
    pub(crate) manifest: UnitManifest,
    /// Representation the sealed manifest digest is bound into.
    pub(crate) representation: Representation,
}

impl VerifiedUnitSet {
    /// Sealed verified manifest for this occurrence set.
    #[must_use]
    pub const fn manifest(&self) -> &UnitManifest {
        &self.manifest
    }

    /// Representation the sealed manifest digest is bound into.
    #[must_use]
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }

    /// Complete ordered occurrence descriptors in canonical order.
    #[must_use]
    pub fn units(&self) -> &[UnitDescriptor] {
        self.manifest.units()
    }

    /// Canonical digest over the sealed body, equal to the representation bind.
    #[must_use]
    pub const fn manifest_digest(&self) -> Blake3Digest32 {
        self.manifest.manifest_digest
    }
}

/// Exact canonical bytes of one sealed manifest.
///
/// Durable v3 bytes carry no source-verification authority by themselves.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalUnitManifestBytes {
    pub(crate) bytes: Vec<u8>,
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

    /// Reports whether the canonical bytes are empty.
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

/// Exact unit-identity difference between two sealed manifests.
///
/// Retained requires exact contract `UnitId` equality; span, name or digest
/// similarity never retains a unit. Ordering the three lists is the diff
/// owner's job; this type only carries the exact identity classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitManifestDiff {
    /// Manifest digest of the old side.
    pub old_digest: Blake3Digest32,
    /// Manifest digest of the new side.
    pub new_digest: Blake3Digest32,
    /// Contract unit identities present only on the new side.
    pub created: Vec<UnitId>,
    /// Contract unit identities present on both sides.
    pub retained: Vec<UnitId>,
    /// Contract unit identities present only on the old side.
    pub retired: Vec<UnitId>,
}

/// Content-free verification receipt for one verified occurrence set.
///
/// The receipt names the exact manifest, representation, profile and unit
/// count a verification covered. It cannot prove current filesystem state or
/// indexed publication, and it holds no source body, so it grants no authority
/// beyond the verification it reports. It is an alias rather than a second
/// type so one verified set cannot be mistaken for two distinct attestations.
pub type UnitManifestVerificationReceipt = VerifiedUnitSet;
