//! Deterministic exact-range unitization for materialized UTF-8 revisions.
//!
//! This package performs no filesystem, database, parser, or network I/O. It
//! consumes exact UTF-8 text plus contiguous source-byte line spans, prefers
//! whole-line boundaries, splits overlong lines only at UTF-8 character
//! boundaries, and proves complete no-gap/no-overlap byte accounting.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

/// Conservative finite unitization limits.
pub const DEFAULT_UNITIZATION_LIMITS: UnitizationLimits = UnitizationLimits {
    max_input_bytes: 8 * 1024 * 1024,
    preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024,
    max_lines: 1_000_000,
    max_units: 1_000_000,
};

/// Closed content-free unitization failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UnitizationError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// Materialized UTF-8 input is empty.
    EmptyInput,
    /// Materialized input exceeds its finite byte ceiling.
    InputTooLarge,
    /// Line inventory is empty or exceeds its finite ceiling.
    InvalidLineInventory,
    /// Line indices are not contiguous from zero.
    LineIndexMismatch,
    /// Line spans do not cover the exact input contiguously.
    LineCoverageMismatch,
    /// A line span is inverted or outside the exact input.
    InvalidLineSpan,
    /// A line terminator does not match exact source bytes.
    InvalidLineEnding,
    /// A line or split boundary is not a UTF-8 character boundary.
    InvalidUtf8Boundary,
    /// Unit count exceeds its finite ceiling.
    TooManyUnits,
    /// No non-empty safe unit boundary can be selected.
    NoProgress,
    /// Unit exceeds its hard byte ceiling.
    UnitTooLarge,
    /// Unit ranges contain a gap, overlap, or duplicate bytes.
    UnitCoverageMismatch,
    /// Byte or index conversion overflowed.
    OffsetOverflow,
    /// Required content-free materialization receipt is absent.
    MissingMaterializationReceipt,
}

impl UnitizationError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "UNITIZATION_INVALID_LIMITS",
            Self::EmptyInput => "UNITIZATION_EMPTY_INPUT",
            Self::InputTooLarge => "UNITIZATION_INPUT_TOO_LARGE",
            Self::InvalidLineInventory => "UNITIZATION_INVALID_LINE_INVENTORY",
            Self::LineIndexMismatch => "UNITIZATION_LINE_INDEX_MISMATCH",
            Self::LineCoverageMismatch => "UNITIZATION_LINE_COVERAGE_MISMATCH",
            Self::InvalidLineSpan => "UNITIZATION_INVALID_LINE_SPAN",
            Self::InvalidLineEnding => "UNITIZATION_INVALID_LINE_ENDING",
            Self::InvalidUtf8Boundary => "UNITIZATION_INVALID_UTF8_BOUNDARY",
            Self::TooManyUnits => "UNITIZATION_TOO_MANY_UNITS",
            Self::NoProgress => "UNITIZATION_NO_PROGRESS",
            Self::UnitTooLarge => "UNITIZATION_UNIT_TOO_LARGE",
            Self::UnitCoverageMismatch => "UNITIZATION_UNIT_COVERAGE_MISMATCH",
            Self::OffsetOverflow => "UNITIZATION_OFFSET_OVERFLOW",
            Self::MissingMaterializationReceipt => {
                "UNITIZATION_MISSING_MATERIALIZATION_RECEIPT"
            }
        }
    }
}

impl fmt::Display for UnitizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for UnitizationError {}

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
    /// Validates all dimensions as non-zero and preferred size no larger than
    /// the hard unit ceiling.
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
    pub const fn content_len(self) -> u64 {
        self.content_end - self.source_start
    }

    /// Exact full span byte length including the terminator.
    pub const fn span_len(self) -> u64 {
        self.source_end - self.source_start
    }

    /// Exact terminator byte length.
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
    /// Exact unnormalized UTF-8 text.
    text: String,
    /// Exact contiguous source-byte line spans.
    pub lines: Vec<SourceLineSpan>,
    /// Content-free materialization receipt.
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
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact UTF-8 bytes.
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact input byte length.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the exact input is empty.
    pub fn is_empty(&self) -> bool {
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
    /// Exact unit text bytes.
    text: String,
}

