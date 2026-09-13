//! Materializer profile descriptors, policies, limits and validated model.

use crate::MaterializationError;
use search_contracts::Blake3Digest32;

/// Maximum profile name length in bytes.
pub const MAX_PROFILE_NAME_BYTES: usize = 128;

/// Conservative finite profile limits shared by baseline descriptors.
pub const DEFAULT_PROFILE_LIMITS: MaterializationProfileLimits = MaterializationProfileLimits {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
    max_map_segments: 1_000_032,
    max_loss_records: 1_000_032,
    max_steps: 64 * 1024 * 1024,
};

/// Baseline-supported source kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// Plain text revision.
    Text,
    /// Source-code revision (never executed, compiled or macro-expanded).
    Code,
}

impl SourceKind {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Code => "code",
        }
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Text => 1,
            Self::Code => 2,
        }
    }
}

/// Baseline-supported source encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceEncoding {
    /// Strict UTF-8, optionally with a byte-order mark.
    Utf8,
    /// UTF-16 little-endian, transcoded exactly to UTF-8.
    Utf16Le,
    /// UTF-16 big-endian, transcoded exactly to UTF-8.
    Utf16Be,
}

impl SourceEncoding {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Utf16Le => "utf16le",
            Self::Utf16Be => "utf16be",
        }
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Utf8 => 1,
            Self::Utf16Le => 2,
            Self::Utf16Be => 3,
        }
    }
}

/// Declared byte-order-mark policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BomPolicy {
    /// A leading BOM is consumed and recorded as loss, never kept silently.
    StripAndRecord,
    /// Any BOM-bearing input is rejected as unsupported under this profile.
    RejectWhenPresent,
}

impl BomPolicy {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::StripAndRecord => 1,
            Self::RejectWhenPresent => 2,
        }
    }
}

/// Declared invalid-sequence policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InvalidSequencePolicy {
    /// Malformed or truncated sequences are typed errors, never silent output.
    Reject,
}

impl InvalidSequencePolicy {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Reject => 1,
        }
    }
}

/// Declared newline policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NewlinePolicy {
    /// Every terminator byte is preserved exactly.
    PreserveExact,
    /// `CRLF` and `CR` terminators become `LF`, each change recorded as loss.
    NormalizeToLf,
}

impl NewlinePolicy {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::PreserveExact => 1,
            Self::NormalizeToLf => 2,
        }
    }
}

/// Declared Unicode normalization policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnicodeNormalization {
    /// No code-point transformation; bytes decode exactly as declared.
    None,
}

impl UnicodeNormalization {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::None => 1,
        }
    }
}

/// Declared loss behavior for transforms that cannot preserve exact bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LossBehavior {
    /// Record every loss and lower assurance accordingly.
    RecordAndLowerAssurance,
    /// Reject any input that would require a lossy transform.
    RejectOnAnyLoss,
}

impl LossBehavior {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::RecordAndLowerAssurance => 1,
            Self::RejectOnAnyLoss => 2,
        }
    }
}

/// Coordinate spaces bound by every materializer profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoordinateSpace {
    /// Native retained byte offsets.
    NativeBytes,
    /// Decoded Unicode scalar positions.
    DecodedScalar,
    /// Canonical Unicode scalar positions.
    CanonicalScalar,
}

impl CoordinateSpace {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeBytes => "native-bytes",
            Self::DecodedScalar => "decoded-scalar",
            Self::CanonicalScalar => "canonical-scalar",
        }
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::NativeBytes => 1,
            Self::DecodedScalar => 2,
            Self::CanonicalScalar => 3,
        }
    }
}

/// Finite profile-owned limits. All dimensions are non-zero after validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationProfileLimits {
    /// Maximum exact retained input bytes.
    pub max_input_bytes: u64,
    /// Maximum canonical output bytes.
    pub max_output_bytes: u64,
    /// Maximum logical lines.
    pub max_lines: u64,
    /// Maximum coordinate map segments.
    pub max_map_segments: u64,
    /// Maximum loss map records.
    pub max_loss_records: u64,
    /// Maximum elementary decode/normalize/map steps.
    pub max_steps: u64,
}

impl MaterializationProfileLimits {
    pub(super) const fn validate(self) -> Result<Self, MaterializationError> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_lines == 0
            || self.max_map_segments == 0
            || self.max_loss_records == 0
            || self.max_steps == 0
        {
            return Err(MaterializationError::ProfileInvalid);
        }
        Ok(self)
    }
}

