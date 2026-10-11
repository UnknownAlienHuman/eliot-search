//! Immutable structural result; decoded manifests remain proposals until replay.
use super::profile::MarkdownUnitizerProfile;
use crate::{UnitManifest, UnitizationError, V3SourceBinding};
use search_contracts::{
    Blake3Digest32, OpaqueId, Representation, RepresentationId, UnitOccurrence,
};
use std::fmt;

/// Content-free refusal from the additional structural preparation pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownUnitizationError {
    /// A finite allowance is zero, widening or internally inconsistent.
    InvalidLimits,
    /// The original cancellation flag was observed.
    Cancelled,
    /// The original monotonic deadline elapsed.
    DeadlineExceeded,
    /// Logical work reservations or measured work exceed the call allowance.
    WorkLimit,
    /// Logical cumulative owner storage exceeds the call allowance.
    StorageLimit,
    /// Complete canonical preimages exceed the call allowance.
    CanonicalLimit,
    /// The immutable profile selectors do not match actual owning inputs.
    ProfileMismatch,
    /// Raw and structural representation IDs are equal or incoherent.
    BindingMismatch,
    /// An observation, reference, range or closed wire record is invalid.
    InvalidStructure,
    /// Checked arithmetic or conversion overflowed.
    ArithmeticOverflow,
    /// A fallible owner reservation failed.
    AllocationFailed,
    /// Owning exact-source Markdown replay refused.
    SourceReplayFailed,
    /// Existing raw v3 preparation refused with a content-free reason.
    Raw(UnitizationError),
    /// Shared canonical parsing or encoding refused.
    Codec,
    /// Complete reconstruction differs from the proposed manifest.
    ManifestMismatch,
    /// A duplicate or conflicting full commitment shares a truncated unit ID.
    IdentityCollision,
}
impl MarkdownUnitizationError {
    /// Stable machine-readable refusal; no source or vendor text is included.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "MD_UNITIZER_INVALID_LIMITS",
            Self::Cancelled => "MD_UNITIZER_CANCELLED",
            Self::DeadlineExceeded => "MD_UNITIZER_DEADLINE_EXCEEDED",
            Self::WorkLimit => "MD_UNITIZER_WORK_LIMIT",
            Self::StorageLimit => "MD_UNITIZER_STORAGE_LIMIT",
            Self::CanonicalLimit => "MD_UNITIZER_CANONICAL_LIMIT",
            Self::ProfileMismatch => "MD_UNITIZER_PROFILE_MISMATCH",
            Self::BindingMismatch => "MD_UNITIZER_BINDING_MISMATCH",
            Self::InvalidStructure => "MD_UNITIZER_INVALID_STRUCTURE",
            Self::ArithmeticOverflow => "MD_UNITIZER_ARITHMETIC_OVERFLOW",
            Self::AllocationFailed => "MD_UNITIZER_ALLOCATION_FAILED",
            Self::SourceReplayFailed => "MD_UNITIZER_SOURCE_REPLAY_FAILED",
            Self::Raw(error) => error.code(),
            Self::Codec => "MD_UNITIZER_CODEC",
            Self::ManifestMismatch => "MD_UNITIZER_MANIFEST_MISMATCH",
            Self::IdentityCollision => "MD_UNITIZER_IDENTITY_COLLISION",
        }
    }
}
impl fmt::Display for MarkdownUnitizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for MarkdownUnitizationError {}
impl From<UnitizationError> for MarkdownUnitizationError {
    fn from(error: UnitizationError) -> Self {
        Self::Raw(error)
    }
}

/// Admission-supplied coordinates with a distinct structural representation.
#[derive(Debug, Eq, PartialEq)]
pub struct MarkdownSourceBinding {
    pub(super) raw: V3SourceBinding,
    pub(super) structural: RepresentationId,
}
impl MarkdownSourceBinding {
    /// Refuses equal raw/structural IDs; does not issue source admission.
    pub fn new(
        raw: V3SourceBinding,
        structural: RepresentationId,
    ) -> Result<Self, MarkdownUnitizationError> {
        if raw.representation_id == structural {
            return Err(MarkdownUnitizationError::BindingMismatch);
        }
        Ok(Self { raw, structural })
    }
    /// Admitted raw coordinates; no UUID is derived from a digest.
    pub const fn raw(&self) -> &V3SourceBinding {
        &self.raw
    }
    /// Admitted structural representation, distinct from raw v3.
    pub const fn structural_representation_id(&self) -> RepresentationId {
        self.structural
    }
}

