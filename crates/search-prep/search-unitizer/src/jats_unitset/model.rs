//! Immutable complete structural observations; decoded records remain proposals.
use super::profile::JatsUnitizerProfile;
use crate::{UnitManifest, UnitizationError, V3SourceBinding};
use search_contracts::{
    Blake3Digest32, OpaqueId, Representation, RepresentationId, UnitOccurrence,
};
use std::fmt;

/// Content-free refusal; neither source nor donor messages escape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JatsUnitizationError {
    /// A positive finite allowance is zero, widened or inconsistent.
    InvalidLimits,
    /// The original live cancellation flag was observed.
    Cancelled,
    /// The original absolute deadline elapsed.
    DeadlineExceeded,
    /// Cumulative admitted logical work was exhausted.
    WorkLimit,
    /// Cumulative admitted logical storage was exhausted.
    StorageLimit,
    /// Complete preimages or encoded output exceed the allowance.
    CanonicalLimit,
    /// Actual selected owner profiles disagree.
    ProfileMismatch,
    /// Admission-supplied raw/structural coordinates are incoherent.
    BindingMismatch,
    /// A complete observation's range/order/owner/closed form is invalid.
    InvalidStructure,
    /// Checked arithmetic or conversion overflowed.
    ArithmeticOverflow,
    /// Fallible owner reservation failed.
    AllocationFailed,
    /// Owning exact-source JATS replay refused.
    SourceReplayFailed,
    /// Existing raw v3 owner refused with a content-free reason.
    Raw(UnitizationError),
    /// Shared canonical parsing or encoding refused.
    Codec,
    /// Complete owning reconstruction differs from the proposal.
    ManifestMismatch,
    /// Two occurrences share a truncated unit ID.
    IdentityCollision,
}
impl JatsUnitizationError {
    /// Stable machine-readable refusal, without source or private coordinates.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "JATS_UNITIZER_INVALID_LIMITS",
            Self::Cancelled => "JATS_UNITIZER_CANCELLED",
            Self::DeadlineExceeded => "JATS_UNITIZER_DEADLINE_EXCEEDED",
            Self::WorkLimit => "JATS_UNITIZER_WORK_LIMIT",
            Self::StorageLimit => "JATS_UNITIZER_STORAGE_LIMIT",
            Self::CanonicalLimit => "JATS_UNITIZER_CANONICAL_LIMIT",
            Self::ProfileMismatch => "JATS_UNITIZER_PROFILE_MISMATCH",
            Self::BindingMismatch => "JATS_UNITIZER_BINDING_MISMATCH",
            Self::InvalidStructure => "JATS_UNITIZER_INVALID_STRUCTURE",
            Self::ArithmeticOverflow => "JATS_UNITIZER_ARITHMETIC_OVERFLOW",
            Self::AllocationFailed => "JATS_UNITIZER_ALLOCATION_FAILED",
            Self::SourceReplayFailed => "JATS_UNITIZER_SOURCE_REPLAY_FAILED",
            Self::Raw(error) => error.code(),
            Self::Codec => "JATS_UNITIZER_CODEC",
            Self::ManifestMismatch => "JATS_UNITIZER_MANIFEST_MISMATCH",
            Self::IdentityCollision => "JATS_UNITIZER_IDENTITY_COLLISION",
        }
    }
}
impl fmt::Display for JatsUnitizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for JatsUnitizationError {}
impl From<UnitizationError> for JatsUnitizationError {
    fn from(error: UnitizationError) -> Self {
        Self::Raw(error)
    }
}

