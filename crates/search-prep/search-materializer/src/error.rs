//! Stable content-free materialization failures.

use core::fmt;

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
    /// Materializer profile descriptor is malformed or unsupported.
    ProfileInvalid,
    /// Request profile identity is not in the accepted profile set.
    ProfileMismatch,
    /// Materialization request is malformed or internally inconsistent.
    RequestInvalid,
    /// Exact retained revision cannot be opened from residency.
    RevisionUnavailable,
    /// Port-attested content digest differs from the requested revision digest.
    RevisionDigestMismatch,
    /// Port-attested residency differs from the requested residency.
    ResidencyMismatch,
    /// Declared source kind or transform is not supported by the profile.
    Unsupported,
    /// Byte prefix admits several encodings under the declared profile.
    EncodingAmbiguous,
    /// Declared encoding is outside the accepted profile set.
    EncodingUnsupported,
    /// Bytes are malformed or truncated for the decided encoding.
    InvalidSequence,
    /// A lossy transform is required where the profile forbids loss.
    Loss,
    /// Coordinate map is malformed, unbounded or inconsistent.
    CoordinateMapInvalid,
    /// Loss map is malformed, unbounded or inconsistent with the representation.
    LossMapInvalid,
    /// Claimed assurance exceeds what loss evidence allows.
    AssuranceViolation,
    /// A finite input/output/map/step budget is exhausted.
    BudgetExhausted,
    /// Cooperative cancellation was observed; no complete product exists.
    Cancelled,
    /// Unsaved bytes lack an explicit admitted snapshot receipt.
    UnsavedSnapshotNotAdmitted,
    /// Optional document provider descriptor is not qualified (P17 gated).
    ProviderNotQualified,
    /// Optional document provider output claim contradicts its loss evidence.
    ProviderOutputInvalid,
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
            Self::ProfileInvalid => "MATERIALIZER_PROFILE_INVALID",
            Self::ProfileMismatch => "MATERIALIZER_PROFILE_MISMATCH",
            Self::RequestInvalid => "MATERIALIZATION_REQUEST_INVALID",
            Self::RevisionUnavailable => "SOURCE_REVISION_UNAVAILABLE",
            Self::RevisionDigestMismatch => "SOURCE_REVISION_DIGEST_MISMATCH",
            Self::ResidencyMismatch => "SOURCE_RESIDENCY_MISMATCH",
            Self::Unsupported => "MATERIALIZATION_UNSUPPORTED",
            Self::EncodingAmbiguous => "SOURCE_ENCODING_AMBIGUOUS",
            Self::EncodingUnsupported => "SOURCE_ENCODING_UNSUPPORTED",
            Self::InvalidSequence => "MATERIALIZATION_INVALID_SEQUENCE",
            Self::Loss => "MATERIALIZATION_LOSS",
            Self::CoordinateMapInvalid => "COORDINATE_MAP_INVALID",
            Self::LossMapInvalid => "LOSS_MAP_INVALID",
            Self::AssuranceViolation => "MATERIALIZATION_ASSURANCE_VIOLATION",
            Self::BudgetExhausted => "MATERIALIZATION_BUDGET_EXHAUSTED",
            Self::Cancelled => "MATERIALIZATION_CANCELLED",
            Self::UnsavedSnapshotNotAdmitted => "UNSAVED_SNAPSHOT_NOT_ADMITTED",
            Self::ProviderNotQualified => "PROVIDER_NOT_QUALIFIED",
            Self::ProviderOutputInvalid => "PROVIDER_OUTPUT_INVALID",
        }
    }
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for MaterializationError {}
