//! One bounded query and correlated cancellation over owned-child stdio.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod dto;
mod error;
mod frame;

pub use dto::{
    ClientFrame, DecodedUtf8Gap, DecodedUtf8Item, DecodedUtf8Match, DecodedUtf8Request,
    DecodedUtf8Result, EvidenceMetadata, ExpansionContent, FailureCode, ObjectKindDto,
    ObjectRefDto, QueryOutcome, RawExactGap, RawExactItem, RawExactMatch, RawExactRequest,
    RawExactResult, RawSourceRef, SearchGap, SearchHit, SearchItem, SearchRequest,
    SessionClientFrame, SessionOutcome, SessionReply, StopReason, Terminal, WindowEntry,
    WindowExpansion, WindowItem, WindowPage, WindowQuery, WindowSummary,
};
pub use error::ProtocolError;
pub use frame::{
    FrameBuffer, decode_client_frame, decode_session_frame, decode_session_reply,
    decode_terminal_frame, encode_client_frame, encode_session_frame, encode_session_reply,
    encode_terminal_frame, prepare_complete, prepare_decoded_utf8, prepare_raw_exact,
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

/// Default CLI page size; explicit session data page sizes remain 1..=64.
pub const DEFAULT_WINDOW_PAGE_SIZE: u8 = 16;
/// Selected default and maximum fixed lifetime, starting at accepted Open before work/pinning.
pub const MAX_WINDOW_TTL_MS: u64 = 120_000;
/// Sole window identity in one owned child; new queries require a new child session.
pub const SESSION_WINDOW_ID: u64 = 1;
/// Maximum total returned expansion bytes, including the original emitted span.
pub const MAX_EXPANSION_BYTES: u64 = 65_536;

#[cfg(test)]
mod tests;