/// Admission-supplied coordinates, never issued or inferred from content here.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsSourceBinding {
    pub(super) raw: V3SourceBinding,
    pub(super) structural: RepresentationId,
}
impl JatsSourceBinding {
    /// Refuses equal raw and structural representations; does not admit a source.
    pub fn new(
        raw: V3SourceBinding,
        structural: RepresentationId,
    ) -> Result<Self, JatsUnitizationError> {
        if raw.representation_id == structural {
            return Err(JatsUnitizationError::BindingMismatch);
        }
        Ok(Self { raw, structural })
    }
    /// Original admitted raw v3 coordinates.
    pub const fn raw(&self) -> &V3SourceBinding {
        &self.raw
    }
    /// Distinct admitted structural representation.
    pub const fn structural_representation_id(&self) -> RepresentationId {
        self.structural
    }
}
/// Original UTF-8 half-open byte extent, never a normalized-value subanchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsUnitSpan {
    pub(super) start: u64,
    pub(super) end: u64,
}
impl JatsUnitSpan {
    /// Inclusive original byte start.
    pub const fn start(self) -> u64 {
        self.start
    }
    /// Exclusive original byte end.
    pub const fn end(self) -> u64 {
        self.end
    }
}
/// Normalized inert metadata with whole-event provenance in its owning record.
#[derive(Eq, PartialEq)]
pub struct JatsMappedValue {
    pub(super) value: String,
}
impl JatsMappedValue {
    /// Mapped normalized value, never fetched, executed or quoted as exact bytes.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Closed mapped assurance token.
    pub const fn assurance(&self) -> &'static str {
        "mapped-text"
    }
}
impl fmt::Debug for JatsMappedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JatsMappedValue")
            .field("bytes", &self.value.len())
            .finish()
    }
}
#[derive(Debug, Eq, PartialEq)]
pub(super) struct NodeData {
    pub(super) ordinal: u64,
    pub(super) kind: &'static str,
    pub(super) parent: Option<u64>,
    pub(super) event: u64,
    pub(super) close_event: u64,
    pub(super) opening: JatsUnitSpan,
    pub(super) closing: Option<JatsUnitSpan>,
    pub(super) span: JatsUnitSpan,
    pub(super) content: Blake3Digest32,
}
/// One original-range occurrence for every element in opening-event order.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsStructuralUnit {
    pub(super) data: NodeData,
    pub(super) occurrence: UnitOccurrence,
    pub(super) identity: Blake3Digest32,
}
impl JatsStructuralUnit {
    /// Contract occurrence with exact original-XML anchor.
    pub const fn occurrence(&self) -> &UnitOccurrence {
        &self.occurrence
    }
    /// Closed element syntax name from the selected producer subset.
    pub const fn kind(&self) -> &'static str {
        self.data.kind
    }
    /// Earlier physically containing element ordinal.
    pub const fn parent(&self) -> Option<u64> {
        self.data.parent
    }
    /// Original opening event ordinal.
    pub const fn event(&self) -> u64 {
        self.data.event
    }
    /// Original closing event ordinal, equal to opening for empty syntax.
    pub const fn close_event(&self) -> u64 {
        self.data.close_event
    }
    /// Exact original opening tag.
    pub const fn opening(&self) -> JatsUnitSpan {
        self.data.opening
    }
    /// Exact original closing tag, absent for empty syntax.
    pub const fn closing(&self) -> Option<JatsUnitSpan> {
        self.data.closing
    }
    /// Complete original element extent; overlaps are not additive coverage.
    pub const fn span(&self) -> JatsUnitSpan {
        self.data.span
    }
    /// Full occurrence commitment beside its truncated unit ID.
    pub const fn identity_digest(&self) -> Blake3Digest32 {
        self.identity
    }
}
/// Every supported attribute in producer source order.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsUnitAttribute {
    pub(super) owner: u64,
    pub(super) kind: &'static str,
    pub(super) span: JatsUnitSpan,
    pub(super) value: JatsMappedValue,
}
impl JatsUnitAttribute {
    /// Owning element ordinal.
    pub const fn owner(&self) -> u64 {
        self.owner
    }
    /// Closed attribute syntax name.
    pub const fn kind(&self) -> &'static str {
        self.kind
    }
    /// Whole-opening-tag provenance, never a decoded-value subanchor.
    pub const fn span(&self) -> JatsUnitSpan {
        self.span
    }
    /// Normalized inert metadata.
    pub const fn value(&self) -> &JatsMappedValue {
        &self.value
    }
}
/// Every text, CDATA or reference observation in event order.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsUnitText {
    pub(super) owner: u64,
    pub(super) event: u64,
    pub(super) kind: &'static str,
    pub(super) span: JatsUnitSpan,
    pub(super) value: JatsMappedValue,
}
impl JatsUnitText {
    /// Owning element ordinal.
    pub const fn owner(&self) -> u64 {
        self.owner
    }
    /// Original event ordinal.
    pub const fn event(&self) -> u64 {
        self.event
    }
    /// Closed text/cdata/reference form.
    pub const fn kind(&self) -> &'static str {
        self.kind
    }
    /// Original whole-event provenance.
    pub const fn span(&self) -> JatsUnitSpan {
        self.span
    }
    /// Normalized inert metadata.
    pub const fn value(&self) -> &JatsMappedValue {
        &self.value
    }
}
/// Inert original comment, including comments outside the article root.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsUnitComment {
    pub(super) owner: Option<u64>,
    pub(super) event: u64,
    pub(super) span: JatsUnitSpan,
}
impl JatsUnitComment {
    /// Owning element, absent for prolog/epilog comments.
    pub const fn owner(&self) -> Option<u64> {
        self.owner
    }
    /// Original event ordinal.
    pub const fn event(&self) -> u64 {
        self.event
    }
    /// Exact original whole-comment syntax extent; no copied comment text.
    pub const fn span(&self) -> JatsUnitSpan {
        self.span
    }
}
pub(super) const STATUS: &str = "complete-subset-mapped-values";
#[derive(Eq, PartialEq)]
pub(super) struct ManifestBody {
    pub(super) profile: JatsUnitizerProfile,
    pub(super) raw: UnitManifest,
    pub(super) structural: RepresentationId,
    pub(super) residency: OpaqueId,
    pub(super) product: Blake3Digest32,
    pub(super) inventory: Blake3Digest32,
    pub(super) events: u64,
    pub(super) declaration: Option<JatsUnitSpan>,
    pub(super) nodes: Vec<JatsStructuralUnit>,
    pub(super) attributes: Vec<JatsUnitAttribute>,
    pub(super) texts: Vec<JatsUnitText>,
    pub(super) comments: Vec<JatsUnitComment>,
}
/// Closed proposed v5 manifest; complete owning replay is its verification authority.
#[derive(Eq, PartialEq)]
pub struct JatsUnitManifest {
    pub(super) body: ManifestBody,
    pub(super) digest: Blake3Digest32,
}
impl JatsUnitManifest {
    /// Unchanged raw v3 partition and sole represented-byte denominator.
    pub const fn raw_manifest(&self) -> &UnitManifest {
        &self.body.raw
    }
    /// Complete ordered element occurrences.
    pub fn units(&self) -> &[JatsStructuralUnit] {
        &self.body.nodes
    }
    /// Complete ordered attributes.
    pub fn attributes(&self) -> &[JatsUnitAttribute] {
        &self.body.attributes
    }
    /// Complete ordered mapped text observations.
    pub fn texts(&self) -> &[JatsUnitText] {
        &self.body.texts
    }
    /// Complete ordered inert comments, including prolog/epilog.
    pub fn comments(&self) -> &[JatsUnitComment] {
        &self.body.comments
    }
    /// Optional exact original XML declaration syntax extent.
    pub const fn declaration_span(&self) -> Option<JatsUnitSpan> {
        self.body.declaration
    }
    /// Total original producer event calls, including EOF.
    pub const fn event_calls(&self) -> u64 {
        self.body.events
    }
    /// Selected producer subset/mapped status; no full schema conformance claim.
    pub const fn observation_status(&self) -> &'static str {
        STATUS
    }
    /// Internally computed structural profile identity.
    pub const fn profile_id(&self) -> &search_contracts::ProfileId {
        self.body.profile.id()
    }
    /// Complete body commitment; digest equality alone is not completeness proof.
    pub const fn digest(&self) -> Blake3Digest32 {
        self.digest
    }
}
impl fmt::Debug for JatsUnitManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JatsUnitManifest")
            .field("digest", &self.digest)
            .field("elements", &self.body.nodes.len())
            .field("attributes", &self.body.attributes.len())
            .field("texts", &self.body.texts.len())
            .field("comments", &self.body.comments.len())
            .finish_non_exhaustive()
    }
}
/// Logical cumulative usage, not CPU/RSS, execution evidence or admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsUnitizationUsage {
    /// Measured owner work plus nested reservations, with no unused refunds.
    pub work: usize,
    /// Actual new complete preimages plus nested reservations.
    pub canonical_bytes: usize,
    /// Cumulative logical held/allocated graph, record and string reservations.
    pub owned_bytes: usize,
    /// Actual owning JATS replay work, reported without a second debit.
    pub jats_replay_work: usize,
    /// Actual owning JATS replay complete preimages, without a second debit.
    pub jats_replay_canonical_bytes: usize,
}
/// Complete package-produced set, retaining raw coverage independently of overlaps.
#[derive(Debug)]
pub struct VerifiedJatsUnitSet {
    pub(super) manifest: JatsUnitManifest,
    pub(super) representation: Representation,
    pub(super) usage: JatsUnitizationUsage,
}
impl VerifiedJatsUnitSet {
    /// Fully reconstructed immutable manifest.
    pub const fn manifest(&self) -> &JatsUnitManifest {
        &self.manifest
    }
    /// Structural representation binding the full manifest commitment.
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }
    /// Cumulative logical usage and separately observed replay counters.
    pub const fn usage(&self) -> JatsUnitizationUsage {
        self.usage
    }
}
/// Owned canonical v5 envelope bytes; no unmetered Clone API.
#[derive(Eq, PartialEq)]
pub struct CanonicalJatsUnitManifestBytes {
    pub(super) bytes: Vec<u8>,
}
impl CanonicalJatsUnitManifestBytes {
    /// Immutable encoded proposal; source verification remains mandatory.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    /// Encoded byte count.
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }
    /// Whether the envelope is empty.
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
impl fmt::Debug for CanonicalJatsUnitManifestBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanonicalJatsUnitManifestBytes")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
