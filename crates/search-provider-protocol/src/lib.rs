//! One bounded query and correlated cancellation over owned-child stdio.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod dto;
mod error;
mod frame;

pub use dto::{
    ClientFrame, EvidenceMetadata, FailureCode, ObjectKindDto, ObjectRefDto, QueryOutcome,
    RawExactGap, RawExactItem, RawExactMatch, RawExactRequest, RawExactResult, RawSourceRef,
    SearchGap, SearchHit, SearchItem, SearchRequest, StopReason, Terminal,
};
pub use error::ProtocolError;
pub use frame::{
    FrameBuffer, decode_client_frame, decode_terminal_frame, encode_client_frame,
    encode_terminal_frame, prepare_complete, prepare_raw_exact,
};
pub use search_contracts::MAX_FRAME_BYTES;

/// Four-byte little-endian body-length prefix.
pub const FRAME_PREFIX_BYTES: usize = 4;
/// Complete terminal body allowance; the shared frame ceiling includes its prefix.
pub const MAX_BODY_BYTES: usize = MAX_FRAME_BYTES - FRAME_PREFIX_BYTES;
/// Exact UTF-8 query-byte ceiling of the real backend.
pub const MAX_QUERY_BYTES: usize = 4096;
/// Maximum nominations, including explicit missing-object gaps.
pub const MAX_ITEMS: usize = 64;
/// Maximum complete prepared unit text; text is never shortened to fit output.
pub const MAX_UNIT_TEXT_BYTES: usize = 65_536;
/// Original one-shot operation allowance, transmitted as a remaining duration.
pub const MAX_REMAINING_MS: u64 = 120_000;
/// Finite request-frame allowance including worst-case JSON query escaping.
pub const MAX_CLIENT_FRAME_BYTES: usize = 6 * MAX_QUERY_BYTES + 4096;
/// This one-query child uses one nonzero correlation identity.
pub const QUERY_REQUEST_ID: u64 = 1;

#[cfg(test)]
mod tests;
