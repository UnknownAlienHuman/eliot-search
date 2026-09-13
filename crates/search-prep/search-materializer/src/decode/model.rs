//! Public decoding contracts and bounded step accounting.

use crate::profile::{MaterializerProfileId, SourceEncoding};
use crate::{LineEnding, MaterializationError};

/// Bounded elementary-step accounting shared by decode, normalize and map
/// stages of one materialization operation.
///
/// One counter travels through every stage so a single operation-wide step
/// bound holds; exhaustion fails with
/// [`MaterializationError::BudgetExhausted`](crate::MaterializationError),
/// never with a truncated product.
pub struct StepCounter {
    used: u64,
    max: u64,
}

impl StepCounter {
    /// Starts an operation step budget.
    #[must_use]
    pub const fn new(max: u64) -> Self {
        Self { used: 0, max }
    }

    /// Consumes steps, failing closed on overflow or budget exhaustion.
    pub fn consume(&mut self, steps: u64) -> Result<(), MaterializationError> {
        self.used = self
            .used
            .checked_add(steps)
            .ok_or(MaterializationError::BudgetExhausted)?;
        if self.used > self.max {
            return Err(MaterializationError::BudgetExhausted);
        }
        Ok(())
    }

    /// Steps consumed so far.
    #[must_use]
    pub const fn used(&self) -> u64 {
        self.used
    }
}

/// Decided encoding with explicit BOM evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodingDecision {
    encoding: SourceEncoding,
    bom_present: bool,
    bom_len_bytes: usize,
}

impl EncodingDecision {
    pub(super) const fn new(
        encoding: SourceEncoding,
        bom_present: bool,
        bom_len_bytes: usize,
    ) -> Self {
        Self {
            encoding,
            bom_present,
            bom_len_bytes,
        }
    }

    /// Decided source encoding (always the declared, validated one).
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Whether a byte-order mark was observed.
    #[must_use]
    pub const fn bom_present(&self) -> bool {
        self.bom_present
    }

    /// Observed BOM length in native bytes (zero when absent).
    #[must_use]
    pub const fn bom_len_bytes(&self) -> usize {
        self.bom_len_bytes
    }
}

/// One decoded line with native byte and decoded scalar coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedLine {
    /// Inclusive native byte start.
    pub native_start: u64,
    /// Exclusive native byte end including the terminator.
    pub native_end: u64,
    /// Exclusive native byte end excluding the terminator.
    pub native_content_end: u64,
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end including the terminator.
    pub decoded_end: u64,
    /// Exclusive decoded scalar end excluding the terminator.
    pub decoded_content_end: u64,
    /// Exact retained line ending.
    pub ending: LineEnding,
}

/// Bounded canonical scalar/text units with native coordinate evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedRepresentation {
    pub(super) text: String,
    pub(super) lines: Vec<DecodedLine>,
    pub(super) encoding: SourceEncoding,
    pub(super) bom_stripped: bool,
    pub(super) bom_len_bytes: u64,
    pub(super) transcoded: bool,
    pub(super) native_len: u64,
    pub(super) decoded_len_chars: u64,
    pub(super) profile_id: MaterializerProfileId,
}

impl DecodedRepresentation {
    /// Decoded text (BOM removed, encoding transcoded, newlines untouched).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Decoded line table in native-byte and scalar coordinates.
    #[must_use]
    pub fn lines(&self) -> &[DecodedLine] {
        &self.lines
    }

    /// Decided source encoding.
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Whether a BOM was stripped (always recorded in the loss map).
    #[must_use]
    pub const fn bom_stripped(&self) -> bool {
        self.bom_stripped
    }

    /// Stripped BOM length in native bytes.
    #[must_use]
    pub const fn bom_len_bytes(&self) -> u64 {
        self.bom_len_bytes
    }

    /// Whether bytes were transcoded from UTF-16 (always recorded).
    #[must_use]
    pub const fn transcoded(&self) -> bool {
        self.transcoded
    }

    /// Exact native input length in bytes.
    #[must_use]
    pub const fn native_len(&self) -> u64 {
        self.native_len
    }

    /// Decoded length in Unicode scalar values.
    #[must_use]
    pub const fn decoded_len_chars(&self) -> u64 {
        self.decoded_len_chars
    }

    /// Profile identity this decoding was performed under.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }
}

impl core::fmt::Debug for DecodedRepresentation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DecodedRepresentation")
            .field("text", &format_args!("<{} chars>", self.decoded_len_chars))
            .field("line_count", &self.lines.len())
            .field("encoding", &self.encoding)
            .field("bom_stripped", &self.bom_stripped)
            .field("transcoded", &self.transcoded)
            .field("native_len", &self.native_len)
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}
