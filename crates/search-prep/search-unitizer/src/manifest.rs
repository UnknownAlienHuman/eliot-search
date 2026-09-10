//! Durable profile-bound unit manifests with exact provenance.
//!
//! A manifest binds one source revision, one materializer representation
//! (representation ID plus canonical, coordinate and loss digests and the
//! exact materializer profile-digest bytes), one validated unitizer profile
//! revision and every ordered unit descriptor under one explicit digest
//! algorithm. It carries digests and spans only, never source text, so the
//! revision store can persist the canonical bytes while this package owns the
//! data and its verification. Storage, admission and publication belong
//! elsewhere: [`decode_unit_manifest`] only parses durable bytes and
//! [`verify_unit_manifest`] reproves provenance against live inputs.
//!
//! Digest bytes are domain-separated identity digests computed by the local
//! [`digest32`] construction (four FNV-1a lanes over the domain, a separator
//! and each length-delimited chunk, fixed little-endian lane encoding). The
//! manifest records its [`DigestAlgorithm`] explicitly and verification
//! rejects any other algorithm instead of reinterpreting digest bytes.

use crate::{UnitizationError, UnitizationInput, UnitizationLimits};
use search_contracts::{Blake3Digest32, DigestAlgorithm, NonZeroRevision, OpaqueId};

/// Durable unit-manifest format identity. Changing boundary semantics,
/// serialization or digest domains requires a new identity; saved manifests
/// are never reinterpreted under a changed format.
pub const UNIT_MANIFEST_FORMAT: &str = "exact-unit-manifest/v1";
/// Durable unit-manifest codec version.
pub const UNIT_MANIFEST_VERSION: u16 = 1;
/// The sole digest algorithm a durable manifest may bind. Any other algorithm
/// is rejected instead of reinterpreted.
pub const UNIT_MANIFEST_DIGEST_ALGORITHM: DigestAlgorithm = DigestAlgorithm::Blake3_256;
/// Maximum unitizer profile-name length in bytes.
pub const MAX_UNITIZER_PROFILE_NAME_BYTES: usize = 128;

const MAGIC: &[u8; 8] = b"ELSUMF01";
const PROFILE_DOMAIN: &[u8] = b"eliot-search/unitizer/profile/v1";
const UNIT_DOMAIN: &[u8] = b"eliot-search/unitizer/unit/v1";
const MANIFEST_DOMAIN: &[u8] = b"eliot-search/unitizer/manifest/v1";
const UNIT_CODEC_BYTES: usize = 73;
const DIGEST_TRAILER_BYTES: usize = 32;

/// Domain-separated 32-byte digest over an ordered byte preimage.
///
/// Four independent FNV-1a 64-bit lanes absorb the domain, a domain
/// separator, then each length-delimited chunk. Fixed little-endian lane
/// encoding keeps the digest byte-identical across runs and platforms. This
/// is an identity digest, not a cryptographic commitment over source bytes.
fn digest32(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    const SEED: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x4822_2325_cbf2_9ce4,
        0x2325_cbf2_9ce4_8422,
    ];
    const PRIME: u64 = 0x1_0000_0000_01B3;
    let mut lanes = SEED;
    let mut index = 0_usize;
    let mut absorb = |byte: u8| {
        let slot = index % 4;
        lanes[slot] ^= u64::from(byte);
        lanes[slot] = lanes[slot].wrapping_mul(PRIME);
        index = index.wrapping_add(1);
    };
    for byte in domain {
        absorb(*byte);
    }
    absorb(0xFF);
    for chunk in chunks {
        for byte in *chunk {
            absorb(*byte);
        }
        absorb(0xFE);
    }
    let mut out = [0_u8; 32];
    for (slot, lane) in lanes.iter().enumerate() {
        let start = slot * 8;
        out[start..start + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

/// Canonical unitizer-profile identity: the domain-separated digest over the
/// profile name, revision, finite limits and boundary-format identity.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitizerProfileId([u8; 32]);

impl UnitizerProfileId {
    /// Rebuilds an identity from its 32 raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Raw identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Debug for UnitizerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("UnitizerProfileId")
            .field(&self.to_string())
            .finish()
    }
}

impl core::fmt::Display for UnitizerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Unvalidated unitizer-profile descriptor: name, monotone revision and the
/// finite limits that own every boundary decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitizerProfileDescriptor {
    /// Human-readable profile name (ASCII alphanumeric plus `-_.:`).
    pub profile_name: String,
    /// Monotone profile revision; zero is rejected.
    pub profile_revision: u64,
    /// Finite profile-owned unitization limits.
    pub limits: UnitizationLimits,
}

