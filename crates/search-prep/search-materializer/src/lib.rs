//! Exact bounded UTF-8 materialization for retained source revisions.
//!
//! The byte transform is shared by receipt-bound callers and the DIRECT readback
//! adapter. It performs no I/O, hashing or authorization. The adapter must verify
//! immutable source bytes before preparation; a text value is not a receipt.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions,
    clippy::must_use_candidate, clippy::too_many_lines)]

use core::fmt;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

/// Conservative finite materialization limits.
pub const DEFAULT_MATERIALIZATION_LIMITS: MaterializationLimits = MaterializationLimits {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
};

/// Closed content-free materialization failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MaterializationError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// Retained revision is empty.
    EmptyInput,
    /// Input exceeds its finite byte ceiling.
    InputTooLarge,
    /// Output exceeds its finite byte ceiling.
    OutputTooLarge,
    /// Retained bytes are not strict UTF-8.
    InvalidUtf8,
    /// NUL bytes or binary control density indicate unsupported binary content.
    BinaryContent,
    /// Number of logical lines exceeds its finite ceiling.
    TooManyLines,
    /// Byte-offset arithmetic overflowed.
    OffsetOverflow,
    /// Caller-provided retained byte count differs from exact bytes.
    InputLengthMismatch,
    /// Caller-provided exact content digest is absent from the retained revision.
    MissingContentDigest,
    /// Required content-free retained-revision receipt is absent.
    MissingRevisionReceipt,
}

impl MaterializationError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "MATERIALIZATION_INVALID_LIMITS",
            Self::EmptyInput => "MATERIALIZATION_EMPTY_INPUT",
            Self::InputTooLarge => "MATERIALIZATION_INPUT_TOO_LARGE",
            Self::OutputTooLarge => "MATERIALIZATION_OUTPUT_TOO_LARGE",
            Self::InvalidUtf8 => "MATERIALIZATION_INVALID_UTF8",
            Self::BinaryContent => "MATERIALIZATION_BINARY_CONTENT",
            Self::TooManyLines => "MATERIALIZATION_TOO_MANY_LINES",
            Self::OffsetOverflow => "MATERIALIZATION_OFFSET_OVERFLOW",
            Self::InputLengthMismatch => "MATERIALIZATION_INPUT_LENGTH_MISMATCH",
            Self::MissingContentDigest => "MATERIALIZATION_MISSING_CONTENT_DIGEST",
            Self::MissingRevisionReceipt => "MATERIALIZATION_MISSING_REVISION_RECEIPT",
        }
    }
}
impl fmt::Display for MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for MaterializationError {}

/// Finite materialization limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationLimits {
    /// Maximum exact retained input bytes.
    pub max_input_bytes: usize,
    /// Maximum exact materialized output bytes.
    pub max_output_bytes: usize,
    /// Maximum logical lines, including a final unterminated line.
    pub max_lines: usize,
}
impl MaterializationLimits {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, MaterializationError> {
        if self.max_input_bytes == 0 || self.max_output_bytes == 0 || self.max_lines == 0 {
            Err(MaterializationError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Exact retained revision supplied by the revision store.
#[derive(Clone, Eq, PartialEq)]
pub struct RetainedRevision {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Monotone retained revision.
    pub revision: NonZeroRevision,
    /// Exact content digest, verified by the caller's readback adapter.
    pub content_digest: Option<Blake3Digest32>,
    /// Caller-recorded exact byte count.
    pub byte_count: u64,
    bytes: Vec<u8>,
    /// Content-free durable revision-store receipt.
    pub revision_receipt: Option<ReceiptRef>,
}
impl RetainedRevision {
    /// Creates an exact retained revision value; does not verify its digest.
    #[must_use]
    pub const fn new(
        source_id: OpaqueId,
        revision: NonZeroRevision,
        content_digest: Option<Blake3Digest32>,
        byte_count: u64,
        bytes: Vec<u8>,
        revision_receipt: Option<ReceiptRef>,
    ) -> Self {
        Self { source_id, revision, content_digest, byte_count, bytes, revision_receipt }
    }
    /// Exact retained bytes.
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    /// Exact retained byte length in memory.
    pub const fn len(&self) -> usize { self.bytes.len() }
    /// Returns whether the retained revision is empty.
    pub const fn is_empty(&self) -> bool { self.bytes.is_empty() }
}
impl fmt::Debug for RetainedRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("RetainedRevision")
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("content_digest", &self.content_digest)
            .field("byte_count", &self.byte_count)
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("revision_receipt", &self.revision_receipt).finish()
    }
}

/// Exact line terminator found in retained bytes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LineEnding {
    /// Line-feed byte.
    Lf,
    /// Carriage-return plus line-feed bytes.
    CrLf,
    /// Standalone carriage-return byte.
    Cr,
    /// Final logical line has no terminator.
    None,
}
impl LineEnding {
    /// Exact terminator byte length.
    pub const fn byte_len(self) -> u8 {
        match self { Self::Lf | Self::Cr => 1, Self::CrLf => 2, Self::None => 0 }
    }
}

/// Exact source-byte span for one logical line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineSpan {
    /// Zero-based logical line index.
    pub line_index: u64,
    /// Inclusive source-byte start.
    pub source_start: u64,
    /// Exclusive source-byte end including the terminator.
    pub source_end: u64,
    /// Exclusive source-byte end excluding the terminator.
    pub content_end: u64,
    /// Exact retained line ending.
    pub ending: LineEnding,
}
impl LineSpan {
    /// Exact content byte length excluding the terminator.
    pub const fn content_len(self) -> u64 { self.content_end - self.source_start }
    /// Exact full span length including the terminator.
    pub const fn span_len(self) -> u64 { self.source_end - self.source_start }
}

