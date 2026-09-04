//! Crate façade for deterministic lexical analysis and sparse encoding.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

#[path = "lib.rs"]
mod analyzer;
mod sparse;

pub use analyzer::*;
pub use sparse::*;
