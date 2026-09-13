//! UTF-8 preparation limits, retained input and output models.

use core::fmt;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use crate::error::MaterializationError;

/// Conservative finite materialization limits.
pub const DEFAULT_MATERIALIZATION_LIMITS: MaterializationLimits = MaterializationLimits {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
};

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
    pub(super) bytes: Vec<u8>,
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
        Self {
            source_id,
            revision,
            content_digest,
            byte_count,
            bytes,
            revision_receipt,
        }
    }

    /// Exact retained bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Exact retained byte length in memory.
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the retained revision is empty.
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for RetainedRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedRevision")
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("content_digest", &self.content_digest)
            .field("byte_count", &self.byte_count)
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("revision_receipt", &self.revision_receipt)
            .finish()
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
        match self {
            Self::Lf | Self::Cr => 1,
            Self::CrLf => 2,
            Self::None => 0,
        }
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
    pub const fn content_len(self) -> u64 {
        self.content_end - self.source_start
    }

    /// Exact full span length including the terminator.
    pub const fn span_len(self) -> u64 {
        self.source_end - self.source_start
    }
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
        if self.lf > 0 {
            styles += 1;
        }
        if self.crlf > 0 {
            styles += 1;
        }
        if self.cr > 0 {
            styles += 1;
        }
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
    pub(super) text: String,
    /// Exact logical line spans.
    pub lines: Vec<LineSpan>,
    /// Content-free materialization receipt.
    pub receipt: MaterializationReceipt,
}

impl MaterializedRevision {
    /// Exact unnormalized UTF-8 text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact UTF-8 bytes.
    pub const fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact output byte length.
    pub const fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether materialized text is empty.
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl fmt::Debug for MaterializedRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializedRevision")
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("content_digest", &self.content_digest)
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .field("lines", &self.lines)
            .field("receipt", &self.receipt)
            .finish()
    }
}

/// Receipt-free byte preparation, not a source identity or admission claim.
///
/// No bytes are normalized. An empty input has zero lines. Private fields keep
/// line mappings attached to the exact text that produced them.
#[derive(Clone, Eq, PartialEq)]
pub struct MaterializedText {
    pub(super) text: String,
    pub(super) lines: Vec<LineSpan>,
    pub(super) line_endings: LineEndingEvidence,
}

impl MaterializedText {
    /// Exact text borrowed without copying.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact immutable line mapping.
    pub fn lines(&self) -> &[LineSpan] {
        &self.lines
    }

    /// Exact line-ending counts.
    pub const fn line_endings(&self) -> LineEndingEvidence {
        self.line_endings
    }
}

impl fmt::Debug for MaterializedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializedText")
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .field("line_count", &self.lines.len())
            .field("line_endings", &self.line_endings)
            .finish()
    }
}
