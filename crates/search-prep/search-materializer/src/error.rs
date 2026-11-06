//! Failures of the selected exact UTF-8 transform.

use std::fmt;

/// Content refusals and bounded operation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializationError {
    /// An execution ceiling is zero or cannot be represented.
    InvalidLimits,
    /// The retained input exceeds its execution ceiling.
    InputTooLarge,
    /// The unchanged representation exceeds its execution ceiling.
    OutputTooLarge,
    /// The selected binary-control policy refused the input.
    BinaryContent,
    /// The input is not valid UTF-8.
    InvalidUtf8,
    /// The line inventory exceeds its execution ceiling.
    TooManyLines,
    /// A bounded allocation failed.
    AllocationFailed,
    /// A coordinate or allocation size cannot be represented.
    OffsetOverflow,
    /// Cancellation was observed.
    Cancelled,
    /// The original operation deadline expired.
    Deadline,
}

impl MaterializationError {
    /// Stable content-free error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "MATERIALIZATION_INVALID_LIMITS",
            Self::InputTooLarge => "MATERIALIZATION_INPUT_TOO_LARGE",
            Self::OutputTooLarge => "MATERIALIZATION_OUTPUT_TOO_LARGE",
            Self::BinaryContent => "MATERIALIZATION_BINARY_CONTENT",
            Self::InvalidUtf8 => "MATERIALIZATION_INVALID_UTF8",
            Self::TooManyLines => "MATERIALIZATION_TOO_MANY_LINES",
            Self::AllocationFailed => "MATERIALIZATION_ALLOCATION_FAILED",
            Self::OffsetOverflow => "MATERIALIZATION_OFFSET_OVERFLOW",
            Self::Cancelled => "MATERIALIZATION_CANCELLED",
            Self::Deadline => "MATERIALIZATION_DEADLINE",
        }
    }
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for MaterializationError {}
