//! Exact bounded UTF-8 materialization for retained source revisions.
//!
//! Public crate paths remain stable while bounded modules own errors, UTF-8
//! preparation, decoding, mapping, product construction, profiles and providers.
//! The legacy artifact adapter owns qualified immutable preparation-object I/O
//! during the T02 migration; source revision bytes remain contract inputs.

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
mod legacy_artifact;
mod legacy_direct;
mod legacy_store;
mod legacy_direct_profile;
mod legacy_direct_receipt;
mod maps;
mod normalize;
mod product;
mod profile;
mod provider;
mod request;
mod utf8;

pub use error::MaterializationError;
pub use legacy_artifact::*;
pub use legacy_store::*;
pub use utf8::{
    DEFAULT_MATERIALIZATION_LIMITS, LineEnding, LineEndingEvidence, LineSpan,
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision, materialize, materialize_utf8,
};
