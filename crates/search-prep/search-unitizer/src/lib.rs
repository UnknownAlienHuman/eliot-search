//! Complete exact UTF-8 prepared artifacts over verified retained source bytes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod layout;
mod prepared;

pub use error::PreparationError;
pub use layout::UnitSpan;
pub use prepared::{PreparationLimits, PreparedUtf8, decode_prepared_utf8, prepare_utf8};

#[cfg(test)]
mod tests;
