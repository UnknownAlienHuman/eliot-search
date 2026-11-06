//! Exact bounded UTF-8 materialization of verified retained source bytes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod utf8;

pub use error::MaterializationError;
pub use utf8::{LineSpan, MaterializationLimits, MaterializedText, materialize_utf8};