/// Validated unitizer profile with a bound canonical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedUnitizerProfile {
    name: String,
    revision: u64,
    limits: UnitizationLimits,
    id: UnitizerProfileId,
}

impl ValidatedUnitizerProfile {
    /// Canonical profile identity.
    #[must_use]
    pub const fn id(&self) -> UnitizerProfileId {
        self.id
    }

    /// Monotone profile revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Human-readable profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Finite profile-owned limits.
    #[must_use]
    pub const fn limits(&self) -> UnitizationLimits {
        self.limits
    }
}

/// Validates a unitizer-profile descriptor and binds its canonical identity.
///
/// Rejects empty, overlong or non-explicit names, zero revisions and
/// invalid limits. Implicit language or size defaults can never pass: every
/// load-bearing behavior is explicit in the descriptor.
pub fn validate_unitizer_profile(
    descriptor: &UnitizerProfileDescriptor,
) -> Result<ValidatedUnitizerProfile, UnitizationError> {
    if descriptor.profile_name.is_empty()
        || descriptor.profile_name.len() > MAX_UNITIZER_PROFILE_NAME_BYTES
        || !descriptor
            .profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(UnitizationError::UnitizerProfileInvalid);
    }
    if descriptor.profile_revision == 0 {
        return Err(UnitizationError::UnitizerProfileInvalid);
    }
    let limits = descriptor.limits.validate()?;
    let profile = ValidatedUnitizerProfile {
        name: descriptor.profile_name.clone(),
        revision: descriptor.profile_revision,
        limits,
        id: UnitizerProfileId::from_bytes([0; 32]),
    };
    let id = unitizer_profile_digest(&profile);
    Ok(ValidatedUnitizerProfile { id, ..profile })
}

/// Domain-separated canonical digest over the full profile identity.
///
/// Covers profile name, revision, finite limits and boundary-format identity.
/// Any load-bearing change yields a different identity, so existing manifests
/// are never reinterpreted under a changed profile.
#[must_use]
pub fn unitizer_profile_digest(profile: &ValidatedUnitizerProfile) -> UnitizerProfileId {
    let limits = profile.limits;
    UnitizerProfileId::from_bytes(digest32(
        PROFILE_DOMAIN,
        &[
            profile.name.as_bytes(),
            &profile.revision.to_le_bytes(),
            &limits.max_input_bytes.to_le_bytes(),
            &limits.preferred_unit_bytes.to_le_bytes(),
            &limits.max_unit_bytes.to_le_bytes(),
            &limits.max_lines.to_le_bytes(),
            &limits.max_units.to_le_bytes(),
            crate::UnitizationLimits::LAYOUT_FORMAT.as_bytes(),
        ],
    ))
}

/// Profile-change classification. Existing unit IDs and manifests are never
/// reinterpreted under a changed profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnitizerProfileChange {
    /// Identical canonical identity: nothing to redo.
    Noop,
    /// Same profile name at a monotone revision: reunitize from the exact
    /// representation and reproject downstream coordinates.
    ReunitizeAndReproject,
    /// Renamed profile, decreased revision or identity change outside a
    /// monotone revision: refuse to reinterpret existing manifests.
    Reject,
}

/// Classifies a unitizer-profile transition without reinterpreting history.
#[must_use]
pub fn classify_unitizer_profile_change(
    old: &ValidatedUnitizerProfile,
    new: &ValidatedUnitizerProfile,
) -> UnitizerProfileChange {
    if old.id() == new.id() {
        UnitizerProfileChange::Noop
    } else if old.name() == new.name() && new.revision() > old.revision() {
        UnitizerProfileChange::ReunitizeAndReproject
    } else {
        UnitizerProfileChange::Reject
    }
}

