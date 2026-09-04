//! ELIOT Memory OS integration boundary.
//!
//! This crate translates already-authenticated, already-decoded ELIOT client
//! intent into bounded Search dispatch state. Transport, storage, search
//! execution, clocks, credentials and process ownership remain outside it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod adapter;

pub use adapter::{
    AdapterError, AdapterLimits, AdapterProtocolVersion, AdapterSessionState, Admission,
    EliotAdapter, EliotBinding, EliotCommand, EliotRequest, RequestLifecycle, ResponseStatus,
    TerminalReceipt,
};
