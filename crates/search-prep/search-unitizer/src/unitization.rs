//! Receipt-bound deterministic exact-range unitization models and execution.

use core::fmt;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use crate::{UnitizationError, unitize_text};

/// Conservative finite unitization limits.
pub const DEFAULT_UNITIZATION_LIMITS: UnitizationLimits = UnitizationLimits {
    max_input_bytes: 8 * 1024 * 1024,
    preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024,
    max_lines: 1_000_000,
    max_units: 1_000_000,
};

/// Finite unitization limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitizationLimits {
    /// Maximum exact input bytes.
    pub max_input_bytes: usize,
    /// Preferred unit size used before line-boundary adjustment.
    pub preferred_unit_bytes: usize,
    /// Hard maximum unit size.
    pub max_unit_bytes: usize,
    /// Maximum exact logical lines.
    pub max_lines: usize,
    /// Maximum emitted units.
    pub max_units: usize,
}

impl UnitizationLimits {
    /// Validates finite, non-zero dimensions and preferred/hard size ordering.
    pub const fn validate(self) -> Result<Self, UnitizationError> {
        if self.max_input_bytes == 0
            || self.preferred_unit_bytes == 0
            || self.max_unit_bytes == 0
            || self.preferred_unit_bytes > self.max_unit_bytes
            || self.max_lines == 0
            || self.max_units == 0
        {
            Err(UnitizationError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Exact source-byte span for one logical materialized line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLineSpan {
    /// Zero-based contiguous logical line index.
    pub line_index: u64,
    /// Inclusive exact source-byte start.
    pub source_start: u64,
    /// Exclusive exact source-byte end including any terminator.
    pub source_end: u64,
    /// Exclusive exact source-byte end excluding the terminator.
    pub content_end: u64,
}

impl SourceLineSpan {
    /// Exact line content byte length excluding the terminator.
    #[must_use]
    pub const fn content_len(self) -> u64 {
        self.content_end - self.source_start
    }

    /// Exact full span byte length including the terminator.
    #[must_use]
    pub const fn span_len(self) -> u64 {
        self.source_end - self.source_start
    }

    /// Exact terminator byte length.
    #[must_use]
    pub const fn terminator_len(self) -> u64 {
        self.source_end - self.content_end
    }
}

/// Exact unitization input produced from one materialized retained revision.
#[derive(Clone, Eq, PartialEq)]
pub struct UnitizationInput {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub revision: NonZeroRevision,
    /// Exact source content digest.
    pub content_digest: Blake3Digest32,
    text: String,
    /// Exact contiguous source-byte line spans.
    pub lines: Vec<SourceLineSpan>,
    /// Content-free materialization receipt supplied by the caller.
    pub materialization_receipt: Option<ReceiptRef>,
}

impl UnitizationInput {
    /// Creates an input from exact materialized values.
    #[must_use]
    pub const fn new(
        source_id: OpaqueId,
        revision: NonZeroRevision,
        content_digest: Blake3Digest32,
        text: String,
        lines: Vec<SourceLineSpan>,
        materialization_receipt: Option<ReceiptRef>,
    ) -> Self {
        Self {
            source_id,
            revision,
            content_digest,
            text,
            lines,
            materialization_receipt,
        }
    }

    /// Exact UTF-8 text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact UTF-8 bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact input byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the exact input is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl fmt::Debug for UnitizationInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnitizationInput")
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("content_digest", &self.content_digest)
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .field("lines", &self.lines)
            .field("materialization_receipt", &self.materialization_receipt)
            .finish()
    }
}

/// Deterministic unit identity derived without provider or database state.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnitIdentity {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub revision: NonZeroRevision,
    /// Zero-based unit ordinal.
    pub ordinal: u64,
    /// Inclusive exact source-byte start.
    pub source_start: u64,
    /// Exclusive exact source-byte end.
    pub source_end: u64,
}

/// One deterministic exact-range text unit.
#[derive(Clone, Eq, PartialEq)]
pub struct TextUnit {
    /// Deterministic unit identity.
    pub identity: UnitIdentity,
    /// Inclusive logical line index touched by the unit.
    pub logical_line_start: u64,
    /// Exclusive logical line index touched by the unit.
    pub logical_line_end: u64,
    /// Whether the unit starts at an exact logical-line boundary.
    pub starts_at_line_boundary: bool,
    /// Whether the unit ends at an exact logical-line boundary.
    pub ends_at_line_boundary: bool,
    text: String,
}

impl TextUnit {
    /// Exact unit UTF-8 text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact unit UTF-8 bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact unit byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the unit is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl fmt::Debug for TextUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextUnit")
            .field("identity", &self.identity)
            .field("logical_line_start", &self.logical_line_start)
            .field("logical_line_end", &self.logical_line_end)
            .field("starts_at_line_boundary", &self.starts_at_line_boundary)
            .field("ends_at_line_boundary", &self.ends_at_line_boundary)
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .finish()
    }
}

/// Content-free exact unitization receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitizationReceipt {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub revision: NonZeroRevision,
    /// Exact source content digest.
    pub content_digest: Blake3Digest32,
    /// Exact input bytes.
    pub input_bytes: u64,
    /// Exact bytes covered by emitted units.
    pub emitted_bytes: u64,
    /// Number of emitted units.
    pub unit_count: u64,
    /// Number of exact logical lines.
    pub line_count: u64,
    /// Content-free materialization receipt.
    pub materialization_receipt: ReceiptRef,
}

/// Complete deterministic unitization result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitizationResult {
    /// Exact ordered units.
    pub units: Vec<TextUnit>,
    /// Content-free byte-accounting receipt.
    pub receipt: UnitizationReceipt,
}

/// Produces deterministic no-gap/no-overlap units from a receipt-bound input.
pub fn unitize(
    input: UnitizationInput,
    limits: UnitizationLimits,
) -> Result<UnitizationResult, UnitizationError> {
    let limits = limits.validate()?;
    if input.is_empty() {
        return Err(UnitizationError::EmptyInput);
    }
    let ranges = unitize_text(input.text(), &input.lines, limits)?;
    let materialization_receipt = input
        .materialization_receipt
        .ok_or(UnitizationError::MissingMaterializationReceipt)?;
    let units = ranges
        .iter()
        .enumerate()
        .map(|(ordinal, span)| {
            Ok(TextUnit {
                identity: UnitIdentity {
                    source_id: input.source_id.clone(),
                    revision: input.revision,
                    ordinal: u64::try_from(ordinal)
                        .map_err(|_| UnitizationError::OffsetOverflow)?,
                    source_start: u64::try_from(span.source_start)
                        .map_err(|_| UnitizationError::OffsetOverflow)?,
                    source_end: u64::try_from(span.source_end)
                        .map_err(|_| UnitizationError::OffsetOverflow)?,
                },
                logical_line_start: span.logical_line_start,
                logical_line_end: span.logical_line_end,
                starts_at_line_boundary: span.starts_at_line_boundary,
                ends_at_line_boundary: span.ends_at_line_boundary,
                text: input
                    .text
                    .get(span.source_start..span.source_end)
                    .ok_or(UnitizationError::InvalidUtf8Boundary)?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, UnitizationError>>()?;
    let input_bytes =
        u64::try_from(input.text.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let unit_count = u64::try_from(units.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let line_count =
        u64::try_from(input.lines.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    Ok(UnitizationResult {
        units,
        receipt: UnitizationReceipt {
            source_id: input.source_id,
            revision: input.revision,
            content_digest: input.content_digest,
            input_bytes,
            emitted_bytes: input_bytes,
            unit_count,
            line_count,
            materialization_receipt,
        },
    })
}