/// Exact materializer provenance bound into a unit manifest.
///
/// Every digest is the true materializer output: representation identity,
/// canonical-text digest, coordinate-map digest and loss-map digest, plus the
/// exact bytes of the materializer profile identity. A content-free receipt
/// reference is never a substitute: verification compares these digests
/// against the live materializer product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializerProvenance {
    materializer_profile_digest: [u8; 32],
    representation_id: Blake3Digest32,
    canonical_digest: Blake3Digest32,
    coordinate_digest: Blake3Digest32,
    loss_digest: Blake3Digest32,
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
    ordinal: u64,
    source_start: u64,
    source_end: u64,
    logical_line_start: u64,
    logical_line_end: u64,
    starts_at_line_boundary: bool,
    ends_at_line_boundary: bool,
    unit_digest: Blake3Digest32,
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
    source_id: OpaqueId,
    revision: NonZeroRevision,
    content_digest: Blake3Digest32,
    representation_id: Blake3Digest32,
    materializer_profile_digest: [u8; 32],
    canonical_digest: Blake3Digest32,
    coordinate_digest: Blake3Digest32,
    loss_digest: Blake3Digest32,
    unitizer_profile_id: UnitizerProfileId,
    unitizer_profile_revision: u64,
    unitizer_limits: UnitizationLimits,
    digest_algorithm: DigestAlgorithm,
    input_bytes: u64,
    emitted_bytes: u64,
    line_count: u64,
    units: Vec<UnitDescriptor>,
    manifest_digest: Blake3Digest32,
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
    bytes: Vec<u8>,
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
    source_id: OpaqueId,
    revision: NonZeroRevision,
    representation_id: Blake3Digest32,
    unitizer_profile_id: UnitizerProfileId,
    materializer_profile_digest: [u8; 32],
    unit_count: u64,
    manifest_digest: Blake3Digest32,
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
    old_digest: Blake3Digest32,
    new_digest: Blake3Digest32,
    retained: Vec<Blake3Digest32>,
    created: Vec<Blake3Digest32>,
    retired: Vec<Blake3Digest32>,
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

const fn digest_tag(algorithm: DigestAlgorithm) -> u8 {
    match algorithm {
        DigestAlgorithm::Blake3_256 => 1,
        DigestAlgorithm::Sha256 => 2,
    }
}

const fn parse_digest_tag(value: u8) -> Result<DigestAlgorithm, UnitizationError> {
    match value {
        1 => Ok(DigestAlgorithm::Blake3_256),
        2 => Ok(DigestAlgorithm::Sha256),
        _ => Err(UnitizationError::UnitManifestDigestMismatch),
    }
}

