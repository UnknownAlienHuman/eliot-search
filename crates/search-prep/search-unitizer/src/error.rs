//! Closed content-free unitization failures.

use core::fmt;

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
    /// A line terminator or content span differs from exact source bytes.
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
    /// Live cancellation prevents a complete unit set.
    Cancelled,
    /// The monotonic operation deadline has elapsed.
    DeadlineExceeded,
    /// Accounted work exceeds the finite call budget.
    WorkBudgetExceeded,
    /// Required content-free materialization receipt is absent.
    MissingMaterializationReceipt,
    /// Unitizer profile descriptor is malformed or unsupported.
    UnitizerProfileInvalid,
    /// Unitizer profile identity is not the accepted profile.
    UnitizerProfileMismatch,
    /// Unit manifest binding is incomplete for the claimed provenance.
    UnitManifestIncomplete,
    /// Legacy unit-manifest v1/v2 must be rebuilt, never relabelled as v3.
    UnitManifestLegacyUnsupported,
    /// A recomputed unit or manifest digest differs from the stored manifest.
    UnitManifestDigestMismatch,
    /// A stored manifest is internally inconsistent across rebuilds.
    UnitizationNondeterministic,
    /// Distinct full occurrence commitments derived the same truncated `UnitId`.
    IdentityCollision,
}

impl UnitizationError {
    /// Stable machine-readable reason code.
    #[must_use]
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
            Self::Cancelled => "UNITIZATION_CANCELLED",
            Self::DeadlineExceeded => "UNITIZATION_DEADLINE_EXCEEDED",
            Self::WorkBudgetExceeded => "UNITIZATION_WORK_BUDGET_EXCEEDED",
            Self::MissingMaterializationReceipt => "UNITIZATION_MISSING_MATERIALIZATION_RECEIPT",
            Self::UnitizerProfileInvalid => "UNITIZER_PROFILE_INVALID",
            Self::UnitizerProfileMismatch => "UNITIZER_PROFILE_MISMATCH",
            Self::UnitManifestIncomplete => "UNIT_MANIFEST_INCOMPLETE",
            Self::UnitManifestLegacyUnsupported => "UNIT_MANIFEST_LEGACY_UNSUPPORTED",
            Self::UnitManifestDigestMismatch => "UNIT_MANIFEST_DIGEST_MISMATCH",
            Self::UnitizationNondeterministic => "UNITIZATION_NONDETERMINISTIC",
            Self::IdentityCollision => "UNIT_ID_CONFLICT",
        }
    }
}

impl fmt::Display for UnitizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for UnitizationError {}
