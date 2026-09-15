//! Public handle products and private retained provenance records.

use std::time::Instant;

/// Public non-self-describing handle attached to one match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHandledMatch {
    pub(crate) source_handle: String,
    pub(crate) evidence_id: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
    pub(crate) source_byte_length: u64,
    pub(crate) expires_in_ms: u64,
}

#[derive(Clone, Debug)]
pub(super) struct ResultHandleRecord {
    pub(super) namespace_id: String,
    pub(super) session_tag: u64,
    pub(super) source_fence_digest: String,
    pub(super) source_id: String,
    pub(super) revision_id: String,
    pub(super) content_digest: String,
    pub(super) byte_length: u64,
    pub(super) expires_at: Instant,
}

/// Exact bounded expansion of one opaque handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultHandleExpansion {
    pub(crate) source_handle: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) source_byte_length: u64,
    pub(crate) bytes: Vec<u8>,
}