/// Domain-separated unit identity over source revision, representation ID,
/// unitizer profile, occurrence ordinal, kind-neutral span and load-bearing
/// structural hints. Path, display text and ranking scores are forbidden
/// inputs. A changed representation, profile or occurrence identity changes
/// the unit ID; no semantic identity across revisions is claimed.
fn derive_unit_digest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    ordinal: u64,
    span: &crate::UnitSpan,
) -> Result<Blake3Digest32, UnitizationError> {
    let start = u64::try_from(span.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
    let end = u64::try_from(span.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
    Ok(Blake3Digest32::from_bytes(digest32(
        UNIT_DOMAIN,
        &[
            input.source_id.as_str().as_bytes(),
            &input.revision.get().to_le_bytes(),
            provenance.representation_id.as_bytes(),
            profile.id.as_bytes(),
            &ordinal.to_le_bytes(),
            &start.to_le_bytes(),
            &end.to_le_bytes(),
            &span.logical_line_start.to_le_bytes(),
            &span.logical_line_end.to_le_bytes(),
            &[
                u8::from(span.starts_at_line_boundary),
                u8::from(span.ends_at_line_boundary),
            ],
        ],
    )))
}

fn assemble_manifest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    digest_algorithm: DigestAlgorithm,
) -> Result<UnitManifest, UnitizationError> {
    if digest_algorithm != UNIT_MANIFEST_DIGEST_ALGORITHM {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if unitizer_profile_digest(profile) != profile.id {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if input.is_empty() {
        return Err(UnitizationError::EmptyInput);
    }
    let spans = super::unitize_text(input.text(), &input.lines, profile.limits)?;
    if spans.is_empty() {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let input_bytes = u64::try_from(input.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let line_count =
        u64::try_from(input.lines.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let mut units = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        let ordinal = u64::try_from(index).map_err(|_| UnitizationError::OffsetOverflow)?;
        let start =
            u64::try_from(span.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
        let end = u64::try_from(span.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        let unit_digest = derive_unit_digest(input, provenance, profile, ordinal, span)?;
        units.push(UnitDescriptor {
            ordinal,
            source_start: start,
            source_end: end,
            logical_line_start: span.logical_line_start,
            logical_line_end: span.logical_line_end,
            starts_at_line_boundary: span.starts_at_line_boundary,
            ends_at_line_boundary: span.ends_at_line_boundary,
            unit_digest,
        });
    }
    let mut ordered: Vec<Blake3Digest32> = units.iter().map(UnitDescriptor::unit_digest).collect();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(UnitizationError::UnitizationNondeterministic);
    }
    let mut manifest = UnitManifest {
        source_id: input.source_id.clone(),
        revision: input.revision,
        content_digest: input.content_digest,
        representation_id: provenance.representation_id,
        materializer_profile_digest: provenance.materializer_profile_digest,
        canonical_digest: provenance.canonical_digest,
        coordinate_digest: provenance.coordinate_digest,
        loss_digest: provenance.loss_digest,
        unitizer_profile_id: profile.id,
        unitizer_profile_revision: profile.revision,
        unitizer_limits: profile.limits,
        digest_algorithm,
        input_bytes,
        emitted_bytes: input_bytes,
        line_count,
        units,
        manifest_digest: Blake3Digest32::from_bytes([0; 32]),
    };
    let mut body = Vec::new();
    encode_body(&manifest, &mut body)?;
    manifest.manifest_digest = Blake3Digest32::from_bytes(digest32(MANIFEST_DOMAIN, &[&body]));
    Ok(manifest)
}

/// Builds every occurrence in canonical order with full binding.
///
/// Validates finite counts, unique unit identities, profile-compliant sizes,
/// complete accounting of the represented bytes and the exact source,
/// representation, materializer and unitizer binding.
///
/// Success contains immutable unit descriptors and digests, not source
/// bodies or ranking data. Cancellation and budget exhaustion surface as
/// typed errors through boundary scanning; they never yield a successful
/// complete manifest.
pub fn build_unit_manifest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    digest_algorithm: DigestAlgorithm,
    max_encoded_bytes: usize,
) -> Result<UnitManifest, UnitizationError> {
    let manifest = assemble_manifest(input, provenance, profile, digest_algorithm)?;
    if encoded_size(manifest.source_id.as_str().len(), manifest.units.len())? > max_encoded_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    Ok(manifest)
}

/// Serializes schema, profile, source, representation and map identities plus
/// the ordered unit descriptors and counts deterministically.
pub fn canonicalize_unit_manifest(
    manifest: &UnitManifest,
) -> Result<CanonicalUnitManifestBytes, UnitizationError> {
    let mut out = Vec::with_capacity(encoded_size(
        manifest.source_id.as_str().len(),
        manifest.units.len(),
    )?);
    encode_body(manifest, &mut out)?;
    out.extend_from_slice(manifest.manifest_digest.as_bytes());
    Ok(CanonicalUnitManifestBytes { bytes: out })
}

/// Domain-separated digest over the canonical manifest bytes.
#[must_use]
pub const fn manifest_digest(manifest: &UnitManifest) -> Blake3Digest32 {
    manifest.manifest_digest
}

/// Parses durable manifest bytes without source text or profile state.
///
/// Structural defects (magic, version, lengths, counts, ordering, encoded
/// limits) fail closed; an unknown digest-algorithm tag or a mismatched
/// digest trailer fails as a digest mismatch instead of being reinterpreted.
/// Full provenance still requires [`verify_unit_manifest`].
pub fn decode_unit_manifest(
    bytes: &[u8],
    max_encoded_bytes: usize,
) -> Result<UnitManifest, UnitizationError> {
    if bytes.len() > max_encoded_bytes || bytes.len() < DIGEST_TRAILER_BYTES {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let (body, trailer) = bytes.split_at(bytes.len() - DIGEST_TRAILER_BYTES);
    if body.len() < minimum_body_len() {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let mut cursor = 0_usize;
    if take(body, &mut cursor, MAGIC.len())? != MAGIC {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let version = u16::from_le_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    if version != UNIT_MANIFEST_VERSION {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let digest_algorithm = parse_digest_tag(take(body, &mut cursor, 1)?[0])?;
    let source_len = usize::from(u16::from_le_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    ));
    if source_len == 0 {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let source_bytes = take(body, &mut cursor, source_len)?;
    let source_text =
        core::str::from_utf8(source_bytes).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let source_id =
        OpaqueId::new(source_text).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let revision = u64::from_le_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    let revision =
        NonZeroRevision::new(revision).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let digest_at = |cursor: &mut usize| -> Result<Blake3Digest32, UnitizationError> {
        let raw: [u8; 32] = take(body, cursor, 32)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
        Ok(Blake3Digest32::from_bytes(raw))
    };
    let content_digest = digest_at(&mut cursor)?;
    let representation_id = digest_at(&mut cursor)?;
    let materializer_profile_digest: [u8; 32] = take(body, &mut cursor, 32)?
        .try_into()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let canonical_digest = digest_at(&mut cursor)?;
    let coordinate_digest = digest_at(&mut cursor)?;
    let loss_digest = digest_at(&mut cursor)?;
    let profile_id_raw: [u8; 32] = take(body, &mut cursor, 32)?
        .try_into()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let unitizer_profile_id = UnitizerProfileId::from_bytes(profile_id_raw);
    let unitizer_profile_revision = u64::from_le_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    if unitizer_profile_revision == 0 {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let limit_at = |cursor: &mut usize| -> Result<usize, UnitizationError> {
        let raw = u64::from_le_bytes(
            take(body, cursor, 8)?
                .try_into()
                .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
        );
        usize::try_from(raw).map_err(|_| UnitizationError::OffsetOverflow)
    };
    let unitizer_limits = UnitizationLimits {
        max_input_bytes: limit_at(&mut cursor)?,
        preferred_unit_bytes: limit_at(&mut cursor)?,
        max_unit_bytes: limit_at(&mut cursor)?,
        max_lines: limit_at(&mut cursor)?,
        max_units: limit_at(&mut cursor)?,
    };
    unitizer_limits
        .validate()
        .map_err(|_| UnitizationError::InvalidLimits)?;
    let count_at = |cursor: &mut usize| -> Result<u64, UnitizationError> {
        Ok(u64::from_le_bytes(
            take(body, cursor, 8)?
                .try_into()
                .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
        ))
    };
    let input_bytes = count_at(&mut cursor)?;
    let emitted_bytes = count_at(&mut cursor)?;
    let unit_count = count_at(&mut cursor)?;
    let line_count = count_at(&mut cursor)?;
    if emitted_bytes != input_bytes {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let unit_count_usize =
        usize::try_from(unit_count).map_err(|_| UnitizationError::OffsetOverflow)?;
    if unit_count_usize > unitizer_limits.max_units {
        return Err(UnitizationError::TooManyUnits);
    }
    if body.len() - cursor != unit_count_usize * UNIT_CODEC_BYTES {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let mut units = Vec::with_capacity(unit_count_usize);
    let mut expected_start = 0_u64;
    for ordinal in 0..unit_count_usize {
        let stored_ordinal = count_at(&mut cursor)?;
        let source_start = count_at(&mut cursor)?;
        let source_end = count_at(&mut cursor)?;
        let logical_line_start = count_at(&mut cursor)?;
        let logical_line_end = count_at(&mut cursor)?;
        let flags = take(body, &mut cursor, 1)?[0];
        if flags > 3 {
            return Err(UnitizationError::UnitManifestIncomplete);
        }
        let unit_digest = digest_at(&mut cursor)?;
        let expected_ordinal =
            u64::try_from(ordinal).map_err(|_| UnitizationError::OffsetOverflow)?;
        if stored_ordinal != expected_ordinal
            || source_start != expected_start
            || source_end <= source_start
        {
            return Err(UnitizationError::UnitizationNondeterministic);
        }
        units.push(UnitDescriptor {
            ordinal: stored_ordinal,
            source_start,
            source_end,
            logical_line_start,
            logical_line_end,
            starts_at_line_boundary: flags & 1 == 1,
            ends_at_line_boundary: flags & 2 == 2,
            unit_digest,
        });
        expected_start = source_end;
    }
    let mut ordered: Vec<Blake3Digest32> = units.iter().map(UnitDescriptor::unit_digest).collect();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(UnitizationError::UnitizationNondeterministic);
    }
    let expected_trailer = digest32(MANIFEST_DOMAIN, &[body]);
    if trailer != expected_trailer {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    let manifest_digest = Blake3Digest32::from_bytes(
        trailer
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestDigestMismatch)?,
    );
    Ok(UnitManifest {
        source_id,
        revision,
        content_digest,
        representation_id,
        materializer_profile_digest,
        canonical_digest,
        coordinate_digest,
        loss_digest,
        unitizer_profile_id,
        unitizer_profile_revision,
        unitizer_limits,
        digest_algorithm,
        input_bytes,
        emitted_bytes,
        line_count,
        units,
        manifest_digest,
    })
}

/// Recomputes unit IDs, ordering, spans, binding and the manifest digest and
/// proves complete profile-defined accounting. It does not assert filesystem
/// currentness or indexed publication.
pub fn verify_unit_manifest(
    manifest: &UnitManifest,
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
) -> Result<UnitManifestVerificationReceipt, UnitizationError> {
    if unitizer_profile_digest(profile) != profile.id {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if manifest.unitizer_profile_id != profile.id
        || manifest.unitizer_profile_revision != profile.revision
        || manifest.unitizer_limits != profile.limits
    {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if manifest.digest_algorithm != UNIT_MANIFEST_DIGEST_ALGORITHM {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if manifest.source_id != input.source_id
        || manifest.revision != input.revision
        || manifest.content_digest != input.content_digest
    {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    if manifest.representation_id != provenance.representation_id
        || manifest.materializer_profile_digest != provenance.materializer_profile_digest
        || manifest.canonical_digest != provenance.canonical_digest
        || manifest.coordinate_digest != provenance.coordinate_digest
        || manifest.loss_digest != provenance.loss_digest
    {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let expected = assemble_manifest(input, provenance, profile, manifest.digest_algorithm)?;
    if expected.units != manifest.units
        || expected.input_bytes != manifest.input_bytes
        || expected.emitted_bytes != manifest.emitted_bytes
        || expected.line_count != manifest.line_count
    {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if expected.manifest_digest != manifest.manifest_digest {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    Ok(UnitManifestVerificationReceipt {
        source_id: manifest.source_id.clone(),
        revision: manifest.revision,
        representation_id: manifest.representation_id,
        unitizer_profile_id: manifest.unitizer_profile_id,
        materializer_profile_digest: manifest.materializer_profile_digest,
        unit_count: u64::try_from(manifest.units.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?,
        manifest_digest: manifest.manifest_digest,
    })
}

/// Returns the exact identity difference between two manifests.
///
/// Reports created, retained and retired unit identities with changed
/// identity reasons carried by the two manifest digests. Retained requires
/// exact unit-identity digest equality; heuristic span or name similarity
/// can never retain a unit.
pub fn diff_unit_manifests(
    old: &UnitManifest,
    new: &UnitManifest,
) -> Result<UnitManifestDiff, UnitizationError> {
    if old.digest_algorithm != new.digest_algorithm {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    let mut old_sorted: Vec<Blake3Digest32> =
        old.units.iter().map(UnitDescriptor::unit_digest).collect();
    old_sorted.sort();
    let mut new_sorted: Vec<Blake3Digest32> =
        new.units.iter().map(UnitDescriptor::unit_digest).collect();
    new_sorted.sort();
    let mut retained = Vec::new();
    let mut created = Vec::new();
    for digest in new.units.iter().map(UnitDescriptor::unit_digest) {
        if old_sorted.binary_search(&digest).is_ok() {
            retained.push(digest);
        } else {
            created.push(digest);
        }
    }
    let mut retired = Vec::new();
    for digest in old.units.iter().map(UnitDescriptor::unit_digest) {
        if new_sorted.binary_search(&digest).is_err() {
            retired.push(digest);
        }
    }
    Ok(UnitManifestDiff {
        old_digest: old.manifest_digest,
        new_digest: new.manifest_digest,
        retained,
        created,
        retired,
    })
}

fn take<'body>(
    body: &'body [u8],
    cursor: &mut usize,
    count: usize,
) -> Result<&'body [u8], UnitizationError> {
    let end = cursor
        .checked_add(count)
        .ok_or(UnitizationError::OffsetOverflow)?;
    let slice = body
        .get(*cursor..end)
        .ok_or(UnitizationError::UnitManifestIncomplete)?;
    *cursor = end;
    Ok(slice)
}

fn encoded_size(source_len: usize, unit_count: usize) -> Result<usize, UnitizationError> {
    let Some(body) = source_len.checked_add(fixed_body_len()).and_then(|base| {
        unit_count
            .checked_mul(UNIT_CODEC_BYTES)
            .and_then(|tail| base.checked_add(tail))
    }) else {
        return Err(UnitizationError::OffsetOverflow);
    };
    body.checked_add(DIGEST_TRAILER_BYTES)
        .ok_or(UnitizationError::OffsetOverflow)
}

const fn fixed_body_len() -> usize {
    // MAGIC + version + algorithm + source_len + revision + six digests +
    // unitizer profile id + profile revision + five limits + four counts.
    8 + 2 + 1 + 2 + 8 + 32 * 6 + 32 + 8 + 8 * 5 + 8 * 4
}

const fn minimum_body_len() -> usize {
    // Fixed body with an empty source name; decode rejects the empty name
    // after this length gate.
    fixed_body_len()
}

fn encode_body(manifest: &UnitManifest, out: &mut Vec<u8>) -> Result<(), UnitizationError> {
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&UNIT_MANIFEST_VERSION.to_le_bytes());
    out.push(digest_tag(manifest.digest_algorithm));
    let source = manifest.source_id.as_str().as_bytes();
    out.extend_from_slice(
        &u16::try_from(source.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?
            .to_le_bytes(),
    );
    out.extend_from_slice(source);
    out.extend_from_slice(&manifest.revision.get().to_le_bytes());
    out.extend_from_slice(manifest.content_digest.as_bytes());
    out.extend_from_slice(manifest.representation_id.as_bytes());
    out.extend_from_slice(&manifest.materializer_profile_digest);
    out.extend_from_slice(manifest.canonical_digest.as_bytes());
    out.extend_from_slice(manifest.coordinate_digest.as_bytes());
    out.extend_from_slice(manifest.loss_digest.as_bytes());
    out.extend_from_slice(manifest.unitizer_profile_id.as_bytes());
    out.extend_from_slice(&manifest.unitizer_profile_revision.to_le_bytes());
    for limit in [
        manifest.unitizer_limits.max_input_bytes,
        manifest.unitizer_limits.preferred_unit_bytes,
        manifest.unitizer_limits.max_unit_bytes,
        manifest.unitizer_limits.max_lines,
        manifest.unitizer_limits.max_units,
    ] {
        out.extend_from_slice(
            &u64::try_from(limit)
                .map_err(|_| UnitizationError::OffsetOverflow)?
                .to_le_bytes(),
        );
    }
    for count in [
        manifest.input_bytes,
        manifest.emitted_bytes,
        u64::try_from(manifest.units.len()).map_err(|_| UnitizationError::OffsetOverflow)?,
        manifest.line_count,
    ] {
        out.extend_from_slice(&count.to_le_bytes());
    }
    for unit in &manifest.units {
        out.extend_from_slice(&unit.ordinal.to_le_bytes());
        out.extend_from_slice(&unit.source_start.to_le_bytes());
        out.extend_from_slice(&unit.source_end.to_le_bytes());
        out.extend_from_slice(&unit.logical_line_start.to_le_bytes());
        out.extend_from_slice(&unit.logical_line_end.to_le_bytes());
        out.push(
            u8::from(unit.starts_at_line_boundary) | (u8::from(unit.ends_at_line_boundary) << 1),
        );
        out.extend_from_slice(unit.unit_digest.as_bytes());
    }
    Ok(())
}
