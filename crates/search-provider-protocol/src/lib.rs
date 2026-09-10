//! Bounded authenticated local-provider protocol semantics.
//!
//! This package performs no socket, pipe, filesystem, process, or secret-store
//! I/O. Transport adapters supply complete finite frames, ceremony material
//! and cryptographic proof digests; this package validates limits,
//! sequencing, replay, progress, pairing, envelopes and session lifecycle
//! before a daemon admits work.
//!
//! Layer order, bottom to top:
//!
//! 1. [`frame`] — canonical `u32`-LE length plus UTF-8 JSON over
//!    `search-contracts`, 8 MiB ceiling, no compression, no fragments.
//! 2. [`negotiation`] — exact major/minor hello negotiation on the canonical
//!    `ProtocolVersion` / `ProtocolRange` (never duplicated here).
//! 3. [`pairing`] — pairing kernel (#73): non-zero ceremony material,
//!    domain-separated proof transcripts, closed ceremony state machine,
//!    single-use challenge ledger. Keyed digests are computed by the
//!    secret-owning daemon adapter over the exact transcripts built here.
//! 4. [`request`] — authenticated envelopes (#89): per-request proof bound
//!    to version, server nonce, request ID, closed command and body digest,
//!    strict fixed-size decoding, 32-in-flight registry, explicit deadlines.
//! 5. [`binding`] — session composition: pairing-first sequencing enforced
//!    in code; envelope admission requires the ceremony token.
//!
//! The public entry module is [`api`]; the crate root re-exports the same
//! surface unchanged.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

pub mod api;
pub mod binding;
pub mod cancel;
pub mod cleanup;
pub mod config;
pub mod error;
pub mod frame;
pub mod negotiation;
pub mod pairing;
pub mod progress;
pub mod request;
pub mod session;
pub mod terminal;

/// Canonical transport payload owned by `search-contracts`.
pub use search_contracts::protocol::{JsonFramePayload, ProviderEnvelope};
/// Canonical protocol identity owned by `search-contracts`.
///
/// Norm #89 forbids duplicate canonical `ProtocolVersion` / `ProtocolRange`
/// definitions: this package re-exports the canonical types instead of
/// defining its own single-`u16` versions.
pub use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};

pub use api::*;
