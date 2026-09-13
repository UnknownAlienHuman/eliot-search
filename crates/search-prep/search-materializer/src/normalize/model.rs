//! Canonical normalized text and coordinate models.

use crate::LineEnding;
use crate::profile::{
    MaterializerProfileId, NewlinePolicy, SourceEncoding,
};

/// One canonical line joining decoded scalar and canonical scalar coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalLine {
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end including the terminator.
    pub decoded_end: u64,
    /// Exclusive decoded scalar end excluding the terminator.
    pub decoded_content_end: u64,
    /// Inclusive canonical scalar start.
    pub canonical_start: u64,
    /// Exclusive canonical scalar end including the terminator.
    pub canonical_end: u64,
    /// Exclusive canonical scalar end excluding the terminator.
    pub canonical_content_end: u64,
    /// Terminator before normalization.
    pub ending_before: LineEnding,
    /// Terminator after normalization.
    pub ending_after: LineEnding,
}

impl CanonicalLine {
    /// Reports whether normalization changed this line's terminator.
    #[must_use]
    pub fn ending_changed(&self) -> bool {
        self.ending_before != self.ending_after
    }
}

/// Canonical representation with a complete offset-change record.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalRepresentation {
    pub(super) text: String,
    pub(super) lines: Vec<CanonicalLine>,
    pub(super) newline_policy: NewlinePolicy,
    pub(super) decoded_len_chars: u64,
    pub(super) canonical_len_chars: u64,
    pub(super) native_len: u64,
    pub(super) source_encoding: SourceEncoding,
    pub(super) bom_stripped: bool,
    pub(super) transcoded: bool,
    pub(super) profile_id: MaterializerProfileId,
}

impl CanonicalRepresentation {
    /// Canonical text after exactly the profile's normalization rules.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Canonical line table joining decoded and canonical coordinates.
    #[must_use]
    pub fn lines(&self) -> &[CanonicalLine] {
        &self.lines
    }

    /// Newline policy applied to produce this representation.
    #[must_use]
    pub const fn newline_policy(&self) -> NewlinePolicy {
        self.newline_policy
    }

    /// Decoded length in Unicode scalar values.
    #[must_use]
    pub const fn decoded_len_chars(&self) -> u64 {
        self.decoded_len_chars
    }

    /// Canonical length in Unicode scalar values.
    #[must_use]
    pub const fn canonical_len_chars(&self) -> u64 {
        self.canonical_len_chars
    }

    /// Canonical length in UTF-8 bytes.
    #[must_use]
    pub fn canonical_len_bytes(&self) -> u64 {
        u64::try_from(self.text.len()).unwrap_or(u64::MAX)
    }

    /// Exact native input length in bytes.
    #[must_use]
    pub const fn native_len(&self) -> u64 {
        self.native_len
    }

    /// Source encoding the canonical text was decoded from.
    #[must_use]
    pub const fn source_encoding(&self) -> SourceEncoding {
        self.source_encoding
    }

    /// Whether a BOM was stripped before normalization.
    #[must_use]
    pub const fn bom_stripped(&self) -> bool {
        self.bom_stripped
    }

    /// Whether bytes were transcoded from UTF-16.
    #[must_use]
    pub const fn transcoded(&self) -> bool {
        self.transcoded
    }

    /// Profile identity this normalization was performed under.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }
}

impl core::fmt::Debug for CanonicalRepresentation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CanonicalRepresentation")
            .field(
                "text",
                &format_args!("<{} chars>", self.canonical_len_chars),
            )
            .field("line_count", &self.lines.len())
            .field("newline_policy", &self.newline_policy)
            .field("encoding", &self.source_encoding)
            .field("bom_stripped", &self.bom_stripped)
            .field("transcoded", &self.transcoded)
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}
