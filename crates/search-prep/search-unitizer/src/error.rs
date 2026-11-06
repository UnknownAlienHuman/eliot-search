//! Explicit content, execution and saved-artifact failures.

use std::fmt;

use search_materializer::MaterializationError;

/// A preparation refusal or operation failure, never a fabricated empty result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    /// The exact input was refused by the binary-control policy.
    BinaryContent,
    /// The exact input is not UTF-8.
    InvalidUtf8,
    /// The input exceeds its execution byte ceiling.
    InputTooLarge,
    /// The representation exceeds its execution byte ceiling.
    OutputTooLarge,
    /// The line inventory exceeds its execution ceiling.
    TooManyLines,
    /// The descriptor inventory exceeds its execution ceiling.
    TooManyUnits,
    /// A Unicode scalar cannot fit within the selected hard unit size.
    UnitTooLarge,
    /// The complete encoding exceeds its execution byte ceiling.
    EncodedTooLarge,
    /// The selected transform versions or chunk settings are unsupported.
    InvalidSettings,
    /// An execution ceiling is zero.
    InvalidLimits,
    /// A bounded allocation failed.
    AllocationFailed,
    /// A coordinate or allocation size cannot be represented.
    OffsetOverflow,
    /// Cancellation was observed.
    Cancelled,
    /// The original deadline expired.
    Deadline,
    /// Saved framing, UTF-8, counts or descriptors are invalid.
    InvalidArtifact,
    /// Source, registration, input or transform binding differs.
    BindingMismatch,
    /// Representation bytes do not match the exact retained input digest.
    IntegrityFailure,
}

impl PreparationError {
    /// Stable content-free machine error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::BinaryContent => "PREPARATION_BINARY_CONTENT",
            Self::InvalidUtf8 => "PREPARATION_INVALID_UTF8",
            Self::InputTooLarge => "PREPARATION_INPUT_TOO_LARGE",
            Self::OutputTooLarge => "PREPARATION_OUTPUT_TOO_LARGE",
            Self::TooManyLines => "PREPARATION_TOO_MANY_LINES",
            Self::TooManyUnits => "PREPARATION_TOO_MANY_UNITS",
            Self::UnitTooLarge => "PREPARATION_UNIT_TOO_LARGE",
            Self::EncodedTooLarge => "PREPARATION_ENCODED_TOO_LARGE",
            Self::InvalidSettings => "PREPARATION_SETTINGS_INVALID",
            Self::InvalidLimits => "PREPARATION_LIMITS_INVALID",
            Self::AllocationFailed => "PREPARATION_ALLOCATION_FAILED",
            Self::OffsetOverflow => "PREPARATION_OFFSET_OVERFLOW",
            Self::Cancelled => "PREPARATION_CANCELLED",
            Self::Deadline => "PREPARATION_DEADLINE",
            Self::InvalidArtifact => "PREPARATION_ARTIFACT_INVALID",
            Self::BindingMismatch => "PREPARATION_BINDING_MISMATCH",
            Self::IntegrityFailure => "PREPARATION_INTEGRITY_FAILURE",
        }
    }
}

impl From<MaterializationError> for PreparationError {
    fn from(error: MaterializationError) -> Self {
        match error {
            MaterializationError::InvalidLimits => Self::InvalidLimits,
            MaterializationError::InputTooLarge => Self::InputTooLarge,
            MaterializationError::OutputTooLarge => Self::OutputTooLarge,
            MaterializationError::BinaryContent => Self::BinaryContent,
            MaterializationError::InvalidUtf8 => Self::InvalidUtf8,
            MaterializationError::TooManyLines => Self::TooManyLines,
            MaterializationError::AllocationFailed => Self::AllocationFailed,
            MaterializationError::OffsetOverflow => Self::OffsetOverflow,
            MaterializationError::Cancelled => Self::Cancelled,
            MaterializationError::Deadline => Self::Deadline,
        }
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PreparationError {}
