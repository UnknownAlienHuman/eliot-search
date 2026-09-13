//! Exact bounded UTF-8 materialization for retained source revisions.
//!
//! Public crate paths remain stable while bounded modules own errors, UTF-8
//! preparation, decoding, mapping, product construction, profiles and providers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::too_many_lines
)]

/// Public entry module: the only cross-package entry for materializer behavior.
pub mod api;
mod assurance;
mod decode;
mod error;
mod maps;
mod normalize;
mod product;
mod profile;
mod provider;
mod request;
mod utf8;

pub use error::MaterializationError;
pub use utf8::{
    DEFAULT_MATERIALIZATION_LIMITS, LineEnding, LineEndingEvidence, LineSpan,
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision, materialize, materialize_utf8,
};
