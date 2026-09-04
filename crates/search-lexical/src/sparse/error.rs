//! Closed sparse-encoding failures.

use core::fmt;

use crate::analyzer::LexicalError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SparseError {
    Analyzer(LexicalError),
    InvalidProfile,
    ProfileUnqualified,
    QualificationMismatch,
    InvalidLimits,
    FeatureCollision,
    CollisionThresholdExceeded,
    FeatureBudgetExceeded,
    CollisionBudgetExceeded,
    StatisticsRequired,
    StatisticsUnexpected,
    StatisticsInvalid,
    DoubleIdf,
    NonFiniteWeight,
    EmptyVector,
    Cancelled,
    FingerprintOverflow,
}

impl SparseError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Analyzer(_) => "SPARSE_ANALYZER_FAILED",
            Self::InvalidProfile => "SPARSE_PROFILE_INVALID",
            Self::ProfileUnqualified => "SPARSE_PROFILE_UNQUALIFIED",
            Self::QualificationMismatch => "SPARSE_QUALIFICATION_MISMATCH",
            Self::InvalidLimits => "SPARSE_LIMITS_INVALID",
            Self::FeatureCollision => "SPARSE_FEATURE_COLLISION",
            Self::CollisionThresholdExceeded => "SPARSE_COLLISION_THRESHOLD_EXCEEDED",
            Self::FeatureBudgetExceeded => "SPARSE_FEATURE_BUDGET_EXCEEDED",
            Self::CollisionBudgetExceeded => "SPARSE_COLLISION_BUDGET_EXCEEDED",
            Self::StatisticsRequired => "SPARSE_STATISTICS_REQUIRED",
            Self::StatisticsUnexpected => "SPARSE_STATISTICS_UNEXPECTED",
            Self::StatisticsInvalid => "SPARSE_STATISTICS_INVALID",
            Self::DoubleIdf => "SPARSE_DOUBLE_IDF",
            Self::NonFiniteWeight => "SPARSE_WEIGHT_NON_FINITE",
            Self::EmptyVector => "SPARSE_VECTOR_EMPTY",
            Self::Cancelled => "SPARSE_ENCODING_CANCELLED",
            Self::FingerprintOverflow => "SPARSE_FINGERPRINT_OVERFLOW",
        }
    }
}

impl fmt::Display for SparseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analyzer(error) => write!(formatter, "{}:{}", self.code(), error.code()),
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for SparseError {}

impl From<LexicalError> for SparseError {
    fn from(value: LexicalError) -> Self {
        Self::Analyzer(value)
    }
}
