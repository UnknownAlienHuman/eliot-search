//! Public publication/query inputs and finite execution ceilings.

use search_contracts::SourceId;
use std::fmt;

use crate::BackendError;

/// Maximum points in one complete durable page and SDK batch.
pub const MAX_PAGE_POINTS: usize = 128;
/// Maximum complete durable page bytes, including its metadata and text.
pub const MAX_PAGE_BYTES: usize = 1_048_576;
/// Maximum protobuf request bytes.
pub const MAX_REQUEST_BYTES: usize = 1_048_576;
/// Maximum protobuf response bytes before SDK decoding.
pub const MAX_RESPONSE_BYTES: usize = 2_097_152;
/// Maximum exact UTF-8 query bytes.
pub const MAX_QUERY_BYTES: usize = 4096;
/// Maximum returned nominations; this never limits the scoring population.
pub const MAX_HITS: usize = 64;
/// Initial CLI nomination count.
pub const DEFAULT_HITS: usize = 10;
/// Maximum complete prepared unit text; units are never truncated for upload.
pub const MAX_POINT_TEXT_BYTES: usize = 65_536;

/// Fail-only execution allowances, excluded from physical index identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendLimits {
    /// Maximum points in one SDK batch.
    pub max_batch_points: usize,
    /// Maximum complete protobuf request bytes.
    pub max_request_bytes: usize,
    /// Maximum complete protobuf response bytes.
    pub max_response_bytes: usize,
    /// Maximum exact query UTF-8 bytes.
    pub max_query_bytes: usize,
    /// Maximum returned nominations.
    pub max_hits: usize,
}

impl Default for BackendLimits {
    fn default() -> Self {
        Self {
            max_batch_points: MAX_PAGE_POINTS,
            max_request_bytes: MAX_REQUEST_BYTES,
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_query_bytes: MAX_QUERY_BYTES,
            max_hits: MAX_HITS,
        }
    }
}

impl BackendLimits {
    pub(super) fn validate(self) -> Result<(), BackendError> {
        for (selected, maximum) in [
            (self.max_batch_points, MAX_PAGE_POINTS),
            (self.max_request_bytes, MAX_REQUEST_BYTES),
            (self.max_response_bytes, MAX_RESPONSE_BYTES),
            (self.max_query_bytes, MAX_QUERY_BYTES),
            (self.max_hits, MAX_HITS),
        ] {
            if selected == 0 || selected > maximum {
                return Err(BackendError::InvalidLimits);
            }
        }
        Ok(())
    }
}

/// One publication-owned fixed point, borrowing actual prepared unit text.
#[derive(Clone, Copy)]
pub struct PointInput<'a> {
    /// Generation-bound UUID bytes selected by publication.
    pub id: [u8; 16],
    /// Persisted source identity routed through CONTROL evidence.
    pub source_id: SourceId,
    /// Complete exact retrieval text; no locally generated aliases or vectors.
    pub retrieval_text: &'a str,
}

impl fmt::Debug for PointInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PointInput")
            .field("id", &self.id)
            .field("source_id", &self.source_id)
            .field("text_bytes", &self.retrieval_text.len())
            .finish()
    }
}

/// A server nomination, not a retained excerpt or proof of source content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Publication-owned generation point identity.
    pub id: [u8; 16],
    /// Routing identity to cross-check against current view evidence.
    pub source_id: SourceId,
    /// Finite server BM25 score.
    pub score: f32,
}

pub fn collection_name(name: &str) -> Result<(), BackendError> {
    if name.is_empty()
        || name.len() > 255
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(BackendError::InvalidInput);
    }
    Ok(())
}
