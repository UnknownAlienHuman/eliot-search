//! Crate façade for deterministic lexical analysis and sparse encoding.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

#[path = "lib.rs"]
mod analyzer;
mod code_identifiers;
mod frozen;
mod scoring;
mod sparse;

pub use analyzer::*;
pub use code_identifiers::*;
pub use frozen::*;
pub use scoring::*;
pub use sparse::*;
