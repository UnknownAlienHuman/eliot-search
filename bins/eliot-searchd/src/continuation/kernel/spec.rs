//! Closed continuation limits and failure vocabulary.

use std::time::Duration;

/// Maximum simultaneous process-local continuation windows.
pub const MAX_CONTINUATIONS: usize = 16;
/// Maximum matches retained across all continuation windows.
pub const MAX_RETAINED_MATCHES: usize = 25_000;
/// Maximum matches returned by one page.
pub const MAX_PAGE_SIZE: usize = 1_000;
/// Default matches returned by one page.
pub const DEFAULT_PAGE_SIZE: usize = 100;
/// Maximum source-gap details retained on the first page.
pub const MAX_GAP_DETAILS: usize = 256;
/// Finite process-local continuation lifetime.
pub const CONTINUATION_TTL: Duration = Duration::from_mins(15);

/// Closed continuation-window failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationError {
    InvalidPageSize,
    NotFound,
    Expired,
    SourceFenceChanged,
    CapacityExceeded,
    TokenExhausted,
    EntropyUnavailable,
    AccessRevoked,
    Purged,
}

impl ContinuationError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidPageSize => "DIRECT_CONTINUATION_PAGE_SIZE_INVALID",
            Self::NotFound => "DIRECT_CONTINUATION_NOT_FOUND",
            Self::Expired => "DIRECT_CONTINUATION_EXPIRED",
            Self::SourceFenceChanged => "DIRECT_CONTINUATION_SOURCE_FENCE_CHANGED",
            Self::CapacityExceeded => "DIRECT_CONTINUATION_CAPACITY_EXCEEDED",
            Self::TokenExhausted => "DIRECT_CONTINUATION_TOKEN_EXHAUSTED",
            Self::EntropyUnavailable => "DIRECT_CONTINUATION_ENTROPY_UNAVAILABLE",
            Self::AccessRevoked => "DIRECT_CONTINUATION_ACCESS_REVOKED",
            Self::Purged => "DIRECT_CONTINUATION_PURGED",
        }
    }
}

pub(super) const fn validate_page_size(
    page_size: usize,
) -> Result<(), ContinuationError> {
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        Err(ContinuationError::InvalidPageSize)
    } else {
        Ok(())
    }
}
