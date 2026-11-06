//! Concrete execution ceilings and owned UTF-8 product.

use std::fmt;
use std::mem::size_of;

use crate::MaterializationError;

/// Fail-only execution ceilings, excluded from transform identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationLimits {
    /// Maximum retained input bytes.
    pub max_input_bytes: usize,
    /// Maximum unchanged representation bytes.
    pub max_output_bytes: usize,
    /// Maximum logical lines.
    pub max_lines: usize,
}

impl MaterializationLimits {
    pub(super) fn validate(self) -> Result<(), MaterializationError> {
        if self.max_input_bytes == 0 || self.max_output_bytes == 0 || self.max_lines == 0 {
            return Err(MaterializationError::InvalidLimits);
        }
        self.max_lines
            .checked_mul(size_of::<LineSpan>())
            .filter(|bytes| *bytes <= isize::MAX.unsigned_abs())
            .ok_or(MaterializationError::OffsetOverflow)?;
        Ok(())
    }
}

/// Exact line coordinates in unchanged source and representation bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineSpan {
    /// Zero-based logical line index.
    pub line_index: u64,
    /// Inclusive byte start.
    pub source_start: u64,
    /// Exclusive byte end including its terminator.
    pub source_end: u64,
    /// Exclusive byte end excluding its terminator.
    pub content_end: u64,
}

/// The original input allocation as UTF-8, with one attached line inventory.
#[derive(Eq, PartialEq)]
pub struct MaterializedText {
    pub(super) text: String,
    pub(super) lines: Vec<LineSpan>,
}

impl MaterializedText {
    /// Borrows the exact unchanged representation.
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text.as_str()
    }

    /// Borrows the inventory produced from this exact representation.
    #[must_use]
    pub const fn lines(&self) -> &[LineSpan] {
        self.lines.as_slice()
    }
}

impl fmt::Debug for MaterializedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializedText")
            .field("text_bytes", &self.text.len())
            .field("line_count", &self.lines.len())
            .finish()
    }
}