/// Selected inert syntax value with whole-event mapped provenance.
#[derive(Eq, PartialEq)]
pub struct MarkdownMappedValue {
    pub(super) value: String,
}
impl MarkdownMappedValue {
    /// Decoded observation, never an exact raw-byte subanchor.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Closed assurance token; no renderer, normalization or fetch is implied.
    pub const fn assurance(&self) -> &'static str {
        "mapped-text"
    }
}
impl fmt::Debug for MarkdownMappedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarkdownMappedValue")
            .field("bytes", &self.value.len())
            .finish()
    }
}

/// One original-range occurrence in donor preorder; overlap is observational.
#[derive(Debug, Eq, PartialEq)]
pub struct MarkdownStructuralUnit {
    pub(super) data: MarkdownNodeData,
    pub(super) occurrence: UnitOccurrence,
    pub(super) identity: Blake3Digest32,
}
/// Private complete observation preimage; no provisional occurrence identity.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct MarkdownNodeData {
    pub(super) ordinal: u64,
    pub(super) kind: &'static str,
    pub(super) heading_level: u8,
    pub(super) parent_section: Option<u64>,
    pub(super) physical_parent: Option<u64>,
    pub(super) heading_section: Option<u64>,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) code_info: Option<MarkdownMappedValue>,
    pub(super) content: Blake3Digest32,
}
impl MarkdownStructuralUnit {
    /// Contract occurrence with exact raw anchor and full structural identity.
    pub const fn occurrence(&self) -> &UnitOccurrence {
        &self.occurrence
    }
    /// Closed Markdown kind token from the v4 profile.
    pub const fn kind(&self) -> &'static str {
        self.data.kind
    }
    /// Heading level1..6, or0 for other kinds.
    pub const fn heading_level(&self) -> u8 {
        self.data.heading_level
    }
    /// Earlier lower-level heading; independent of physical containment.
    pub const fn parent_section(&self) -> Option<u64> {
        self.data.parent_section
    }
    /// Nearest containing published structural node ordinal.
    pub const fn physical_parent(&self) -> Option<u64> {
        self.data.physical_parent
    }
    /// Logical heading relation; a heading refers to itself.
    pub const fn heading_section(&self) -> Option<u64> {
        self.data.heading_section
    }
    /// Inclusive original byte start.
    pub const fn start(&self) -> u64 {
        self.data.start
    }
    /// Exclusive original byte end.
    pub const fn end(&self) -> u64 {
        self.data.end
    }
    /// Selected inert mapped code-info observation.
    pub const fn code_info(&self) -> Option<&MarkdownMappedValue> {
        self.data.code_info.as_ref()
    }
    /// Full occurrence commitment retained beside the truncated `UnitId`.
    pub const fn identity_digest(&self) -> Blake3Digest32 {
        self.identity
    }
}

/// Every yielded inline observation, including zero-width spans and mapped values.
#[derive(Debug, Eq, PartialEq)]
pub struct MarkdownInlineObservation {
    pub(super) owner: u64,
    pub(super) form: &'static str,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) text: Option<MarkdownMappedValue>,
    pub(super) destination: Option<MarkdownMappedValue>,
    pub(super) title: Option<MarkdownMappedValue>,
}
impl MarkdownInlineObservation {
    /// Structural owner ordinal.
    pub const fn owner(&self) -> u64 {
        self.owner
    }
    /// Closed observed form, retaining link/image subtype.
    pub const fn form(&self) -> &'static str {
        self.form
    }
    /// Inclusive whole-event raw start.
    pub const fn start(&self) -> u64 {
        self.start
    }
    /// Exclusive whole-event raw end; equality is permitted.
    pub const fn end(&self) -> u64 {
        self.end
    }
    /// Selected mapped inline-code value; Text/HTML payloads remain absent.
    pub const fn text(&self) -> Option<&MarkdownMappedValue> {
        self.text.as_ref()
    }
    /// Inert mapped destination, never fetched.
    pub const fn destination(&self) -> Option<&MarkdownMappedValue> {
        self.destination.as_ref()
    }
    /// Inert mapped title.
    pub const fn title(&self) -> Option<&MarkdownMappedValue> {
        self.title.as_ref()
    }
}