impl TextUnit {
    /// Exact unit UTF-8 text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact unit UTF-8 bytes.
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact unit byte length.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the unit is empty.
    pub fn is_empty(&self) -> bool {
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

/// Produces deterministic no-gap/no-overlap units.
pub fn unitize(
    input: UnitizationInput,
    limits: UnitizationLimits,
) -> Result<UnitizationResult, UnitizationError> {
    let limits = limits.validate()?;
    validate_input(&input, limits)?;
    let materialization_receipt = input
        .materialization_receipt
        .clone()
        .ok_or(UnitizationError::MissingMaterializationReceipt)?;

    let mut units = Vec::new();
    let mut source_start = 0_usize;
    while source_start < input.len() {
        if units.len() >= limits.max_units {
            return Err(UnitizationError::TooManyUnits);
        }
        let source_end = choose_unit_end(&input, source_start, limits)?;
        if source_end <= source_start {
            return Err(UnitizationError::NoProgress);
        }
        let unit_len = source_end - source_start;
        if unit_len > limits.max_unit_bytes {
            return Err(UnitizationError::UnitTooLarge);
        }
        if !input.text.is_char_boundary(source_start)
            || !input.text.is_char_boundary(source_end)
        {
            return Err(UnitizationError::InvalidUtf8Boundary);
        }
        let logical_line_start = line_for_start(&input.lines, source_start)?;
        let logical_line_end = line_for_end(&input.lines, source_end)?;
        let source_start_u64 = u64::try_from(source_start)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let source_end_u64 = u64::try_from(source_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let ordinal = u64::try_from(units.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let starts_at_line_boundary = source_start == 0
            || input.lines.iter().any(|line| {
                usize::try_from(line.source_end).ok() == Some(source_start)
            });
        let ends_at_line_boundary = input.lines.iter().any(|line| {
            usize::try_from(line.source_end).ok() == Some(source_end)
        });
        let text = input
            .text
            .get(source_start..source_end)
            .ok_or(UnitizationError::InvalidUtf8Boundary)?
            .to_owned();
        units.push(TextUnit {
            identity: UnitIdentity {
                source_id: input.source_id.clone(),
                revision: input.revision,
                ordinal,
                source_start: source_start_u64,
                source_end: source_end_u64,
            },
            logical_line_start,
            logical_line_end,
            starts_at_line_boundary,
            ends_at_line_boundary,
            text,
        });
        source_start = source_end;
    }

    validate_unit_coverage(&units, input.len(), limits)?;
    let input_bytes = u64::try_from(input.len())
        .map_err(|_| UnitizationError::OffsetOverflow)?;
    let unit_count = u64::try_from(units.len())
        .map_err(|_| UnitizationError::OffsetOverflow)?;
    let line_count = u64::try_from(input.lines.len())
        .map_err(|_| UnitizationError::OffsetOverflow)?;
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

fn validate_input(
    input: &UnitizationInput,
    limits: UnitizationLimits,
) -> Result<(), UnitizationError> {
    if input.is_empty() {
        return Err(UnitizationError::EmptyInput);
    }
    if input.len() > limits.max_input_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    if input.lines.is_empty() || input.lines.len() > limits.max_lines {
        return Err(UnitizationError::InvalidLineInventory);
    }
    let bytes = input.bytes();
    let mut cursor = 0_usize;
    for (expected_index, line) in input.lines.iter().enumerate() {
        if line.line_index
            != u64::try_from(expected_index)
                .map_err(|_| UnitizationError::OffsetOverflow)?
        {
            return Err(UnitizationError::LineIndexMismatch);
        }
        let start = usize::try_from(line.source_start)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let end = usize::try_from(line.source_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let content_end = usize::try_from(line.content_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        if start != cursor || start >= end || content_end < start || content_end > end {
            return Err(UnitizationError::InvalidLineSpan);
        }
        if end > bytes.len() {
            return Err(UnitizationError::InvalidLineSpan);
        }
        if !input.text.is_char_boundary(start)
            || !input.text.is_char_boundary(content_end)
            || !input.text.is_char_boundary(end)
        {
            return Err(UnitizationError::InvalidUtf8Boundary);
        }
        match bytes.get(content_end..end) {
            Some(b"") | Some(b"\n") | Some(b"\r") | Some(b"\r\n") => {}
            _ => return Err(UnitizationError::InvalidLineEnding),
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(UnitizationError::LineCoverageMismatch);
    }
    Ok(())
}

fn choose_unit_end(
    input: &UnitizationInput,
    start: usize,
    limits: UnitizationLimits,
) -> Result<usize, UnitizationError> {
    let input_len = input.len();
    let preferred_end = start
        .checked_add(limits.preferred_unit_bytes)
        .ok_or(UnitizationError::OffsetOverflow)?
        .min(input_len);
    let hard_end = start
        .checked_add(limits.max_unit_bytes)
        .ok_or(UnitizationError::OffsetOverflow)?
        .min(input_len);
    if hard_end == input_len && input_len - start <= limits.preferred_unit_bytes {
        return Ok(input_len);
    }

    let mut last_boundary_before_preferred = None;
    let mut first_boundary_after_preferred = None;
    for line in &input.lines {
        let boundary = usize::try_from(line.source_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        if boundary <= start {
            continue;
        }
        if boundary <= preferred_end {
            last_boundary_before_preferred = Some(boundary);
        } else if boundary <= hard_end {
            first_boundary_after_preferred = Some(boundary);
            break;
        } else {
            break;
        }
    }
    if let Some(boundary) = last_boundary_before_preferred {
        return Ok(boundary);
    }
    if let Some(boundary) = first_boundary_after_preferred {
        return Ok(boundary);
    }

    let split_target = preferred_end.min(hard_end);
    let mut boundary = floor_char_boundary(&input.text, split_target);
    if boundary <= start {
        boundary = next_char_boundary(&input.text, start, hard_end)
            .ok_or(UnitizationError::NoProgress)?;
    }
    if boundary <= start || boundary > hard_end {
        return Err(UnitizationError::NoProgress);
    }
    Ok(boundary)
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn next_char_boundary(text: &str, start: usize, hard_end: usize) -> Option<usize> {
    let mut index = start.checked_add(1)?;
    while index <= hard_end {
        if text.is_char_boundary(index) {
            return Some(index);
        }
        index = index.checked_add(1)?;
    }
    None
}

fn line_for_start(
    lines: &[SourceLineSpan],
    start: usize,
) -> Result<u64, UnitizationError> {
    for line in lines {
        let end = usize::try_from(line.source_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        if start < end {
            return Ok(line.line_index);
        }
    }
    Err(UnitizationError::LineCoverageMismatch)
}

fn line_for_end(
    lines: &[SourceLineSpan],
    end: usize,
) -> Result<u64, UnitizationError> {
    let last_byte = end.checked_sub(1).ok_or(UnitizationError::NoProgress)?;
    for line in lines {
        let start = usize::try_from(line.source_start)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        let line_end = usize::try_from(line.source_end)
            .map_err(|_| UnitizationError::OffsetOverflow)?;
        if last_byte >= start && last_byte < line_end {
            return line
                .line_index
                .checked_add(1)
                .ok_or(UnitizationError::OffsetOverflow);
        }
    }
    Err(UnitizationError::LineCoverageMismatch)
}

fn validate_unit_coverage(
    units: &[TextUnit],
    input_len: usize,
    limits: UnitizationLimits,
) -> Result<(), UnitizationError> {
    if units.is_empty() || units.len() > limits.max_units {
        return Err(UnitizationError::UnitCoverageMismatch);
    }
    let mut cursor = 0_u64;
    let mut emitted = 0_usize;
    for (expected_ordinal, unit) in units.iter().enumerate() {
        if unit.identity.ordinal
            != u64::try_from(expected_ordinal)
                .map_err(|_| UnitizationError::OffsetOverflow)?
            || unit.identity.source_start != cursor
            || unit.identity.source_end <= unit.identity.source_start
            || unit.is_empty()
            || unit.len() > limits.max_unit_bytes
        {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let declared_len = unit
            .identity
            .source_end
            .checked_sub(unit.identity.source_start)
            .ok_or(UnitizationError::UnitCoverageMismatch)?;
        if declared_len
            != u64::try_from(unit.len())
                .map_err(|_| UnitizationError::OffsetOverflow)?
        {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        emitted = emitted
            .checked_add(unit.len())
            .ok_or(UnitizationError::OffsetOverflow)?;
        cursor = unit.identity.source_end;
    }
    if cursor
        != u64::try_from(input_len).map_err(|_| UnitizationError::OffsetOverflow)?
        || emitted != input_len
    {
        return Err(UnitizationError::UnitCoverageMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str, lines: Vec<SourceLineSpan>) -> UnitizationInput {
        UnitizationInput::new(
            OpaqueId::new("source:test").expect("source"),
            NonZeroRevision::new(1).expect("revision"),
            Blake3Digest32::from_bytes([1; 32]),
            text.to_owned(),
            lines,
            Some(ReceiptRef::new("receipt:materialization").expect("receipt")),
        )
    }

    fn simple_lines(text: &str) -> Vec<SourceLineSpan> {
        let bytes = text.as_bytes();
        let mut spans = Vec::new();
        let mut start = 0_usize;
        let mut index = 0_usize;
        while index < bytes.len() {
            let (content_end, end) = if bytes[index] == b'\r'
                && bytes.get(index + 1) == Some(&b'\n')
            {
                (index, index + 2)
            } else if matches!(bytes[index], b'\n' | b'\r') {
                (index, index + 1)
            } else {
                index += 1;
                continue;
            };
            spans.push(SourceLineSpan {
                line_index: u64::try_from(spans.len()).expect("index"),
                source_start: u64::try_from(start).expect("offset"),
                source_end: u64::try_from(end).expect("offset"),
                content_end: u64::try_from(content_end).expect("offset"),
            });
            start = end;
            index = end;
        }
        if start < bytes.len() {
            spans.push(SourceLineSpan {
                line_index: u64::try_from(spans.len()).expect("index"),
                source_start: u64::try_from(start).expect("offset"),
                source_end: u64::try_from(bytes.len()).expect("offset"),
                content_end: u64::try_from(bytes.len()).expect("offset"),
            });
        }
        spans
    }

    fn limits(preferred: usize, maximum: usize) -> UnitizationLimits {
        UnitizationLimits {
            preferred_unit_bytes: preferred,
            max_unit_bytes: maximum,
            ..DEFAULT_UNITIZATION_LIMITS
        }
    }

    #[test]
    fn exact_reconstruction_has_no_gaps_or_overlap() {
        let text = "alpha\r\nbeta\nγamma";
        let result = unitize(
            input(text, simple_lines(text)),
            limits(7, 12),
        )
        .expect("unitize");
        let reconstructed = result
            .units
            .iter()
            .map(TextUnit::text)
            .collect::<String>();
        assert_eq!(reconstructed, text);
        assert_eq!(result.receipt.input_bytes, result.receipt.emitted_bytes);
    }

    #[test]
    fn line_boundaries_are_preferred_when_line_fits_hard_limit() {
        let text = "one\ntwo\nthree\n";
        let result = unitize(
            input(text, simple_lines(text)),
            limits(5, 10),
        )
        .expect("unitize");
        assert_eq!(result.units[0].text(), "one\n");
        assert!(result.units[0].ends_at_line_boundary);
        assert_eq!(result.units[1].text(), "two\n");
    }

    #[test]
    fn overlong_unicode_line_splits_only_at_character_boundaries() {
        let text = "αβγδεζηθ";
        let result = unitize(
            input(text, simple_lines(text)),
            limits(5, 6),
        )
        .expect("unitize");
        assert!(result.units.len() > 1);
        for unit in &result.units {
            assert!(unit.len() <= 6);
            assert!(unit.text().is_char_boundary(unit.text().len()));
        }
        assert_eq!(
            result.units.iter().map(TextUnit::text).collect::<String>(),
            text
        );
    }

    #[test]
    fn crlf_is_never_split_when_the_line_fits_hard_limit() {
        let text = "abcd\r\nef";
        let result = unitize(
            input(text, simple_lines(text)),
            limits(5, 8),
        )
        .expect("unitize");
        assert_eq!(result.units[0].text(), "abcd\r\n");
    }

    #[test]
    fn output_is_deterministic() {
        let text = "a\nbb\nccc\ndddd";
        let first = unitize(
            input(text, simple_lines(text)),
            limits(4, 7),
        )
        .expect("first");
        let second = unitize(
            input(text, simple_lines(text)),
            limits(4, 7),
        )
        .expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn malformed_line_coverage_is_rejected() {
        let text = "abc";
        let lines = vec![SourceLineSpan {
            line_index: 0,
            source_start: 1,
            source_end: 3,
            content_end: 3,
        }];
        assert_eq!(
            unitize(input(text, lines), limits(2, 3)),
            Err(UnitizationError::InvalidLineSpan)
        );
    }

    #[test]
    fn line_inventory_must_cover_all_bytes() {
        let text = "abc";
        let lines = vec![SourceLineSpan {
            line_index: 0,
            source_start: 0,
            source_end: 2,
            content_end: 2,
        }];
        assert_eq!(
            unitize(input(text, lines), limits(2, 3)),
            Err(UnitizationError::LineCoverageMismatch)
        );
    }

    #[test]
    fn finite_unit_limit_is_fail_closed() {
        let text = "abcdefgh";
        let limits = UnitizationLimits {
            preferred_unit_bytes: 2,
            max_unit_bytes: 2,
            max_units: 3,
            ..DEFAULT_UNITIZATION_LIMITS
        };
        assert_eq!(
            unitize(input(text, simple_lines(text)), limits),
            Err(UnitizationError::TooManyUnits)
        );
    }

    #[test]
    fn debug_does_not_dump_source_or_unit_text() {
        let text = "sensitive source text";
        let input = input(text, simple_lines(text));
        assert!(!format!("{input:?}").contains(text));
        let result = unitize(input, limits(8, 16)).expect("unitize");
        assert!(!format!("{result:?}").contains(text));
    }
}