/// Unvalidated materializer profile descriptor supplied by configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializerProfileDescriptor {
    /// Human-readable profile name (ASCII, non-empty, bounded).
    pub profile_name: String,
    /// Monotone profile revision; downgrades are rejected, never reinterpreted.
    pub profile_revision: u64,
    /// Accepted source kinds, non-empty, duplicate-free.
    pub source_kinds: Vec<SourceKind>,
    /// Accepted source encodings, non-empty, duplicate-free.
    pub encodings: Vec<SourceEncoding>,
    /// Declared BOM policy.
    pub bom_policy: BomPolicy,
    /// Declared invalid-sequence policy.
    pub invalid_sequence_policy: InvalidSequencePolicy,
    /// Declared newline policy.
    pub newline_policy: NewlinePolicy,
    /// Declared Unicode normalization policy.
    pub unicode_normalization: UnicodeNormalization,
    /// Declared loss behavior.
    pub loss_behavior: LossBehavior,
    /// Finite profile-owned limits.
    pub limits: MaterializationProfileLimits,
    /// Required coordinate spaces (baseline: the full triple).
    pub coordinate_spaces: Vec<CoordinateSpace>,
    /// Digest of the golden fixture set this profile was qualified against.
    pub golden_fixture_digest: Blake3Digest32,
}

/// Canonical materializer profile identity: the domain-separated digest over
/// every load-bearing behavior and bound.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MaterializerProfileId([u8; 32]);

impl MaterializerProfileId {
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

impl core::fmt::Debug for MaterializerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("MaterializerProfileId")
            .field(&self.to_string())
            .finish()
    }
}

impl core::fmt::Display for MaterializerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Validated materializer profile with a bound canonical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMaterializerProfile {
    pub(super) name: String,
    pub(super) revision: u64,
    pub(super) kinds: Vec<SourceKind>,
    pub(super) encodings: Vec<SourceEncoding>,
    pub(super) bom_policy: BomPolicy,
    pub(super) invalid_sequence_policy: InvalidSequencePolicy,
    pub(super) newline_policy: NewlinePolicy,
    pub(super) unicode_normalization: UnicodeNormalization,
    pub(super) loss_behavior: LossBehavior,
    pub(super) limits: MaterializationProfileLimits,
    pub(super) spaces: Vec<CoordinateSpace>,
    pub(super) golden: Blake3Digest32,
    pub(super) id: MaterializerProfileId,
}

impl ValidatedMaterializerProfile {
    /// Canonical profile identity.
    #[must_use]
    pub const fn id(&self) -> MaterializerProfileId {
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

    /// Accepted source kinds.
    #[must_use]
    pub fn source_kinds(&self) -> &[SourceKind] {
        &self.kinds
    }

    /// Accepted source encodings.
    #[must_use]
    pub fn encodings(&self) -> &[SourceEncoding] {
        &self.encodings
    }

    /// Declared BOM policy.
    #[must_use]
    pub const fn bom_policy(&self) -> BomPolicy {
        self.bom_policy
    }

    /// Declared invalid-sequence policy.
    #[must_use]
    pub const fn invalid_sequence_policy(&self) -> InvalidSequencePolicy {
        self.invalid_sequence_policy
    }

    /// Declared newline policy.
    #[must_use]
    pub const fn newline_policy(&self) -> NewlinePolicy {
        self.newline_policy
    }

    /// Declared Unicode normalization policy.
    #[must_use]
    pub const fn unicode_normalization(&self) -> UnicodeNormalization {
        self.unicode_normalization
    }

    /// Declared loss behavior.
    #[must_use]
    pub const fn loss_behavior(&self) -> LossBehavior {
        self.loss_behavior
    }

    /// Finite profile-owned limits.
    #[must_use]
    pub const fn limits(&self) -> MaterializationProfileLimits {
        self.limits
    }

    /// Required coordinate spaces.
    #[must_use]
    pub fn coordinate_spaces(&self) -> &[CoordinateSpace] {
        &self.spaces
    }

    /// Golden fixture digest this profile was qualified against.
    #[must_use]
    pub const fn golden_fixture_digest(&self) -> Blake3Digest32 {
        self.golden
    }
}