/// Aggregate exact line-ending evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineEndingEvidence {
    /// Number of LF lines.
    pub lf: u64,
    /// Number of CRLF lines.
    pub crlf: u64,
    /// Number of standalone CR lines.
    pub cr: u64,
    /// Number of final unterminated lines.
    pub unterminated: u64,
}
impl LineEndingEvidence {
    /// Returns whether more than one terminated line-ending style is present.
    pub const fn is_mixed(self) -> bool {
        let mut styles = 0_u8;
        if self.lf > 0 { styles += 1; }
        if self.crlf > 0 { styles += 1; }
        if self.cr > 0 { styles += 1; }
        styles > 1
    }
}

/// Content-free exact materialization receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationReceipt {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub revision: NonZeroRevision,
    /// Exact source content digest.
    pub content_digest: Blake3Digest32,
    /// Exact input bytes.
    pub input_bytes: u64,
    /// Exact output bytes.
    pub output_bytes: u64,
    /// Exact logical line count.
    pub line_count: u64,
    /// Exact line-ending evidence.
    pub line_endings: LineEndingEvidence,
    /// Durable retained-revision receipt supplied by the caller.
    pub revision_receipt: ReceiptRef,
}

/// Exact materialized text with source-byte line mapping.
#[derive(Clone, Eq, PartialEq)]
pub struct MaterializedRevision {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub revision: NonZeroRevision,
    /// Exact source content digest.
    pub content_digest: Blake3Digest32,
    text: String,
    /// Exact logical line spans.
    pub lines: Vec<LineSpan>,
    /// Content-free materialization receipt.
    pub receipt: MaterializationReceipt,
}
impl MaterializedRevision {
    /// Exact unnormalized UTF-8 text.
    pub fn text(&self) -> &str { &self.text }
    /// Exact UTF-8 bytes.
    pub fn bytes(&self) -> &[u8] { self.text.as_bytes() }
    /// Exact output byte length.
    pub const fn len(&self) -> usize { self.text.len() }
    /// Returns whether materialized text is empty.
    pub const fn is_empty(&self) -> bool { self.text.is_empty() }
}
impl fmt::Debug for MaterializedRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("MaterializedRevision")
            .field("source_id", &self.source_id).field("revision", &self.revision)
            .field("content_digest", &self.content_digest)
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .field("lines", &self.lines).field("receipt", &self.receipt).finish()
    }
}

/// Receipt-free byte preparation, not a source identity or admission claim.
///
/// No bytes are normalized. An empty input has zero lines. Private fields keep
/// line mappings attached to the exact text that produced them.
#[derive(Clone, Eq, PartialEq)]
pub struct MaterializedText {
    text: String,
    lines: Vec<LineSpan>,
    line_endings: LineEndingEvidence,
}
impl MaterializedText {
    /// Exact text borrowed without copying.
    pub fn text(&self) -> &str { &self.text }
    /// Exact immutable line mapping.
    pub fn lines(&self) -> &[LineSpan] { &self.lines }
    /// Exact line-ending counts.
    pub const fn line_endings(&self) -> LineEndingEvidence { self.line_endings }
}
impl fmt::Debug for MaterializedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("MaterializedText")
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .field("line_count", &self.lines.len()).finish()
    }
}