/// Closed proposed v4 manifest; only complete owning replay verifies it.
#[derive(Eq, PartialEq)]
pub struct MarkdownUnitManifest {
    pub(super) body: MarkdownManifestBody,
    pub(super) digest: Blake3Digest32,
}
/// Private body, sealed into a proposal only after its complete digest exists.
#[derive(Eq, PartialEq)]
pub(super) struct MarkdownManifestBody {
    pub(super) profile: MarkdownUnitizerProfile,
    pub(super) raw: UnitManifest,
    pub(super) structural: RepresentationId,
    pub(super) residency: OpaqueId,
    pub(super) markdown_product: Blake3Digest32,
    pub(super) markdown_nodes: Blake3Digest32,
    pub(super) markdown_inline: Blake3Digest32,
    pub(super) nodes: Vec<MarkdownStructuralUnit>,
    pub(super) inline: Vec<MarkdownInlineObservation>,
}
pub(super) const INLINE_STATUS: &str = "observed-spans-and-selected-values";
pub(super) const REFERENCE_STATUS: &str = "potentially-incomplete";
impl MarkdownUnitManifest {
    /// Unchanged exact v3 raw partition and provenance, independent of overlaps.
    pub const fn raw_manifest(&self) -> &UnitManifest {
        &self.body.raw
    }
    /// Complete ordered structural units.
    pub fn units(&self) -> &[MarkdownStructuralUnit] {
        &self.body.nodes
    }
    /// Complete ordered yielded inline observations.
    pub fn inline_observations(&self) -> &[MarkdownInlineObservation] {
        &self.body.inline
    }
    /// Profile identity of this proposed manifest.
    pub const fn profile_id(&self) -> &search_contracts::ProfileId {
        self.body.profile.id()
    }
    /// Canonical body commitment; a digest alone is not completeness proof.
    pub const fn digest(&self) -> Blake3Digest32 {
        self.digest
    }
    /// Selected span/value observation scope, not a decoded display-text inventory.
    pub const fn inline_metadata_status(&self) -> &'static str {
        INLINE_STATUS
    }
    /// Donor references remain potentially incomplete.
    pub const fn reference_metadata_status(&self) -> &'static str {
        REFERENCE_STATUS
    }
}
impl fmt::Debug for MarkdownUnitManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarkdownUnitManifest")
            .field("digest", &self.digest)
            .field("nodes", &self.body.nodes.len())
            .field("inline", &self.body.inline.len())
            .finish_non_exhaustive()
    }
}

/// Logical quota usage, not CPU time, RSS, qualification or source admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownUnitizationUsage {
    /// Cumulative measured owner work plus conservative nested reservations.
    pub work: usize,
    /// New measured complete preimages plus conservative nested reservations.
    pub canonical_bytes: usize,
    /// Cumulative logical owner storage reservations; no unused refund.
    pub owned_bytes: usize,
    /// Actual owning Markdown replay work reported separately, without double debit.
    pub markdown_replay_work: usize,
    /// Actual owning Markdown replay complete canonical bytes, without double debit.
    pub markdown_replay_canonical_bytes: usize,
}

/// Complete package-produced structural set with preserved raw coverage.
#[derive(Debug)]
pub struct VerifiedMarkdownUnitSet {
    pub(super) manifest: MarkdownUnitManifest,
    pub(super) representation: Representation,
    pub(super) usage: MarkdownUnitizationUsage,
}
impl VerifiedMarkdownUnitSet {
    /// Completely reconstructed immutable manifest, not merely a decoded proposal.
    pub const fn manifest(&self) -> &MarkdownUnitManifest {
        &self.manifest
    }
    /// Structural representation whose digest equals the complete manifest digest.
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }
    /// Logical quota and actual nested replay counters.
    pub const fn usage(&self) -> MarkdownUnitizationUsage {
        self.usage
    }
}

/// Owned v4 canonical bytes without an unmetered Clone API.
#[derive(Eq, PartialEq)]
pub struct CanonicalMarkdownUnitManifestBytes {
    pub(super) bytes: Vec<u8>,
}
impl CanonicalMarkdownUnitManifestBytes {
    /// Immutable canonical byte borrow; these bytes alone are not verification.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    /// Encoded byte count.
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }
    /// Whether the owned byte sequence is empty.
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
impl fmt::Debug for CanonicalMarkdownUnitManifestBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanonicalMarkdownUnitManifestBytes")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
