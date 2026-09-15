//! Closed DIRECT result-handle failure vocabulary.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultHandleError {
    CapacityExceeded,
    TokenExhausted,
    NotFound,
    Expired,
    SourceFenceChanged,
    SourceUnavailable,
    RevisionChanged,
    RangeInvalid,
    ExpansionTooLarge,
    ReadbackMismatch,
    EntropyUnavailable,
    AccessRevoked,
    Purged,
    DurableRetentionRequired,
}

impl ResultHandleError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::CapacityExceeded => "DIRECT_RESULT_HANDLE_CAPACITY_EXCEEDED",
            Self::TokenExhausted => "DIRECT_RESULT_HANDLE_TOKEN_EXHAUSTED",
            Self::NotFound => "DIRECT_RESULT_HANDLE_NOT_FOUND",
            Self::Expired => "DIRECT_RESULT_HANDLE_EXPIRED",
            Self::SourceFenceChanged => "DIRECT_RESULT_HANDLE_SOURCE_FENCE_CHANGED",
            Self::SourceUnavailable => "DIRECT_RESULT_HANDLE_SOURCE_UNAVAILABLE",
            Self::RevisionChanged => "DIRECT_RESULT_HANDLE_REVISION_CHANGED",
            Self::RangeInvalid => "DIRECT_RESULT_HANDLE_RANGE_INVALID",
            Self::ExpansionTooLarge => "DIRECT_RESULT_HANDLE_EXPANSION_TOO_LARGE",
            Self::ReadbackMismatch => "DIRECT_RESULT_HANDLE_READBACK_MISMATCH",
            Self::EntropyUnavailable => "DIRECT_RESULT_HANDLE_ENTROPY_UNAVAILABLE",
            Self::AccessRevoked => "DIRECT_RESULT_HANDLE_ACCESS_REVOKED",
            Self::Purged => "DIRECT_RESULT_HANDLE_PURGED",
            Self::DurableRetentionRequired => "DIRECT_RESULT_HANDLE_DURABLE_RETENTION_REQUIRED",
        }
    }
}