/// Prepares exact UTF-8 bytes without manufacturing a revision or receipt.
///
/// This is a pure transform. A storage adapter must independently establish the
/// provenance and integrity of bytes before using its output as search evidence.
pub fn materialize_utf8(
    bytes: Vec<u8>,
    limits: MaterializationLimits,
) -> Result<MaterializedText, MaterializationError> {
    let limits = limits.validate()?;
    check_byte_limits(bytes.len(), limits)?;
    reject_binary_controls(&bytes)?;
    let text = String::from_utf8(bytes).map_err(|_| MaterializationError::InvalidUtf8)?;
    let (lines, line_endings) = scan_lines(text.as_bytes(), limits)?;
    Ok(MaterializedText { text, lines, line_endings })
}

/// Materializes a receipt-bound retained revision without normalization.
///
/// The supplied digest and revision receipt are retained, not invented or
/// cryptographically verified here. Storage readback owns those checks.
pub fn materialize(
    input: RetainedRevision,
    limits: MaterializationLimits,
) -> Result<MaterializedRevision, MaterializationError> {
    let limits = limits.validate()?;
    if input.is_empty() { return Err(MaterializationError::EmptyInput); }
    check_byte_limits(input.len(), limits)?;
    let exact_len = u64::try_from(input.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if input.byte_count != exact_len { return Err(MaterializationError::InputLengthMismatch); }
    let content_digest = input.content_digest.ok_or(MaterializationError::MissingContentDigest)?;
    let revision_receipt = input.revision_receipt.ok_or(MaterializationError::MissingRevisionReceipt)?;
    let MaterializedText { text, lines, line_endings } = materialize_utf8(input.bytes, limits)?;
    let line_count = u64::try_from(lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let receipt = MaterializationReceipt {
        source_id: input.source_id.clone(), revision: input.revision, content_digest,
        input_bytes: exact_len, output_bytes: exact_len, line_count, line_endings, revision_receipt,
    };
    Ok(MaterializedRevision {
        source_id: input.source_id, revision: input.revision, content_digest, text, lines, receipt,
    })
}

fn check_byte_limits(length: usize, limits: MaterializationLimits) -> Result<(), MaterializationError> {
    if length > limits.max_input_bytes { return Err(MaterializationError::InputTooLarge); }
    if length > limits.max_output_bytes { return Err(MaterializationError::OutputTooLarge); }
    Ok(())
}

fn reject_binary_controls(bytes: &[u8]) -> Result<(), MaterializationError> {
    if bytes.contains(&0) { return Err(MaterializationError::BinaryContent); }
    let disallowed_controls = bytes.iter()
        .filter(|byte| **byte < 0x20 && !matches!(**byte, b'\t' | b'\n' | b'\r' | 0x0c))
        .count();
    let threshold = bytes.len().div_ceil(100).max(4);
    if disallowed_controls >= threshold { Err(MaterializationError::BinaryContent) } else { Ok(()) }
}

fn scan_lines(
    bytes: &[u8],
    limits: MaterializationLimits,
) -> Result<(Vec<LineSpan>, LineEndingEvidence), MaterializationError> {
    let mut lines = Vec::new();
    let mut evidence = LineEndingEvidence::default();
    let mut start = 0_usize;
    let mut index = 0_usize;
    while index < bytes.len() {
        let ending = match bytes[index] {
            b'\n' => Some((LineEnding::Lf, 1_usize)),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => Some((LineEnding::CrLf, 2_usize)),
            b'\r' => Some((LineEnding::Cr, 1_usize)),
            _ => None,
        };
        let Some((ending, terminator_bytes)) = ending else { index += 1; continue; };
        if lines.len() >= limits.max_lines { return Err(MaterializationError::TooManyLines); }
        let content_end = index;
        let source_end = index.checked_add(terminator_bytes).ok_or(MaterializationError::OffsetOverflow)?;
        lines.push(LineSpan {
            line_index: u64::try_from(lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_start: u64::try_from(start).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_end: u64::try_from(source_end).map_err(|_| MaterializationError::OffsetOverflow)?,
            content_end: u64::try_from(content_end).map_err(|_| MaterializationError::OffsetOverflow)?,
            ending,
        });
        match ending {
            LineEnding::Lf => evidence.lf += 1,
            LineEnding::CrLf => evidence.crlf += 1,
            LineEnding::Cr => evidence.cr += 1,
            LineEnding::None => {}
        }
        start = source_end;
        index = source_end;
    }
    if start < bytes.len() {
        if lines.len() >= limits.max_lines { return Err(MaterializationError::TooManyLines); }
        lines.push(LineSpan {
            line_index: u64::try_from(lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_start: u64::try_from(start).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_end: u64::try_from(bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
            content_end: u64::try_from(bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
            ending: LineEnding::None,
        });
        evidence.unterminated += 1;
    }
    Ok((lines, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn retained(bytes: &[u8]) -> RetainedRevision {
        RetainedRevision::new(
            OpaqueId::new("source:test").expect("source"),
            NonZeroRevision::new(1).expect("revision"),
            Some(Blake3Digest32::from_bytes([1; 32])),
            u64::try_from(bytes.len()).expect("length"), bytes.to_vec(),
            Some(ReceiptRef::new("receipt:revision").expect("receipt")),
        )
    }
    #[test]
    fn exact_utf8_bytes_are_preserved() {
        let bytes = "alpha\r\nbeta\nγ".as_bytes();
        let result = materialize(retained(bytes), DEFAULT_MATERIALIZATION_LIMITS).expect("materialize");
        assert_eq!(result.bytes(), bytes);
        assert_eq!(result.receipt.input_bytes, result.receipt.output_bytes);
    }
    #[test]
    fn line_endings_and_offsets_are_exact() {
        let result = materialize(retained(b"a\r\nb\nc\rd"), DEFAULT_MATERIALIZATION_LIMITS).expect("materialize");
        assert_eq!(result.lines.len(), 4);
        assert_eq!((result.lines[0].source_start, result.lines[0].content_end, result.lines[0].source_end), (0, 1, 3));
        assert_eq!(result.lines[0].ending, LineEnding::CrLf);
        assert_eq!(result.lines[1].ending, LineEnding::Lf);
        assert_eq!(result.lines[2].ending, LineEnding::Cr);
        assert_eq!(result.lines[3].ending, LineEnding::None);
        assert!(result.receipt.line_endings.is_mixed());
    }
    #[test]
    fn final_terminator_does_not_create_phantom_line() {
        let result = materialize(retained(b"a\n"), DEFAULT_MATERIALIZATION_LIMITS).expect("materialize");
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].ending, LineEnding::Lf);
        assert_eq!(result.receipt.line_endings.unterminated, 0);
    }
    #[test]
    fn invalid_utf8_is_rejected() {
        assert_eq!(materialize(retained(&[0xff, 0xfe]), DEFAULT_MATERIALIZATION_LIMITS), Err(MaterializationError::InvalidUtf8));
    }
    #[test]
    fn nul_content_is_rejected_as_binary() {
        assert_eq!(materialize(retained(b"a\0b"), DEFAULT_MATERIALIZATION_LIMITS), Err(MaterializationError::BinaryContent));
    }
    #[test]
    fn byte_count_mismatch_is_rejected() {
        let mut input = retained(b"abc"); input.byte_count = 2;
        assert_eq!(materialize(input, DEFAULT_MATERIALIZATION_LIMITS), Err(MaterializationError::InputLengthMismatch));
    }
    #[test]
    fn line_limit_is_fail_closed() {
        let limits = MaterializationLimits { max_lines: 1, ..DEFAULT_MATERIALIZATION_LIMITS };
        assert_eq!(materialize(retained(b"a\nb"), limits), Err(MaterializationError::TooManyLines));
    }
    #[test]
    fn debug_output_does_not_dump_source_text() {
        let input = retained(b"sensitive source text");
        assert!(!format!("{input:?}").contains("sensitive source text"));
        let result = materialize(input, DEFAULT_MATERIALIZATION_LIMITS).expect("materialize");
        assert!(!format!("{result:?}").contains("sensitive source text"));
        assert!(format!("{result:?}").contains("UTF-8 bytes"));
    }
    #[test]
    fn receipt_free_and_bound_paths_use_identical_mapping() {
        for bytes in [b"a\r\nb\nc\rd".as_slice(), "αβ\n𐀀".as_bytes(), b"\n\n"] {
            let plain = materialize_utf8(bytes.to_vec(), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
            let bound = materialize(retained(bytes), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
            assert_eq!(plain.text(), bound.text());
            assert_eq!(plain.lines(), bound.lines.as_slice());
            assert_eq!(plain.line_endings(), bound.receipt.line_endings);
        }
    }
    #[test]
    fn empty_text_does_not_create_a_revision_receipt() {
        let text = materialize_utf8(Vec::new(), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
        assert!(text.text().is_empty()); assert!(text.lines().is_empty());
        assert_eq!(materialize(retained(b""), DEFAULT_MATERIALIZATION_LIMITS), Err(MaterializationError::EmptyInput));
    }
    #[test]
    fn byte_preparation_keeps_finite_limits_and_redacted_debug() {
        let limits = MaterializationLimits { max_input_bytes: 2, ..DEFAULT_MATERIALIZATION_LIMITS };
        assert_eq!(materialize_utf8(b"abc".to_vec(), limits), Err(MaterializationError::InputTooLarge));
        let text = materialize_utf8(b"private-sentinel".to_vec(), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
        assert!(!format!("{text:?}").contains("private-sentinel"));
    }
}
