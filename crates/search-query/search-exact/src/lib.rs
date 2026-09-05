//! Frozen-denominator proofs and bounded source-byte literal execution.
//!
//! The literal primitive supplies mechanics, never a complete denominator,
//! source authorization or a qualified proof. The proof API is unchanged.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod proof;
pub use proof::*;

pub mod literal;
